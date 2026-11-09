//! egui application: tray integration plus the popup / settings / about pages.

use crate::hidpp::{Charging, Outcome, Reading};
use crate::icon::{self, Tone};
use crate::monitor::{Command, MonitorHandle, Shared};
use crate::notify::{CRITICAL_THRESHOLD, LOW_THRESHOLD};
use crate::settings::{APP_NAME, REFRESH_CHOICES, Settings};
use crate::startup;
use crate::update::{self, UpdateStatus};
use crate::winutil;
use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Stroke, Vec2};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Popup,
    Settings,
    About,
}

impl Page {
    fn size(self) -> Vec2 {
        match self {
            Page::Popup => Vec2::new(320.0, 288.0),
            Page::Settings => Vec2::new(360.0, 306.0),
            Page::About => Vec2::new(360.0, 404.0),
        }
    }
}

pub enum UiEvent {
    TrayClick { x: f64, y: f64 },
    Menu(MenuId),
    Show(Page),
    UpdateDone(UpdateStatus),
}

const BG: Color32 = Color32::from_rgb(0x1B, 0x1C, 0x21);
const CARD: Color32 = Color32::from_rgb(0x26, 0x27, 0x2E);
const TEXT_DIM: Color32 = Color32::from_rgb(0x9A, 0x9C, 0xA8);
const TEXT: Color32 = Color32::from_rgb(0xEC, 0xED, 0xF2);

struct MenuIds {
    show: MenuId,
    settings: MenuId,
    startup: MenuId,
    update: MenuId,
    about: MenuId,
    exit: MenuId,
}

pub struct App {
    page: Page,
    settings: Settings,
    monitor: MonitorHandle,
    events: Receiver<UiEvent>,
    event_tx: std::sync::mpsc::Sender<UiEvent>,
    tray: TrayIcon,
    ids: MenuIds,
    startup_item: CheckMenuItem,
    tray_sig: Option<(Option<u8>, Tone, String)>,
    visible: bool,
    seen_focus: bool,
    shown_at: Instant,
    pending_place: Option<Option<(f64, f64)>>,
    styled: bool,
    quitting: bool,
    startup_error: Option<String>,
    autohide: bool,
}

fn load_system_font(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    if let Ok(bytes) = std::fs::read(format!(r"{windir}\Fonts\segoeui.ttf")) {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "segoe".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "segoe".into());
        ctx.set_fonts(fonts);
    }
}

fn style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = BG;
    v.override_text_color = Some(TEXT);
    v.widgets.noninteractive.bg_fill = CARD;
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x30, 0x31, 0x3A);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x30, 0x31, 0x3A);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x3B, 0x3C, 0x47);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x3B, 0x3C, 0x47);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x46, 0x47, 0x54);
    v.selection.bg_fill = Color32::from_rgb(0x4F, 0xC3, 0xF7);
    v.selection.stroke = Stroke::new(1.0, Color32::BLACK);
    let r = CornerRadius::same(8);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    ctx.set_visuals(v);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |s| {
            s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            s.spacing.button_padding = Vec2::new(12.0, 6.0);
        });
    }
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        settings: Settings,
        initial: Option<Page>,
        events: Receiver<UiEvent>,
        event_tx: std::sync::mpsc::Sender<UiEvent>,
    ) -> Self {
        load_system_font(&cc.egui_ctx);
        style(&cc.egui_ctx);

        let menu = Menu::new();
        let show = MenuItem::new("Show Battery", true, None);
        let settings_i = MenuItem::new("Settings", true, None);
        let startup_item =
            CheckMenuItem::new("Start with Windows", true, startup::is_enabled(), None);
        let update_i = MenuItem::new("Check for Updates", true, None);
        let about = MenuItem::new("About", true, None);
        let exit = MenuItem::new("Exit", true, None);
        let _ = menu.append_items(&[
            &show,
            &settings_i,
            &startup_item,
            &update_i,
            &about,
            &PredefinedMenuItem::separator(),
            &exit,
        ]);
        let ids = MenuIds {
            show: show.id().clone(),
            settings: settings_i.id().clone(),
            startup: startup_item.id().clone(),
            update: update_i.id().clone(),
            about: about.id().clone(),
            exit: exit.id().clone(),
        };

        let rgba = icon::render(None, Tone::Offline);
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(APP_NAME)
            .with_icon(Icon::from_rgba(rgba, icon::SIZE as u32, icon::SIZE as u32).expect("icon"))
            .build()
            .expect("tray icon");

        let ctx = cc.egui_ctx.clone();
        let tx = event_tx.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |e: tray_icon::TrayIconEvent| {
            if let tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                rect,
                ..
            } = e
            {
                let _ = tx.send(UiEvent::TrayClick {
                    x: rect.position.x + rect.size.width as f64 / 2.0,
                    y: rect.position.y,
                });
                ctx.request_repaint();
            }
        }));
        let ctx = cc.egui_ctx.clone();
        let tx = event_tx.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(
            move |e: tray_icon::menu::MenuEvent| {
                let _ = tx.send(UiEvent::Menu(e.id));
                ctx.request_repaint();
            },
        ));

        let wake_ctx = cc.egui_ctx.clone();
        let monitor =
            crate::monitor::spawn(settings.refresh_secs, settings.notifications, move || {
                wake_ctx.request_repaint()
            });

        let mut app = App {
            page: initial.unwrap_or(Page::Popup),
            settings,
            monitor,
            events,
            event_tx,
            tray,
            ids,
            startup_item,
            tray_sig: None,
            visible: false,
            seen_focus: false,
            shown_at: Instant::now(),
            pending_place: None,
            styled: false,
            quitting: false,
            startup_error: None,
            autohide: !std::env::args().any(|a| a == "--no-autohide"),
        };
        if initial.is_some() {
            app.visible = true;
            app.pending_place = Some(None);
        }
        app
    }

    fn show(&mut self, ctx: &egui::Context, page: Page, anchor: Option<(f64, f64)>) {
        self.page = page;
        self.visible = true;
        self.seen_focus = false;
        self.shown_at = Instant::now();
        self.pending_place = Some(anchor);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.request_repaint();
    }

    fn hide(&mut self, ctx: &egui::Context) {
        self.visible = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }

    fn place(&mut self, ctx: &egui::Context, anchor: Option<(f64, f64)>) {
        let ppp = ctx.pixels_per_point();
        let size = self.page.size();
        let work = winutil::work_area();
        let (mx, my) = (12.0 * ppp, 12.0 * ppp);
        let (w, h) = (size.x * ppp, size.y * ppp);
        let mut x = work.right as f32 - w - mx;
        if let Some((ax, _)) = anchor {
            x = (ax as f32 - w / 2.0).clamp(work.left as f32 + mx, work.right as f32 - w - mx);
        }
        let taskbar_on_top = work.top > 0;
        let y = if taskbar_on_top {
            work.top as f32 + my
        } else {
            work.bottom as f32 - h - my
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            x / ppp,
            y / ppp,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                UiEvent::TrayClick { x, y } => {
                    crate::diag::log(format!(
                        "tray click visible={} page={:?} x={x} y={y}",
                        self.visible, self.page
                    ));
                    // Clicking the tray icon while the popup is open toggles it closed.
                    let recently = self.shown_at.elapsed() < Duration::from_millis(400);
                    if self.visible && self.page == Page::Popup && !recently {
                        self.hide(ctx);
                    } else if !self.visible || self.page != Page::Popup {
                        self.show(ctx, Page::Popup, Some((x, y)));
                    }
                }
                UiEvent::Show(p) => self.show(ctx, p, None),
                UiEvent::UpdateDone(s) => {
                    if let Ok(mut st) = self.monitor.state.lock() {
                        st.update_status = s;
                    }
                    ctx.request_repaint();
                }
                UiEvent::Menu(id) => {
                    crate::diag::log(format!("menu {:?}", id));
                    if id == self.ids.show {
                        self.show(ctx, Page::Popup, None);
                    } else if id == self.ids.settings {
                        self.show(ctx, Page::Settings, None);
                    } else if id == self.ids.about {
                        self.show(ctx, Page::About, None);
                    } else if id == self.ids.update {
                        self.start_update_check();
                        self.show(ctx, Page::About, None);
                    } else if id == self.ids.startup {
                        // CheckMenuItem has already toggled itself.
                        let want = self.startup_item.is_checked();
                        self.apply_startup(want);
                    } else if id == self.ids.exit {
                        self.quitting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
    }

    fn apply_startup(&mut self, want: bool) {
        match startup::set_enabled(want) {
            Ok(()) => self.startup_error = None,
            Err(e) => self.startup_error = Some(format!("Could not change startup setting: {e}")),
        }
        let actual = startup::is_enabled();
        self.settings.start_with_windows = actual;
        self.startup_item.set_checked(actual);
        self.settings.save();
    }

    fn start_update_check(&mut self) {
        {
            let mut st = self.monitor.state.lock().unwrap();
            if st.update_status == UpdateStatus::Checking {
                return;
            }
            st.update_status = UpdateStatus::Checking;
        }
        let tx = self.event_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UiEvent::UpdateDone(update::check(VERSION)));
        });
    }

    fn update_tray(&mut self, snap: &Snapshot) {
        let (pct, tone) = (snap.display_percent(), snap.tone());
        let tip = snap.tooltip();
        let sig = (pct, tone, tip.clone());
        if self.tray_sig.as_ref() == Some(&sig) {
            return;
        }
        let rgba = icon::render(pct, tone);
        if let Ok(ic) = Icon::from_rgba(rgba, icon::SIZE as u32, icon::SIZE as u32) {
            let _ = self.tray.set_icon(Some(ic));
        }
        let _ = self.tray.set_tooltip(Some(tip));
        self.tray_sig = Some(sig);
    }
}

/// Immutable copy of the monitor state used for a single frame.
struct Snapshot {
    outcome: Option<Outcome>,
    updated: Option<chrono::DateTime<chrono::Local>>,
    last_good: Option<(Reading, chrono::DateTime<chrono::Local>)>,
    refreshing: bool,
    update_status: UpdateStatus,
}

impl Snapshot {
    fn from(s: &Shared) -> Self {
        Snapshot {
            outcome: s.outcome.clone(),
            updated: s.updated,
            last_good: s.last_good.clone(),
            refreshing: s.refreshing,
            update_status: s.update_status.clone(),
        }
    }
    fn reading(&self) -> Option<&Reading> {
        match &self.outcome {
            Some(Outcome::Ok(r)) => Some(r),
            _ => None,
        }
    }
    /// Percentage for the tray icon: live reading, else the last known one (drawn grey).
    fn display_percent(&self) -> Option<u8> {
        self.reading()
            .and_then(|r| r.battery.percent)
            .or_else(|| self.last_known().map(|(p, _)| p))
    }
    fn last_known(&self) -> Option<(u8, String)> {
        let (r, t) = self.last_good.as_ref()?;
        Some((r.battery.percent?, t.format("%H:%M").to_string()))
    }
    fn tone(&self) -> Tone {
        match self.reading() {
            None => Tone::Offline,
            Some(r) => match (r.battery.charging, r.battery.percent) {
                (Charging::Charging | Charging::ChargingSlow | Charging::Full, _) => Tone::Charging,
                (_, Some(p)) if p <= CRITICAL_THRESHOLD => Tone::Critical,
                (_, Some(p)) if p <= LOW_THRESHOLD => Tone::Low,
                _ => Tone::Good,
            },
        }
    }
    fn tooltip(&self) -> String {
        match &self.outcome {
            None => format!("{APP_NAME} - reading battery..."),
            Some(Outcome::Ok(r)) => {
                let p = r
                    .battery
                    .percent
                    .map(|p| format!("{p}%"))
                    .unwrap_or_else(|| "level unknown".into());
                format!(
                    "{}: {} ({})",
                    r.device_name,
                    p,
                    charging_text(r.battery.charging)
                )
            }
            Some(Outcome::DeviceOffline) => match self.last_known() {
                Some((p, t)) => format!("Mouse not responding (last known {p}% at {t})"),
                None => format!("{APP_NAME} - mouse not connected"),
            },
            Some(Outcome::NoReceiver) => format!("{APP_NAME} - no Logitech receiver found"),
            Some(Outcome::Error(_)) => format!("{APP_NAME} - HID error"),
        }
    }
}

fn charging_text(c: Charging) -> &'static str {
    match c {
        Charging::Discharging => "Discharging",
        Charging::Charging => "Charging",
        Charging::ChargingSlow => "Charging (slow)",
        Charging::Full => "Fully charged",
        Charging::Error => "Charging error",
        Charging::Unknown => "Charging status unavailable",
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    /// Runs even while the window is hidden, so the tray keeps working.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.styled {
            self.styled = true;
            winutil::round_window(APP_NAME);
        }
        self.handle_events(ctx);

        // Closing the window only hides it; Exit in the tray menu really quits.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(ctx);
        }

        if let Some(anchor) = self.pending_place.take() {
            self.place(ctx, anchor);
        }

        let snap = Snapshot::from(&self.monitor.state.lock().unwrap());
        self.update_tray(&snap);

        // Popup behaves like a flyout: dismiss when it loses focus.
        if self.visible {
            let focused = ctx.input(|i| i.viewport().focused).unwrap_or(false);
            if focused {
                self.seen_focus = true;
            } else if self.autohide
                && self.seen_focus
                && self.shown_at.elapsed() > Duration::from_millis(300)
            {
                self.hide(ctx);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.hide(ctx);
            }
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        let snap = Snapshot::from(&self.monitor.state.lock().unwrap());
        let frame = egui::Frame::new()
            .fill(BG)
            .stroke(Stroke::new(1.0, Color32::from_rgb(0x3A, 0x3B, 0x44)))
            .inner_margin(egui::Margin::same(16));
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            let title = match self.page {
                Page::Popup => APP_NAME,
                Page::Settings => "Settings",
                Page::About => "About",
            };
            let hdr = ui.horizontal(|ui| {
                if self.page != Page::Popup && ui.small_button("<  Back").clicked() {
                    self.page = Page::Popup;
                    self.pending_place = Some(None);
                }
                ui.label(RichText::new(title).size(13.0).color(TEXT_DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("x").clicked() {
                        self.hide(ctx);
                    }
                });
            });
            if hdr.response.interact(egui::Sense::drag()).drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            ui.add_space(4.0);
            match self.page {
                Page::Popup => self.popup_ui(ui, ctx, &snap),
                Page::Settings => self.settings_ui(ui),
                Page::About => self.about_ui(ui, &snap),
            }
        });
    }
}

impl App {
    fn popup_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snap: &Snapshot) {
        let tone = snap.tone();
        let c = {
            let [r, g, b] = if snap.reading().is_none() {
                Tone::Offline.rgb()
            } else {
                tone.rgb()
            };
            Color32::from_rgb(r, g, b)
        };
        // (headline, percent for bar, device label, status line, footer note)
        let (headline, bar, device, status): (String, Option<u8>, String, String);
        let mut note: Option<String> = None;
        match &snap.outcome {
            None => {
                headline = "...".into();
                bar = None;
                device = "Reading battery".into();
                status = "Please wait".into();
            }
            Some(Outcome::Ok(r)) => {
                headline = r
                    .battery
                    .percent
                    .map(|p| format!("{p}%"))
                    .unwrap_or_else(|| "--".into());
                bar = r.battery.percent;
                device = format!("{}  -  Connected", r.device_name);
                status = charging_text(r.battery.charging).into();
                if r.battery.percent.is_none() {
                    note = Some("This device does not report an exact percentage.".into());
                }
            }
            Some(Outcome::DeviceOffline) => {
                bar = None;
                device = "Mouse not responding".into();
                status = "Asleep, switched off or out of range".into();
                match snap.last_known() {
                    Some((p, t)) => {
                        headline = format!("{p}%");
                        note = Some(format!(
                            "Last known reading at {t}; checking again shortly."
                        ));
                    }
                    None => headline = "--".into(),
                }
            }
            Some(Outcome::NoReceiver) => {
                headline = "--".into();
                bar = None;
                device = "No device found".into();
                status = "Plug in the receiver or the USB cable".into();
            }
            Some(Outcome::Error(e)) => {
                headline = "--".into();
                bar = None;
                device = "Could not read the device".into();
                status = e.clone();
            }
        }

        egui::Frame::new()
            .fill(CARD)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(device).size(12.5).color(TEXT_DIM));
                ui.label(
                    RichText::new(headline)
                        .font(FontId::proportional(52.0))
                        .color(c)
                        .strong(),
                );
                ui.label(RichText::new(status).size(13.0));
                ui.add_space(4.0);
                let target = bar.map(|p| p as f32 / 100.0).unwrap_or(0.0);
                let shown = ctx.animate_value_with_time(ui.scope_id().with("bar"), target, 0.4);
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 10.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(5),
                    Color32::from_rgb(0x3A, 0x3B, 0x44),
                );
                if shown > 0.0 {
                    let mut f = rect;
                    f.set_width((rect.width() * shown).max(10.0));
                    ui.painter().rect_filled(f, CornerRadius::same(5), c);
                }
            });
        if let Some(n) = note {
            ui.label(RichText::new(n).size(11.5).color(TEXT_DIM));
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let upd = match snap.updated {
                Some(t) => format!("Updated {}", t.format("%H:%M:%S")),
                None => "Not updated yet".into(),
            };
            ui.label(RichText::new(upd).size(11.5).color(TEXT_DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Settings").clicked() {
                    self.page = Page::Settings;
                    self.pending_place = Some(None);
                }
                let label = if snap.refreshing {
                    "Refreshing..."
                } else {
                    "Refresh"
                };
                if ui
                    .add_enabled(
                        !snap.refreshing,
                        egui::Button::new(RichText::new(label).size(12.0)).small(),
                    )
                    .clicked()
                {
                    let _ = self.monitor.tx.send(Command::RefreshNow);
                }
            });
        });
        if let Some(Outcome::Ok(r)) = &snap.outcome {
            ui.label(
                RichText::new(if r.via_receiver {
                    "via LIGHTSPEED receiver"
                } else {
                    "via USB cable"
                })
                .size(11.0)
                .color(TEXT_DIM),
            );
        }
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.clone();
        egui::Frame::new()
            .fill(CARD)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Refresh interval").size(13.0));
                egui::ComboBox::from_id_salt("interval")
                    .selected_text(interval_label(self.settings.refresh_secs))
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for s in REFRESH_CHOICES {
                            ui.selectable_value(
                                &mut self.settings.refresh_secs,
                                s,
                                interval_label(s),
                            );
                        }
                    });
                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.settings.notifications,
                    "Low battery notifications (20% / 10%)",
                );
                let mut startup = self.settings.start_with_windows;
                if ui.checkbox(&mut startup, "Start with Windows").changed() {
                    self.apply_startup(startup);
                }
                ui.checkbox(
                    &mut self.settings.start_minimized,
                    "Launch minimized to tray",
                );
            });
        if let Some(e) = &self.startup_error {
            ui.colored_label(Color32::from_rgb(0xFF, 0x5C, 0x5C), e);
        }
        ui.label(
            RichText::new("Settings are saved automatically.")
                .size(11.5)
                .color(TEXT_DIM),
        );
        if self.settings != before {
            if self.settings.refresh_secs != before.refresh_secs {
                let _ = self
                    .monitor
                    .tx
                    .send(Command::SetInterval(self.settings.refresh_secs));
            }
            if self.settings.notifications != before.notifications {
                let _ = self
                    .monitor
                    .tx
                    .send(Command::SetNotifications(self.settings.notifications));
            }
            self.settings.save();
        }
    }

    fn about_ui(&mut self, ui: &mut egui::Ui, snap: &Snapshot) {
        egui::Frame::new().fill(CARD).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(14)).show(
            ui,
            |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(APP_NAME).size(20.0).strong());
                ui.label(RichText::new(format!("Version {VERSION}")).color(TEXT_DIM));
                ui.add_space(4.0);
                ui.label("A lightweight system tray battery monitor for the Logitech G Pro X Superlight 2. Talks to the mouse directly over HID++, no G HUB required.");
                ui.add_space(4.0);
                ui.label(RichText::new("Free and open source, MIT License.").size(12.0).color(TEXT_DIM));
                ui.label(
                    RichText::new("Not affiliated with or endorsed by Logitech. Logitech and G HUB are trademarks of Logitech.")
                        .size(11.5)
                        .color(TEXT_DIM),
                );
                ui.label(RichText::new("Signing status: unsigned build.").size(11.5).color(TEXT_DIM));
            },
        );
        ui.horizontal(|ui| {
            if ui.button("GitHub").clicked() {
                winutil::open_url(&format!("https://github.com/{}", update::REPO));
            }
            if ui.button("Releases").clicked() {
                winutil::open_url(update::RELEASES_URL);
            }
        });
        ui.horizontal(|ui| {
            let checking = snap.update_status == UpdateStatus::Checking;
            if ui
                .add_enabled(!checking, egui::Button::new("Check for Updates"))
                .clicked()
            {
                self.start_update_check();
            }
            match &snap.update_status {
                UpdateStatus::Idle => {}
                UpdateStatus::Checking => {
                    ui.label(RichText::new("Checking...").color(TEXT_DIM));
                }
                UpdateStatus::UpToDate => {
                    ui.label(RichText::new("You are up to date.").color(TEXT_DIM));
                }
                UpdateStatus::Available { version, url } => {
                    if ui.link(format!("Version {version} available")).clicked() {
                        winutil::open_url(url);
                    }
                }
                UpdateStatus::Failed(e) => {
                    ui.label(
                        RichText::new(format!("Check failed: {e}"))
                            .size(11.5)
                            .color(Color32::from_rgb(0xFF, 0x8A, 0x8A)),
                    );
                }
            }
        });
    }
}

fn interval_label(s: u64) -> String {
    if s < 60 {
        format!("{s} seconds")
    } else {
        format!("{} minute{}", s / 60, if s / 60 == 1 { "" } else { "s" })
    }
}

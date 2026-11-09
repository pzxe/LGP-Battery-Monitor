#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod diag;
mod hidpp;
mod icon;
mod monitor;
mod notify;
mod settings;
mod startup;
mod update;
mod winutil;

use app::{App, Page, UiEvent};
use eframe::egui;
use settings::{APP_NAME, Settings};
use std::io::Write;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);

    diag::init(has("--debug"));

    if has("--version") {
        let _ = writeln!(winutil::console_writer(), "{APP_NAME} {}", app::VERSION);
        return Ok(());
    }
    if has("--probe") {
        // CLI diagnostic: print one battery reading and exit.
        let mut out = winutil::console_writer();
        match hidapi::HidApi::new() {
            Ok(mut api) => {
                let _ = writeln!(out, "{:#?}", hidpp::read_battery(&mut api, &mut None));
            }
            Err(e) => {
                let _ = writeln!(out, "HID init failed: {e}");
            }
        }
        return Ok(());
    }
    if has("--test-notification") {
        startup::register_toast_identity();
        notify::show(notify::Notice::Low(20));
        std::thread::sleep(std::time::Duration::from_secs(2));
        return Ok(());
    }
    if has("--help") || has("-h") {
        let _ = writeln!(
            winutil::console_writer(),
            "{APP_NAME} {}\nOptions:\n  --show            open the battery popup on start\n  --page <name>     open 'settings' or 'about' on start\n  --probe           print one battery reading and exit\n  --version         print version",
            app::VERSION
        );
        return Ok(());
    }

    if !winutil::acquire_single_instance() {
        winutil::signal_existing_instance();
        return Ok(());
    }

    let settings = Settings::load();
    startup::register_toast_identity();
    // Keep the registry in sync if the user removed the entry externally.
    let mut settings = settings;
    settings.start_with_windows = startup::is_enabled();

    let page_arg = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1));
    let initial = match page_arg.map(String::as_str) {
        Some("settings") => Some(Page::Settings),
        Some("about") => Some(Page::About),
        _ if has("--show") => Some(Page::Popup),
        _ if !settings.start_minimized && !has("--autostart") => Some(Page::Popup),
        _ if !settings.start_minimized => Some(Page::Popup),
        _ => None,
    };

    let (tx, rx) = std::sync::mpsc::channel::<UiEvent>();
    let size = initial.map(p_size).unwrap_or([320.0, 288.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_visible(initial.is_some()),
        ..Default::default()
    };

    let tx_app = tx.clone();
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            let tx2 = tx.clone();
            winutil::listen_for_show(move || {
                let _ = tx2.send(UiEvent::Show(Page::Popup));
                ctx.request_repaint();
            });
            Ok(Box::new(App::new(cc, settings, initial, rx, tx_app)))
        }),
    )
}

fn p_size(p: Page) -> [f32; 2] {
    match p {
        Page::Popup => [320.0, 288.0],
        Page::Settings => [360.0, 306.0],
        Page::About => [360.0, 404.0],
    }
}

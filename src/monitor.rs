//! Background polling thread: reads the battery, raises notifications and wakes the UI.

use crate::hidpp::{self, Outcome, Reading};
use crate::notify::{self, Notifier};
use crate::update::UpdateStatus;
use chrono::{DateTime, Local};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug)]
pub struct Shared {
    pub outcome: Option<Outcome>,
    pub updated: Option<DateTime<Local>>,
    /// Last successful reading, shown (clearly labelled) while the device is offline.
    pub last_good: Option<(Reading, DateTime<Local>)>,
    pub update_status: UpdateStatus,
    pub refreshing: bool,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            outcome: None,
            updated: None,
            last_good: None,
            update_status: UpdateStatus::Idle,
            refreshing: false,
        }
    }
}

pub type SharedState = Arc<Mutex<Shared>>;

pub enum Command {
    RefreshNow,
    SetInterval(u64),
    SetNotifications(bool),
}

pub struct MonitorHandle {
    pub tx: Sender<Command>,
    pub state: SharedState,
}

pub fn spawn(
    interval_secs: u64,
    notifications: bool,
    wake: impl Fn() + Send + 'static,
) -> MonitorHandle {
    let (tx, rx) = channel();
    let state: SharedState = Arc::new(Mutex::new(Shared::default()));
    let st = state.clone();
    std::thread::Builder::new()
        .name("battery-monitor".into())
        .spawn(move || run(rx, st, interval_secs, notifications, wake))
        .expect("spawn monitor thread");
    MonitorHandle { tx, state }
}

fn run(
    rx: Receiver<Command>,
    state: SharedState,
    mut interval: u64,
    mut notifications: bool,
    wake: impl Fn(),
) {
    let mut api = hidapi::HidApi::new().ok();
    let mut notifier = Notifier::default();
    let mut hint = None;
    loop {
        if let Ok(mut s) = state.lock() {
            s.refreshing = true;
        }
        wake();

        if api.is_none() {
            api = hidapi::HidApi::new().ok();
        }
        let outcome = match api.as_mut() {
            Some(a) => hidpp::read_battery(a, &mut hint),
            None => Outcome::Error("HID subsystem unavailable".into()),
        };
        let now = Local::now();
        if let Outcome::Ok(r) = &outcome
            && notifications
            && let Some(p) = r.battery.percent
            && let Some(n) = notifier.evaluate(p, r.battery.charging)
        {
            notify::show(n);
        }
        if let Ok(mut s) = state.lock() {
            if let Outcome::Ok(r) = &outcome {
                s.last_good = Some((r.clone(), now));
            }
            s.outcome = Some(outcome);
            s.updated = Some(now);
            s.refreshing = false;
        }
        wake();

        // Wait for the interval, or react to commands.
        // While the mouse is asleep/offline, look again sooner so we notice when it wakes.
        let online = matches!(
            state.lock().ok().and_then(|s| s.outcome.clone()),
            Some(Outcome::Ok(_))
        );
        let mut deadline = Duration::from_secs(if online { interval } else { interval.min(15) });
        loop {
            let started = std::time::Instant::now();
            match rx.recv_timeout(deadline) {
                Ok(Command::RefreshNow) => break,
                Ok(Command::SetInterval(s)) => {
                    interval = s;
                    break;
                }
                Ok(Command::SetNotifications(n)) => {
                    notifications = n;
                    deadline = deadline.saturating_sub(started.elapsed());
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

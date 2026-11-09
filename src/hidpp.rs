//! Minimal HID++ 2.0 client used to read the battery of Logitech devices.
//!
//! Implemented from the publicly documented HID++ 2.0 protocol (Root feature
//! 0x0000, Unified Battery 0x1004, Battery Status 0x1000, Device Name 0x0005).
//! No third-party protocol code is copied.

use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub const LOGITECH_VID: u16 = 0x046D;
const LONG_REPORT: u8 = 0x11;
const REPORT_LEN: usize = 20;
const SW_ID: u8 = 0x07;
const IO_TIMEOUT: Duration = Duration::from_millis(600);

pub const FEATURE_DEVICE_NAME: u16 = 0x0005;
pub const FEATURE_BATTERY_STATUS: u16 = 0x1000;
pub const FEATURE_UNIFIED_BATTERY: u16 = 0x1004;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Charging {
    Discharging,
    Charging,
    ChargingSlow,
    Full,
    /// The device reported a charging fault.
    Error,
    /// The device reported a charging state we cannot interpret.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryInfo {
    /// Exact percentage if the device reports one.
    pub percent: Option<u8>,
    pub charging: Charging,
    pub feature: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub device_name: String,
    pub battery: BatteryInfo,
    pub via_receiver: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ok(Reading),
    /// Receiver or USB interface found, but the mouse is asleep / off / out of range.
    DeviceOffline,
    /// No Logitech HID++ interface at all.
    NoReceiver,
    Error(String),
}

// ---------- pure helpers (unit-tested) ----------

pub fn build_request(
    dev_index: u8,
    feature_index: u8,
    function: u8,
    params: &[u8],
) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[0] = LONG_REPORT;
    r[1] = dev_index;
    r[2] = feature_index;
    r[3] = (function << 4) | SW_ID;
    for (i, p) in params.iter().take(REPORT_LEN - 4).enumerate() {
        r[4 + i] = *p;
    }
    r
}

#[derive(Debug, PartialEq, Eq)]
pub enum Response<'a> {
    Ok(&'a [u8]),
    /// HID++ 2.0 (0xFF) or 1.0 (0x8F) error with code.
    Err(u8),
    NotOurs,
}

pub fn classify_response(
    buf: &[u8],
    dev_index: u8,
    feature_index: u8,
    function: u8,
) -> Response<'_> {
    if buf.len() < 7 || buf[1] != dev_index || (buf[0] != LONG_REPORT && buf[0] != 0x10) {
        return Response::NotOurs;
    }
    let fn_sw = (function << 4) | SW_ID;
    if (buf[2] == 0xFF || buf[2] == 0x8F) && buf[3] == feature_index && buf[4] == fn_sw {
        return Response::Err(buf[5]);
    }
    if buf[2] == feature_index && buf[3] == fn_sw {
        return Response::Ok(&buf[4..]);
    }
    Response::NotOurs
}

/// Unified Battery (0x1004) GetStatus response params.
pub fn parse_unified_battery(params: &[u8], supports_soc: bool) -> BatteryInfo {
    let soc = params.first().copied().unwrap_or(0);
    let status = params.get(2).copied().unwrap_or(0xFF);
    let charging = match status {
        0 => Charging::Discharging,
        1 => Charging::Charging,
        2 => Charging::ChargingSlow,
        3 => Charging::Full,
        4 => Charging::Error,
        _ => Charging::Unknown,
    };
    let percent = if supports_soc && soc <= 100 {
        Some(soc)
    } else {
        None
    };
    BatteryInfo {
        percent,
        charging,
        feature: "Unified Battery (0x1004)",
    }
}

/// Battery Status (0x1000) GetBatteryLevelStatus response params.
pub fn parse_battery_status(params: &[u8]) -> BatteryInfo {
    let level = params.first().copied().unwrap_or(0);
    let status = params.get(2).copied().unwrap_or(0xFF);
    let charging = match status {
        0 => Charging::Discharging,
        1 | 2 | 4 => Charging::Charging,
        3 => Charging::Full,
        5..=7 => Charging::Error,
        _ => Charging::Unknown,
    };
    BatteryInfo {
        percent: if level <= 100 { Some(level) } else { None },
        charging,
        feature: "Battery Status (0x1000)",
    }
}

// ---------- device I/O ----------

#[derive(Debug)]
#[allow(dead_code)]
enum ReqError {
    Timeout,
    Device(u8),
    Io(String),
}

impl std::fmt::Display for ReqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReqError::Timeout => write!(f, "timed out"),
            ReqError::Device(c) => write!(f, "device error 0x{c:02X}"),
            ReqError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

struct Channel {
    dev: HidDevice,
}

impl Channel {
    fn request(&self, idx: u8, feat: u8, func: u8, params: &[u8]) -> Result<Vec<u8>, ReqError> {
        // Drain stale input so we only see our answer.
        let mut scratch = [0u8; 64];
        while let Ok(n) = self.dev.read_timeout(&mut scratch, 0) {
            if n == 0 {
                break;
            }
        }
        let req = build_request(idx, feat, func, params);
        self.dev
            .write(&req)
            .map_err(|e| ReqError::Io(e.to_string()))?;
        let deadline = Instant::now() + IO_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(ReqError::Timeout);
            }
            let mut buf = [0u8; 64];
            let n = self
                .dev
                .read_timeout(&mut buf, left.as_millis() as i32)
                .map_err(|e| ReqError::Io(e.to_string()))?;
            if n == 0 {
                return Err(ReqError::Timeout);
            }
            match classify_response(&buf[..n], idx, feat, func) {
                Response::Ok(p) => return Ok(p.to_vec()),
                Response::Err(c) => return Err(ReqError::Device(c)),
                Response::NotOurs => continue,
            }
        }
    }

    fn feature_index(&self, idx: u8, feature: u16) -> Result<Option<u8>, ReqError> {
        let p = self.request(idx, 0x00, 0, &feature.to_be_bytes())?;
        Ok(match p.first() {
            Some(0) | None => None,
            Some(i) => Some(*i),
        })
    }

    fn device_name(&self, idx: u8) -> Option<String> {
        let fi = self.feature_index(idx, FEATURE_DEVICE_NAME).ok()??;
        let len = *self.request(idx, fi, 0, &[]).ok()?.first()? as usize;
        let mut name = Vec::new();
        while name.len() < len.min(64) {
            let chunk = self.request(idx, fi, 1, &[name.len() as u8]).ok()?;
            let take = (len - name.len()).min(chunk.len());
            if take == 0 {
                break;
            }
            name.extend_from_slice(&chunk[..take]);
        }
        let s = String::from_utf8_lossy(&name)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        (!s.is_empty()).then_some(s)
    }

    fn device_type(&self, idx: u8) -> Option<u8> {
        let fi = self.feature_index(idx, FEATURE_DEVICE_NAME).ok()??;
        self.request(idx, fi, 2, &[]).ok()?.first().copied()
    }

    fn read_battery(&self, idx: u8) -> Result<Option<BatteryInfo>, ReqError> {
        if let Some(fi) = self.feature_index(idx, FEATURE_UNIFIED_BATTERY)? {
            let caps = self.request(idx, fi, 0, &[])?;
            let supports_soc = caps.get(1).is_some_and(|f| f & 0x02 != 0);
            let st = self.request(idx, fi, 1, &[])?;
            return Ok(Some(parse_unified_battery(&st, supports_soc)));
        }
        if let Some(fi) = self.feature_index(idx, FEATURE_BATTERY_STATUS)? {
            let st = self.request(idx, fi, 0, &[])?;
            return Ok(Some(parse_battery_status(&st)));
        }
        Ok(None)
    }
}

const DEVICE_TYPE_MOUSE: u8 = 3;

/// Remembers where the mouse was last found so later polls skip the slow full scan.
pub type Hint = Option<(std::ffi::CString, u8)>;

fn describe(ch: &Channel, idx: u8, battery: BatteryInfo) -> (bool, Reading) {
    let is_mouse = ch.device_type(idx) == Some(DEVICE_TYPE_MOUSE);
    let name = ch
        .device_name(idx)
        .unwrap_or_else(|| "Logitech device".into());
    (
        is_mouse,
        Reading {
            device_name: name,
            battery,
            via_receiver: idx != 0xFF,
        },
    )
}

/// Fast path: query the slot that worked last time.
fn read_hinted(api: &HidApi, hint: &Hint) -> Option<Reading> {
    let (path, idx) = hint.as_ref()?;
    let ch = Channel {
        dev: api.open_path(path).ok()?,
    };
    let battery = ch.read_battery(*idx).ok()??;
    Some(describe(&ch, *idx, battery).1)
}

/// Scans all Logitech HID++ long-report interfaces and returns the best battery reading.
pub fn read_battery(api: &mut HidApi, hint: &mut Hint) -> Outcome {
    if let Err(e) = api.refresh_devices() {
        return Outcome::Error(format!("HID enumeration failed: {e}"));
    }
    if let Some(r) = read_hinted(api, hint) {
        return Outcome::Ok(r);
    }
    *hint = None;
    let infos: Vec<_> = api
        .device_list()
        .filter(|d| {
            d.vendor_id() == LOGITECH_VID && d.usage_page() == 0xFF00 && d.usage() == 0x0002
        })
        .map(|d| (d.path().to_owned(), d.product_id()))
        .collect();
    crate::diag::log(format!("{} HID++ interface(s)", infos.len()));
    if infos.is_empty() {
        return Outcome::NoReceiver;
    }

    let mut best: Option<(bool, Reading, (std::ffi::CString, u8))> = None;
    let mut last_err: Option<String> = None;
    'scan: for (path, pid) in infos {
        let dev = match api.open_path(&path) {
            Ok(d) => d,
            Err(e) => {
                last_err = Some(format!("Cannot open HID interface (PID {pid:04X}): {e}"));
                continue;
            }
        };
        let ch = Channel { dev };
        // 0xFF = the device itself (USB cable); 1..=6 = receiver slots.
        for idx in std::iter::once(0xFFu8).chain(1..=6) {
            // Ping through Root.GetProtocolVersion (function 1).
            let ping = ch.request(idx, 0x00, 1, &[0, 0, 0x5A]);
            crate::diag::log(format!("pid {pid:04X} idx {idx}: ping {ping:?}"));
            match ping {
                Ok(p) if p.first().is_some_and(|m| *m >= 2) => {}
                _ => continue,
            }
            let rb = ch.read_battery(idx);
            crate::diag::log(format!("idx {idx}: battery {rb:?}"));
            let battery = match rb {
                Ok(Some(b)) => b,
                Ok(None) => continue,
                Err(e) => {
                    last_err = Some(format!("Battery query failed: {e}"));
                    continue;
                }
            };
            let (is_mouse, reading) = describe(&ch, idx, battery);
            if best.as_ref().is_none_or(|(m, _, _)| !m && is_mouse) {
                best = Some((is_mouse, reading, (path.clone(), idx)));
            }
            if is_mouse {
                break 'scan;
            }
        }
    }
    match best {
        Some((_, r, h)) => {
            *hint = Some(h);
            Outcome::Ok(r)
        }
        None => match last_err {
            Some(e) => Outcome::Error(e),
            None => Outcome::DeviceOffline,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_layout() {
        let r = build_request(0x01, 0x05, 1, &[0xAA]);
        assert_eq!(&r[..5], &[0x11, 0x01, 0x05, 0x17, 0xAA]);
        assert_eq!(r.len(), 20);
    }

    #[test]
    fn classify_ok_and_error() {
        let mut ok = [0u8; 20];
        ok[..6].copy_from_slice(&[0x11, 1, 5, 0x17, 80, 0x08]);
        assert!(matches!(classify_response(&ok, 1, 5, 1), Response::Ok(p) if p[0] == 80));
        let mut err = [0u8; 20];
        err[..6].copy_from_slice(&[0x11, 1, 0xFF, 5, 0x17, 0x09]);
        assert_eq!(classify_response(&err, 1, 5, 1), Response::Err(9));
        let mut err1 = [0u8; 20];
        err1[..6].copy_from_slice(&[0x11, 1, 0x8F, 0, 0x17, 0x09]);
        assert_eq!(classify_response(&err1, 1, 0, 1), Response::Err(9));
        assert_eq!(classify_response(&ok, 2, 5, 1), Response::NotOurs);
        assert_eq!(classify_response(&ok[..4], 1, 5, 1), Response::NotOurs);
    }

    #[test]
    fn unified_battery() {
        let b = parse_unified_battery(&[73, 0x08, 1, 1], true);
        assert_eq!(b.percent, Some(73));
        assert_eq!(b.charging, Charging::Charging);
        let b = parse_unified_battery(&[0, 0x08, 0, 0], false);
        assert_eq!(b.percent, None);
        assert_eq!(b.charging, Charging::Discharging);
        assert_eq!(
            parse_unified_battery(&[100, 0, 3, 1], true).charging,
            Charging::Full
        );
        assert_eq!(
            parse_unified_battery(&[50, 0, 9, 0], true).charging,
            Charging::Unknown
        );
    }

    #[test]
    fn battery_status() {
        let b = parse_battery_status(&[55, 50, 0]);
        assert_eq!(b.percent, Some(55));
        assert_eq!(b.charging, Charging::Discharging);
        assert_eq!(parse_battery_status(&[90, 0, 3]).charging, Charging::Full);
        assert_eq!(parse_battery_status(&[90, 0, 6]).charging, Charging::Error);
    }
}

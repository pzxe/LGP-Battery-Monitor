//! Small Win32 helpers.

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, RECT};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SPI_GETWORKAREA, SW_SHOWNORMAL,
    SystemParametersInfoW,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub fn work_area() -> WorkArea {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut _ as *mut _, 0) };
    if ok == 0 {
        unsafe {
            return WorkArea {
                left: 0,
                top: 0,
                right: GetSystemMetrics(SM_CXSCREEN),
                bottom: GetSystemMetrics(SM_CYSCREEN),
            };
        }
    }
    WorkArea {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

/// Ask Windows 11 for rounded window corners (no-op on Windows 10).
pub fn round_window(title: &str) {
    let t = wide(title);
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), t.as_ptr());
        if !hwnd.is_null() {
            let pref: i32 = 2; // DWMWCP_ROUND
            DwmSetWindowAttribute(hwnd, 33, &pref as *const _ as *const _, 4);
        }
    }
}

pub fn open_url(url: &str) {
    if !url.starts_with("https://") {
        return;
    }
    let (op, u) = (wide("open"), wide(url));
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            u.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Returns true if this process is the first instance.
pub fn acquire_single_instance() -> bool {
    let name = wide("Local\\LGP-Battery-Monitor-Singleton");
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        // Intentionally leaked: held for the process lifetime.
        !h.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
    }
}

const SHOW_EVENT: &str = "Local\\LGP-Battery-Monitor-Show";

/// Second instance: ask the running one to show its popup.
pub fn signal_existing_instance() {
    let name = wide(SHOW_EVENT);
    unsafe {
        let h = CreateEventW(std::ptr::null(), 0, 0, name.as_ptr());
        if !h.is_null() {
            SetEvent(h);
            CloseHandle(h);
        }
    }
}

/// First instance: invoke `on_signal` whenever another instance asks us to show.
pub fn listen_for_show(on_signal: impl Fn() + Send + 'static) {
    let name = wide(SHOW_EVENT);
    let h = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) } as usize;
    if h == 0 {
        return;
    }
    std::thread::spawn(move || {
        loop {
            let r = unsafe { WaitForSingleObject(h as _, INFINITE) };
            if r != 0 {
                return;
            }
            on_signal();
        }
    });
}

/// Windows-subsystem builds have no console; attach to the parent's for CLI output.
pub fn console_writer() -> Box<dyn std::io::Write> {
    unsafe {
        // Redirected (pipe/file) output: use it as is.
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        if !h.is_null() && h as isize != -1 {
            return Box::new(std::io::stdout());
        }
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
    match std::fs::OpenOptions::new().write(true).open("CONOUT$") {
        Ok(f) => Box::new(f),
        Err(_) => Box::new(std::io::stdout()),
    }
}

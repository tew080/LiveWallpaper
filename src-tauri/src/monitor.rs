//! Polls Windows once a second for two things that should pause the wallpaper:
//! the desktop being completely hidden behind other windows, and the PC
//! running on battery. Emits `runtime:changed` only when something changes.

#![cfg(windows)]

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager};
use windows_sys::Win32::{
    Foundation::{BOOL, HWND, LPARAM, RECT},
    Graphics::{
        Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
    },
    System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
    UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GetClassNameW, GetWindowLongW, GetWindowRect, IsIconic,
        IsWindowVisible, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    },
};

use crate::settings::{AppState, RuntimeState};

/// Pixels of slack when comparing a window rect with a monitor work area
/// (maximized windows overhang their monitor by a few pixels).
const TOLERANCE: i32 = 8;

struct Ctx {
    mons: Vec<RECT>,
    covered: Vec<bool>,
}

unsafe extern "system" fn monitor_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let list = unsafe { &mut *(lparam as *mut Vec<RECT>) };
    let mut info: MONITORINFO = unsafe { zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(hmon, &mut info) } != 0 {
        list.push(info.rcWork); // work area = monitor minus taskbar
    }
    1
}

unsafe extern "system" fn window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam as *mut Ctx) };
    if ctx.covered.iter().all(|c| *c) {
        return 0; // nothing left to find
    }

    unsafe {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            return 1;
        }

        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & WS_EX_TOOLWINDOW != 0 || (ex & WS_EX_LAYERED != 0 && ex & WS_EX_TRANSPARENT != 0) {
            return 1; // tool windows and click-through overlays don't hide the desktop
        }

        // Windows on another virtual desktop / suspended UWP apps are "cloaked".
        let mut cloaked: u32 = 0;
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32, // windows-sys 0.59 declares the parameter as u32
            &mut cloaked as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return 1;
        }

        let mut cls = [0u16; 64];
        let n = GetClassNameW(hwnd, cls.as_mut_ptr(), cls.len() as i32);
        let class = String::from_utf16_lossy(&cls[..n.max(0) as usize]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return 1;
        }

        let mut r: RECT = zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
            return 1;
        }
        for (i, m) in ctx.mons.iter().enumerate() {
            if r.left <= m.left + TOLERANCE
                && r.top <= m.top + TOLERANCE
                && r.right >= m.right - TOLERANCE
                && r.bottom >= m.bottom - TOLERANCE
            {
                ctx.covered[i] = true;
            }
        }
    }
    1
}

/// True when every monitor has at least one window covering its work area.
fn desktop_covered() -> bool {
    let mut mons: Vec<RECT> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(monitor_proc),
            &mut mons as *mut Vec<RECT> as LPARAM,
        );
    }
    if mons.is_empty() {
        return false;
    }
    let mut ctx = Ctx {
        covered: vec![false; mons.len()],
        mons,
    };
    unsafe {
        EnumWindows(Some(window_proc), &mut ctx as *mut Ctx as LPARAM);
    }
    ctx.covered.iter().all(|c| *c)
}

fn on_battery() -> bool {
    let mut status: SYSTEM_POWER_STATUS = unsafe { zeroed() };
    // ACLineStatus: 0 = offline (battery), 1 = online, 255 = unknown
    unsafe { GetSystemPowerStatus(&mut status) != 0 && status.ACLineStatus == 0 }
}

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mut last = RuntimeState::default();
        loop {
            let check_cover = app
                .try_state::<AppState>()
                .map(|s| s.settings.lock().unwrap().pause_when_covered)
                .unwrap_or(false);

            let now = RuntimeState {
                covered: check_cover && desktop_covered(),
                on_battery: on_battery(),
            };

            if now != last {
                if let Some(state) = app.try_state::<AppState>() {
                    *state.runtime.lock().unwrap() = now.clone();
                }
                let _ = app.emit("runtime:changed", &now);
                last = now;
            }
            thread::sleep(Duration::from_millis(1000));
        }
    });
}

//! Embeds a window between the wallpaper and the desktop icons.
//!
//! Classic layout (Windows 7 .. 11 23H2):
//!   Progman
//!   WorkerW            <- has child SHELLDLL_DefView (the icons)
//!   WorkerW            <- sibling right after it: this is where we attach
//!
//! Windows 11 24H2+ keeps the WorkerW *inside* Progman, so we fall back to
//! parenting to Progman and pushing the window to the bottom of the z-order.

#![cfg(windows)]

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetSystemMetrics, HWND_BOTTOM, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SMTO_NORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW, SendMessageTimeoutW,
    SetParent, SetWindowPos,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn enum_proc(top: HWND, lparam: LPARAM) -> BOOL {
    let defview = wide("SHELLDLL_DefView");
    let has_icons = unsafe { FindWindowExW(top, null_mut(), defview.as_ptr(), null()) };
    if !has_icons.is_null() {
        let worker = wide("WorkerW");
        let target = unsafe { FindWindowExW(null_mut(), top, worker.as_ptr(), null()) };
        unsafe { *(lparam as *mut HWND) = target };
        return 0; // stop enumeration
    }
    1
}

/// `raw_hwnd` is the HWND of the Tauri window, passed as an integer.
pub fn attach(raw_hwnd: isize) -> Result<(), String> {
    let hwnd = raw_hwnd as HWND;

    unsafe {
        let progman_cls = wide("Progman");
        let progman = FindWindowW(progman_cls.as_ptr(), null());
        if progman.is_null() {
            return Err("Progman window not found".into());
        }

        // Ask Explorer to spawn the WorkerW behind the icons.
        let mut result: usize = 0;
        SendMessageTimeoutW(progman, 0x052C, 0xD, 0x1, SMTO_NORMAL, 1000, &mut result);

        let mut workerw: HWND = null_mut();
        EnumWindows(Some(enum_proc), &mut workerw as *mut HWND as LPARAM);

        let parent = if !workerw.is_null() {
            workerw
        } else {
            progman // Windows 11 24H2+ layout
        };

        if SetParent(hwnd, parent).is_null() {
            return Err("SetParent failed".into());
        }

        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    Ok(())
}

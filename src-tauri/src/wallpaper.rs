//! Embeds a window between the wallpaper and the desktop icons.
//!
//! Explorer's window layout differs between Windows versions, so `attach`
//! inspects it and picks a strategy:
//!
//! * classic (Windows 7 .. 11 23H2): a top-level `WorkerW` that is the sibling
//!   right after the `WorkerW` holding `SHELLDLL_DefView` (the icons).
//! * Windows 11 24H2+: `Progman` itself holds `SHELLDLL_DefView` and a child
//!   `WorkerW`. If that child sits *below* the icons we parent to it;
//!   otherwise we parent to `Progman` and place ourselves directly below the
//!   icons.
//!
//! Every decision is written to `wallpaper-attach.log` (app log dir) so a
//! wrong result can be diagnosed.

#![cfg(windows)]

use std::{
    fs,
    path::PathBuf,
    ptr::{null, null_mut},
    thread,
    time::Duration,
};

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetParent, GetSystemMetrics,
    GetWindowRect, HWND_BOTTOM, SMTO_NORMAL, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SendMessageTimeoutW, SetParent, SetWindowPos,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn class_of(hwnd: HWND) -> String {
    if hwnd.is_null() {
        return "<null>".into();
    }
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn describe(hwnd: HWND) -> String {
    format!("{}@{:p}", class_of(hwnd), hwnd)
}

fn rect_of(hwnd: HWND) -> String {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    if unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
        return "?".into();
    }
    format!("({},{})-({},{})", r.left, r.top, r.right, r.bottom)
}

/// Direct children of `parent`, topmost first.
fn children(parent: HWND) -> Vec<HWND> {
    let mut list = Vec::new();
    let mut prev: HWND = null_mut();
    loop {
        let next = unsafe { FindWindowExW(parent, prev, null(), null()) };
        if next.is_null() || list.len() >= 64 {
            break;
        }
        list.push(next);
        prev = next;
    }
    list
}

fn find_child(parent: HWND, class: &str) -> HWND {
    let cls = wide(class);
    unsafe { FindWindowExW(parent, null_mut(), cls.as_ptr(), null()) }
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

fn list(parent: HWND) -> String {
    children(parent)
        .iter()
        .map(|h| describe(*h))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `raw_hwnd` is the HWND of the Tauri window, passed as an integer.
/// Safe to call repeatedly.
pub fn attach(raw_hwnd: isize, log: &mut Vec<String>) -> Result<(), String> {
    let hwnd = raw_hwnd as HWND;

    unsafe {
        let progman_cls = wide("Progman");
        let progman = FindWindowW(progman_cls.as_ptr(), null());
        if progman.is_null() {
            return Err("Progman window not found (Explorer not ready yet?)".into());
        }

        // Ask Explorer to spawn the wallpaper WorkerW.
        let mut result: usize = 0;
        SendMessageTimeoutW(progman, 0x052C, 0xD, 0x1, SMTO_NORMAL, 1000, &mut result);

        let kids = children(progman);
        log.push(format!(
            "{} children, top to bottom: [{}]",
            describe(progman),
            list(progman)
        ));

        let mut classic: HWND = null_mut();
        EnumWindows(Some(enum_proc), &mut classic as *mut HWND as LPARAM);

        let defview = find_child(progman, "SHELLDLL_DefView");
        let child_worker = find_child(progman, "WorkerW");
        let pos = |h: HWND| kids.iter().position(|k| *k == h);

        // (parent, insert_after, description)
        let (parent, insert_after, mode) = if !classic.is_null() {
            (classic, HWND_BOTTOM, "classic WorkerW")
        } else {
            match (pos(defview), pos(child_worker)) {
                (Some(d), Some(w)) if w > d => (
                    child_worker,
                    HWND_BOTTOM,
                    "WorkerW child of Progman (below the icons)",
                ),
                (Some(_), _) => (progman, defview, "Progman, directly below the icons"),
                _ => (progman, HWND_BOTTOM, "Progman, bottom of z-order"),
            }
        };
        log.push(format!("mode: {mode}; parent: {}", describe(parent)));

        SetParent(hwnd, parent);
        let now = GetParent(hwnd);
        if now != parent {
            return Err(format!(
                "SetParent did not take effect (parent is {})",
                describe(now)
            ));
        }

        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );

        log.push(format!(
            "parent rect {}; wallpaper rect {}",
            rect_of(parent),
            rect_of(hwnd)
        ));
        log.push(format!("{} children now: [{}]", describe(progman), list(progman)));
    }
    Ok(())
}

/// Attaches on a background thread, retrying until Explorer is ready (it may
/// not be when the app starts at login). The window is shown only once it has
/// been attached, and the attachment is re-applied after showing because
/// showing a window can change its z-order.
pub fn attach_in_background(window: tauri::WebviewWindow, log_file: Option<PathBuf>) {
    let raw = match window.hwnd() {
        Ok(h) => h.0 as isize,
        Err(_) => return,
    };

    thread::spawn(move || {
        let mut log: Vec<String> = Vec::new();
        let mut attached = false;

        for attempt in 1..=120 {
            match attach(raw, &mut log) {
                Ok(()) => {
                    attached = true;
                    break;
                }
                Err(e) => {
                    log.push(format!("attempt {attempt}: {e}"));
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        if attached {
            let _ = window.show();
            thread::sleep(Duration::from_millis(800));
            log.push("--- re-applying after show ---".into());
            if let Err(e) = attach(raw, &mut log) {
                log.push(format!("re-attach failed: {e}"));
            }
        } else {
            log.push("gave up: wallpaper window was not shown".into());
        }

        if let Some(path) = log_file {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(path, log.join("\n"));
        }
    });
}

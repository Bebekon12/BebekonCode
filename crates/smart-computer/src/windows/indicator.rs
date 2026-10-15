//! A host-owned visible control surface survives a minimized/disconnected GUI.
use super::{security, win};
use crate::{
    engine::{self, Shared},
    Result,
};
use std::{cell::RefCell, ptr};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::*,
        UI::{Controls::BST_CHECKED, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};
thread_local! {static STATE:RefCell<Option<Shared>>=const{RefCell::new(None)};}
thread_local! {static DISPLAYED:RefCell<Option<String>>=const{RefCell::new(None)};}
const STOP: usize = 1;
const ALLOW: usize = 2;
const DENY: usize = 3;
const SENSITIVE: usize = 4;
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let handled = STATE.with(|slot| {
        let slot = slot.borrow();
        let Some(shared) = slot.as_ref() else {
            return false;
        };
        match message {
            WM_COMMAND => {
                let id = wparam.0 & 65535;
                match id {
                    STOP => engine::stop(shared),
                    ALLOW | DENY => {
                        let pending = shared.0.lock().ok().and_then(|s| s.status.pending.clone());
                        if let Some(pending) = pending {
                            let checked = unsafe {
                                SendMessageW(
                                    GetDlgItem(Some(window), SENSITIVE as i32).unwrap_or_default(),
                                    BM_GETCHECK,
                                    None,
                                    None,
                                )
                            }
                            .0 == BST_CHECKED.0 as isize;
                            if id == ALLOW
                                && (!pending.sensitive || checked)
                                && restore_target(shared).is_err()
                            {
                                let _ = engine::approve(shared, &pending.id, false, false);
                                return true;
                            }
                            let _ = engine::approve(shared, &pending.id, id == ALLOW, checked);
                        }
                    }
                    _ => {}
                }
                true
            }
            WM_HOTKEY | WM_CLOSE => {
                engine::stop(shared);
                true
            }
            WM_TIMER => {
                if let Ok(state) = shared.0.lock() {
                    if !security::alive(state.parent_pid) {
                        drop(state);
                        engine::stop(shared);
                        unsafe {
                            PostQuitMessage(0);
                        }
                        return true;
                    }
                    let visible = state.status.enabled
                        || state.status.executing
                        || state.status.pending.is_some();
                    unsafe {
                        let _ =
                            ShowWindow(window, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
                    }
                    let title = if let Some(p) = &state.status.pending {
                        format!("Smart Computer · Подтвердите: {}", p.description)
                    } else {
                        "Smart Computer · управление включено · Ctrl+Alt+Pause — стоп".into()
                    };
                    let title = security::wide(&title);
                    unsafe {
                        let _ = SetWindowTextW(window, PCWSTR(title.as_ptr()));
                    }
                    let pending = state.status.pending.is_some();
                    let pending_id = state.status.pending.as_ref().map(|p| p.id.clone());
                    DISPLAYED.with(|last| {
                        if *last.borrow() != pending_id {
                            *last.borrow_mut() = pending_id;
                            unsafe {
                                SendMessageW(
                                    GetDlgItem(Some(window), SENSITIVE as i32).unwrap_or_default(),
                                    BM_SETCHECK,
                                    Some(WPARAM(0)),
                                    None,
                                );
                            }
                        }
                    });
                    unsafe {
                        let _ = EnableWindow(
                            GetDlgItem(Some(window), ALLOW as i32).unwrap_or_default(),
                            pending,
                        );
                        let _ = EnableWindow(
                            GetDlgItem(Some(window), DENY as i32).unwrap_or_default(),
                            pending,
                        );
                    }
                }
                true
            }
            WM_DESTROY => {
                engine::stop(shared);
                unsafe {
                    PostQuitMessage(0);
                }
                true
            }
            _ => false,
        }
    });
    if handled {
        LRESULT(0)
    } else {
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }
}
pub fn restore_target(shared: &Shared) -> Result<()> {
    let state = shared.0.lock().map_err(|_| crate::Error::Busy)?;
    let pending = state.status.pending.as_ref().ok_or(crate::Error::Denied)?;
    let Some(id) = &pending.window else {
        return Ok(());
    };
    let window = state
        .status
        .windows
        .iter()
        .find(|w| &w.id == id)
        .ok_or(crate::Error::Denied)?;
    let raw = isize::from_str_radix(id.split('-').next().unwrap_or(""), 16)
        .map_err(|_| crate::Error::Denied)?;
    let h = HWND(raw as *mut _);
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(h, Some(&mut pid));
    }
    if pid != window.pid {
        return Err(crate::Error::Denied);
    }
    security::same_user(pid)?;
    if pending.action.is_some() && !unsafe { SetForegroundWindow(h) }.as_bool() {
        return Err(crate::Error::Denied);
    }
    Ok(())
}
pub fn run_indicator(shared: Shared) -> Result<()> {
    STATE.with(|s| *s.borrow_mut() = Some(shared.clone()));
    let class = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        lpszClassName: w!("BebekonSmartComputerIndicator"),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(crate::Error::Unavailable);
    }
    let window = win(unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("BebekonSmartComputerIndicator"),
            w!("Smart Computer"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            40,
            40,
            740,
            140,
            None,
            None,
            None,
            Some(ptr::null()),
        )
    })?;
    for (id, text, x, width, style) in [
        (STOP, "■ Стоп", 12, 100, BS_PUSHBUTTON),
        (ALLOW, "Разрешить один раз", 128, 180, BS_PUSHBUTTON),
        (DENY, "Отклонить", 324, 100, BS_PUSHBUTTON),
        (
            SENSITIVE,
            "Разрешаю изменение данных",
            440,
            274,
            BS_AUTOCHECKBOX,
        ),
    ] {
        let text = security::wide(text);
        win(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style as u32),
                x,
                26,
                width,
                32,
                Some(window),
                Some(HMENU(id as *mut _)),
                None,
                None,
            )
        })?;
    }
    if !unsafe {
        RegisterHotKey(
            Some(window),
            1,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            VK_PAUSE.0 as u32,
        )
    }
    .is_ok()
    {
        engine::stop(&shared);
        return Err(crate::Error::Unavailable);
    }
    if unsafe { SetTimer(Some(window), 1, 100, None) } == 0 {
        engine::stop(&shared);
        return Err(crate::Error::Unavailable);
    }
    {
        let mut state = shared.0.lock().map_err(|_| crate::Error::Busy)?;
        state.indicator_ready = true;
        state.status.available = state.desktop_ready;
    }
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    unsafe {
        let _ = UnregisterHotKey(Some(window), 1);
        let _ = DestroyWindow(window);
    }
    Ok(())
}

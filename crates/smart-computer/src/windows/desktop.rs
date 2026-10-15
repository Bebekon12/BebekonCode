use super::{security, win};
use crate::{
    engine::{Desktop, Shared},
    invalid,
    protocol::*,
    vision::Frame,
    Error, Result,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    mem::size_of,
    time::{Duration, Instant},
};
use windows::{
    core::{Interface, BSTR},
    Graphics::{
        Capture::*,
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        Imaging::{BitmapPixelFormat, SoftwareBitmap},
    },
    Media::Ocr::OcrEngine,
    Storage::Streams::DataWriter,
    Win32::{
        Foundation::*,
        Graphics::{Direct3D::*, Direct3D11::*, Dwm::*, Dxgi::IDXGIDevice},
        System::{
            Com::*,
            Threading::*,
            Variant::VARIANT,
            WinRT::{Direct3D11::*, Graphics::Capture::IGraphicsCaptureItemInterop},
        },
        UI::{Accessibility::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};

fn hwnd(w: &WindowInfo) -> Result<HWND> {
    let raw = w.id.split('-').next().ok_or(Error::Denied)?;
    let raw = isize::from_str_radix(raw, 16).map_err(|_| Error::Denied)?;
    Ok(HWND(raw as *mut _))
}
fn stamp(pid: u32) -> Result<u64> {
    let p = security::process(pid)?;
    let mut c = FILETIME::default();
    let mut e = FILETIME::default();
    let mut k = FILETIME::default();
    let mut u = FILETIME::default();
    win(unsafe { GetProcessTimes(p.0, &mut c, &mut e, &mut k, &mut u) })?;
    Ok((c.dwHighDateTime as u64) << 32 | c.dwLowDateTime as u64)
}
fn bounds(h: HWND) -> Result<Rect> {
    let mut r = RECT::default();
    win(unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut r as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    })?;
    let result = Rect {
        x: r.left,
        y: r.top,
        width: r.right.saturating_sub(r.left) as u32,
        height: r.bottom.saturating_sub(r.top) as u32,
    };
    result.validate()?;
    Ok(result)
}
fn sensitive(title: &str, app: &str) -> bool {
    let s = format!("{title} {app}").to_lowercase();
    [
        "credential",
        "credentials",
        "password",
        "парол",
        "sign in",
        "log in",
        "вход в",
        "1password",
        "bitwarden",
        "keepass",
        "lastpass",
        "uac",
        "windows security",
        "безопасность windows",
        "consent.exe",
        "winlogon.exe",
        "windowsterminal.exe",
        "powershell.exe",
        "pwsh.exe",
        "cmd.exe",
        "conhost.exe",
    ]
    .iter()
    .any(|v| s.contains(v))
}
fn info(h: HWND) -> Result<WindowInfo> {
    security::interactive_desktop()?;
    if !unsafe { IsWindow(Some(h)) }.as_bool()
        || !unsafe { IsWindowVisible(h) }.as_bool()
        || unsafe { IsIconic(h) }.as_bool()
    {
        return Err(Error::Unavailable);
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(h, Some(&mut pid));
    }
    security::same_user(pid)?;
    let path = security::path(pid)?;
    let application = path
        .file_name()
        .ok_or(Error::Denied)?
        .to_string_lossy()
        .to_string();
    if pid == std::process::id() || application.eq_ignore_ascii_case("bebekoncode.exe") {
        return Err(Error::Denied);
    }
    let mut title = vec![0u16; 512];
    let count = unsafe { GetWindowTextW(h, &mut title) };
    if count <= 0 {
        return Err(Error::Unavailable);
    }
    let title = String::from_utf16_lossy(&title[..count as usize]);
    if sensitive(&title, &application) {
        return Err(Error::Denied);
    }
    let mut cloaked = 0u32;
    win(unsafe { DwmGetWindowAttribute(h, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4) })?;
    if cloaked != 0 {
        return Err(Error::Denied);
    }
    Ok(WindowInfo {
        id: format!("{:x}-{:x}", h.0 as usize, stamp(pid)?),
        title,
        application,
        pid,
        bounds: bounds(h)?,
        dpi: unsafe { GetDpiForWindow(h) },
        focused: unsafe { GetForegroundWindow() } == h,
    })
}
pub fn window_choices() -> Result<Vec<WindowInfo>> {
    unsafe extern "system" fn collect(h: HWND, param: LPARAM) -> windows::core::BOOL {
        let result = unsafe { &mut *(param.0 as *mut Vec<WindowInfo>) };
        if result.len() < 128 {
            if let Ok(w) = info(h) {
                result.push(w);
            }
        }
        true.into()
    }
    let mut out = vec![];
    win(unsafe {
        EnumWindows(
            Some(collect),
            LPARAM((&mut out as *mut Vec<WindowInfo>) as isize),
        )
    })?;
    Ok(out)
}
fn shortened(s: impl AsRef<str>, n: usize) -> String {
    s.as_ref().chars().take(n).collect()
}
#[allow(non_upper_case_globals)] // Names are the official Windows constants.
fn type_name(t: UIA_CONTROLTYPE_ID) -> String {
    match t {
        UIA_ButtonControlTypeId => "button",
        UIA_EditControlTypeId => "edit",
        UIA_TextControlTypeId => "text",
        UIA_CheckBoxControlTypeId => "checkbox",
        UIA_ComboBoxControlTypeId => "combobox",
        UIA_MenuItemControlTypeId => "menu_item",
        UIA_ListItemControlTypeId => "list_item",
        UIA_TreeItemControlTypeId => "tree_item",
        UIA_TabItemControlTypeId => "tab_item",
        UIA_WindowControlTypeId => "window",
        UIA_PaneControlTypeId => "pane",
        UIA_SliderControlTypeId => "slider",
        _ => "other",
    }
    .into()
}

pub struct NativeDesktop {
    action_epoch: u64,
    uia: IUIAutomation,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt: IDirect3DDevice,
}
impl NativeDesktop {
    pub fn new() -> Result<Self> {
        win(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() })?;
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let uia: IUIAutomation =
            win(unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) })?;
        let uia2: IUIAutomation2 = win(uia.cast())?;
        win(unsafe { uia2.SetConnectionTimeout(500) })?;
        win(unsafe { uia2.SetTransactionTimeout(1000) })?;
        let mut device = None;
        let mut context = None;
        win(unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        })?;
        let device = device.ok_or(Error::Unavailable)?;
        let context = context.ok_or(Error::Unavailable)?;
        let dxgi: IDXGIDevice = win(device.cast())?;
        let winrt = win(unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) })?;
        let winrt = win(winrt.cast())?;
        Ok(Self {
            action_epoch: 0,
            uia,
            device,
            context,
            winrt,
        })
    }
    fn root(&self, w: &WindowInfo) -> Result<IUIAutomationElement> {
        win(unsafe { self.uia.ElementFromHandle(hwnd(w)?) })
    }
    fn element(&self, e: &IUIAutomationElement, w: &WindowInfo) -> Result<Element> {
        if win(unsafe { e.CurrentIsPassword() })?.as_bool()
            || win(unsafe { e.CurrentIsOffscreen() })?.as_bool()
        {
            return Err(Error::Denied);
        }
        if win(unsafe { e.CurrentProcessId() })? as u32 != w.pid {
            return Err(Error::Denied);
        }
        let r = win(unsafe { e.CurrentBoundingRectangle() })?;
        let bounds = Rect {
            x: r.left - w.bounds.x,
            y: r.top - w.bounds.y,
            width: r.right.saturating_sub(r.left) as u32,
            height: r.bottom.saturating_sub(r.top) as u32,
        };
        if bounds.width == 0 || bounds.height == 0 {
            return Err(Error::Unavailable);
        }
        let name = shortened(win(unsafe { e.CurrentName() })?.to_string(), 256);
        let automation_id = shortened(win(unsafe { e.CurrentAutomationId() })?.to_string(), 256);
        let control_type = type_name(win(unsafe { e.CurrentControlType() })?);
        let id = format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}|{}|{}|{}|{}|{}",
                    w.id, name, automation_id, control_type, bounds.x, bounds.y
                )
                .as_bytes()
            )
        );
        let mut patterns = vec![];
        unsafe {
            if e.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
                .is_ok()
            {
                patterns.push("invoke".into());
            }
            if e.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                .is_ok()
            {
                patterns.push("set_value".into());
            }
            if e.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                UIA_SelectionItemPatternId,
            )
            .is_ok()
            {
                patterns.push("select".into());
            }
            if e.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId)
                .is_ok()
            {
                patterns.push("toggle".into());
            }
            if e.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(
                UIA_ExpandCollapsePatternId,
            )
            .is_ok()
            {
                patterns.push("expand".into());
            }
        }
        let text = Self::element_text(e)?;
        Ok(Element {
            id,
            name,
            automation_id,
            control_type,
            bounds,
            enabled: win(unsafe { e.CurrentIsEnabled() })?.as_bool(),
            focused: win(unsafe { e.CurrentHasKeyboardFocus() })?.as_bool(),
            patterns,
            text,
        })
    }
    fn element_text(e: &IUIAutomationElement) -> Result<Option<String>> {
        if win(unsafe { e.CurrentIsPassword() })?.as_bool() {
            return Err(Error::Denied);
        }
        if let Ok(p) =
            unsafe { e.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
        {
            return Ok(Some(shortened(
                win(unsafe { p.CurrentValue() })?.to_string(),
                512,
            )));
        }
        if let Ok(p) =
            unsafe { e.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
        {
            let range = win(unsafe { p.DocumentRange() })?;
            return Ok(Some(shortened(
                win(unsafe { range.GetText(512) })?.to_string(),
                512,
            )));
        }
        Ok(None)
    }
    fn authorized(&self, w: &WindowInfo, shared: &Shared, focus: bool) -> Result<()> {
        {
            let state = shared.0.lock().map_err(|_| Error::Busy)?;
            if state.epoch != self.action_epoch
                || !state.status.enabled
                || state.status.stopped
                || state.read_only
            {
                return Err(Error::Cancelled);
            }
            if !state
                .status
                .windows
                .iter()
                .any(|selected| selected.id == w.id && selected.pid == w.pid)
            {
                return Err(Error::Denied);
            }
        }
        security::interactive_desktop()?;
        if focus {
            self.focused(w)?;
        }
        Ok(())
    }
    fn verified_effect(
        &self,
        w: &WindowInfo,
        shared: &Shared,
        effect: bool,
    ) -> Result<Option<bool>> {
        self.authorized(w, shared, true)?;
        Ok(Some(effect))
    }
    fn scan(
        &self,
        w: &WindowInfo,
        selector: &Selector,
        limit: usize,
    ) -> Result<Vec<(Element, IUIAutomationElement)>> {
        let root = self.root(w)?;
        let condition = win(unsafe { self.uia.CreateTrueCondition() })?;
        let walker = win(unsafe { self.uia.CreateTreeWalker(&condition) })?;
        let mut queue = VecDeque::from([root]);
        let mut result = vec![];
        let start = Instant::now();
        let mut visited = 0;
        while let Some(e) = queue.pop_front() {
            if win(unsafe { e.CurrentIsPassword() })?.as_bool() {
                continue;
            }
            visited += 1;
            if visited > 400 || start.elapsed() > Duration::from_millis(1500) {
                return Err(invalid(
                    "UIA: поиск ограничен; уточните цель или запросите изображение",
                ));
            }
            if let Ok(value) = self.element(&e, w) {
                let matches = selector
                    .name
                    .as_ref()
                    .is_none_or(|s| value.name.to_lowercase().contains(&s.to_lowercase()))
                    && selector
                        .automation_id
                        .as_ref()
                        .is_none_or(|s| &value.automation_id == s)
                    && selector
                        .control_type
                        .as_ref()
                        .is_none_or(|s| &value.control_type == s)
                    && selector.element_id.as_ref().is_none_or(|s| &value.id == s);
                if matches {
                    result.push((value, e.clone()));
                    if result.len() >= limit {
                        break;
                    }
                }
            }
            if let Ok(mut child) = unsafe { walker.GetFirstChildElement(&e) } {
                for _ in 0..80 {
                    if queue.len() >= 400 {
                        break;
                    }
                    queue.push_back(child.clone());
                    match unsafe { walker.GetNextSiblingElement(&child) } {
                        Ok(next) => child = next,
                        Err(_) => break,
                    }
                }
            }
        }
        Ok(result)
    }
    fn target(&self, w: &WindowInfo, selector: &Selector) -> Result<IUIAutomationElement> {
        let mut values = self.scan(w, selector, 2)?;
        if values.len() != 1 {
            return Err(invalid("Элемент не найден однозначно; обновите наблюдение"));
        }
        let (info, element) = values.remove(0);
        if !info.enabled {
            return Err(Error::Denied);
        }
        Ok(element)
    }
    fn focused(&self, w: &WindowInfo) -> Result<()> {
        if unsafe { GetForegroundWindow() } != hwnd(w)? {
            return Err(invalid("Фокус изменился; ввод остановлен"));
        }
        let e = win(unsafe { self.uia.GetFocusedElement() })?;
        if win(unsafe { e.CurrentIsPassword() })?.as_bool()
            || win(unsafe { e.CurrentProcessId() })? as u32 != w.pid
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    fn running(shared: &Shared) -> Result<()> {
        let state = shared.0.lock().map_err(|_| Error::Busy)?;
        if !state.status.enabled || state.status.stopped {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    fn input(&self, w: &WindowInfo, shared: &Shared, inputs: &[INPUT]) -> Result<()> {
        Self::running(shared)?;
        self.focused(w)?;
        let guard = shared.0.lock().map_err(|_| Error::Busy)?;
        if guard.epoch != self.action_epoch
            || !guard.status.enabled
            || guard.status.stopped
            || guard.read_only
            || !guard
                .status
                .windows
                .iter()
                .any(|allowed| allowed.id == w.id)
        {
            return Err(Error::Cancelled);
        }
        if unsafe { GetForegroundWindow() } != hwnd(w)? {
            return Err(invalid("Фокус изменился; ввод остановлен"));
        }
        if unsafe { SendInput(inputs, size_of::<INPUT>() as i32) } != inputs.len() as u32 {
            return Err(Error::Denied);
        }
        drop(guard);
        Self::running(shared)?;
        Ok(())
    }
    fn point(&self, w: &WindowInfo, x: i32, y: i32) -> Result<INPUT> {
        if x < 0 || y < 0 || x as u32 >= w.bounds.width || y as u32 >= w.bounds.height {
            return Err(invalid("Точка вне выбранного окна"));
        }
        let point = POINT {
            x: w.bounds.x + x,
            y: w.bounds.y + y,
        };
        let hit = unsafe { WindowFromPoint(point) };
        let root = unsafe { GetAncestor(hit, GA_ROOT) };
        if root != hwnd(w)? {
            return Err(invalid("Выбранное окно перекрыто другим окном"));
        }
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        if width <= 1 || height <= 1 {
            return Err(Error::Unavailable);
        }
        Ok(mouse(
            ((point.x - left) as i64 * 65535 / (width - 1) as i64) as i32,
            ((point.y - top) as i64 * 65535 / (height - 1) as i64) as i32,
            0,
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        ))
    }
    fn capture_raw(&self, w: &WindowInfo) -> Result<Frame> {
        if !win(GraphicsCaptureSession::IsSupported())? {
            return Err(Error::Unavailable);
        }
        let factory: IGraphicsCaptureItemInterop = win(windows::core::factory::<
            GraphicsCaptureItem,
            IGraphicsCaptureItemInterop,
        >())?;
        let item: GraphicsCaptureItem = win(unsafe { factory.CreateForWindow(hwnd(w)?) })?;
        self.capture_item(item)
    }
    fn capture_item(&self, item: GraphicsCaptureItem) -> Result<Frame> {
        let size = win(item.Size())?;
        if size.Width <= 0
            || size.Height <= 0
            || size.Width as u64 * size.Height as u64 > MAX_FRAME_PIXELS
        {
            return Err(Error::Unavailable);
        }
        let pool = win(Direct3D11CaptureFramePool::CreateFreeThreaded(
            &self.winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            size,
        ))?;
        let session = win(pool.CreateCaptureSession(&item))?;
        let result = (|| {
            win(session.StartCapture())?;
            let until = Instant::now() + Duration::from_secs(2);
            let frame = loop {
                match pool.TryGetNextFrame() {
                    Ok(frame) => break frame,
                    Err(_) => {
                        if Instant::now() >= until {
                            return Err(Error::Cancelled);
                        }
                        std::thread::sleep(Duration::from_millis(15));
                    }
                }
            };
            let surface = win(frame.Surface())?;
            let access: IDirect3DDxgiInterfaceAccess = win(surface.cast())?;
            let texture: ID3D11Texture2D = win(unsafe { access.GetInterface() })?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe {
                texture.GetDesc(&mut desc);
            }
            let content = win(frame.ContentSize())?;
            let width = content.Width as u32;
            let height = content.Height as u32;
            if width > desc.Width
                || height > desc.Height
                || width as u64 * height as u64 > MAX_FRAME_PIXELS
            {
                return Err(Error::Unavailable);
            }
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            win(unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut staging)) })?;
            let staging = staging.ok_or(Error::Unavailable)?;
            unsafe {
                self.context.CopyResource(&staging, &texture);
            }
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            win(unsafe {
                self.context
                    .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            })?;
            let mut rgba = vec![0u8; width as usize * height as usize * 4];
            for y in 0..height as usize {
                let row = unsafe {
                    std::slice::from_raw_parts(
                        (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                        width as usize * 4,
                    )
                };
                for x in 0..width as usize {
                    let i = (y * width as usize + x) * 4;
                    rgba[i..i + 4].copy_from_slice(&[
                        row[x * 4 + 2],
                        row[x * 4 + 1],
                        row[x * 4],
                        255,
                    ]);
                }
            }
            unsafe {
                self.context.Unmap(&staging, 0);
            }
            win(frame.Close())?;
            Ok(Frame {
                width,
                height,
                rgba,
            })
        })();
        let _ = session.Close();
        let _ = pool.Close();
        result
    }
    fn redact(&self, w: &WindowInfo, frame: &mut Frame) -> Result<()> {
        let root = self.root(w)?;
        let value = VARIANT::from(true);
        let condition = win(unsafe {
            self.uia
                .CreatePropertyCondition(UIA_IsPasswordPropertyId, &value)
        })?;
        let elements = win(unsafe { root.FindAll(TreeScope_Subtree, &condition) })?;
        let length = win(unsafe { elements.Length() })?;
        if length > 64 {
            return Err(Error::Denied);
        }
        for i in 0..length {
            let e = win(unsafe { elements.GetElement(i) })?;
            let r = win(unsafe { e.CurrentBoundingRectangle() })?;
            let l = (r.left - w.bounds.x - 8).max(0) as u32;
            let t = (r.top - w.bounds.y - 8).max(0) as u32;
            let right = (r.right - w.bounds.x + 8).max(0) as u32;
            let bottom = (r.bottom - w.bounds.y + 8).max(0) as u32;
            for y in t.min(frame.height)..bottom.min(frame.height) {
                for x in l.min(frame.width)..right.min(frame.width) {
                    let j = (y as usize * frame.width as usize + x as usize) * 4;
                    frame.rgba[j..j + 4].copy_from_slice(&[32, 32, 32, 255]);
                }
            }
        }
        Ok(())
    }
}
fn mouse(dx: i32, dy: i32, data: u32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
fn mask(frame: &mut Frame, r: RECT, x: i32, y: i32) {
    let left = (i64::from(r.left) - i64::from(x)).clamp(0, i64::from(frame.width)) as u32;
    let top = (i64::from(r.top) - i64::from(y)).clamp(0, i64::from(frame.height)) as u32;
    let right = (i64::from(r.right) - i64::from(x)).clamp(0, i64::from(frame.width)) as u32;
    let bottom = (i64::from(r.bottom) - i64::from(y)).clamp(0, i64::from(frame.height)) as u32;
    for row in top..bottom {
        for col in left..right {
            let j = (row as usize * frame.width as usize + col as usize) * 4;
            frame.rgba[j..j + 4].copy_from_slice(&[32, 32, 32, 255]);
        }
    }
}
fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
fn virtual_key(s: &str) -> Result<VIRTUAL_KEY> {
    Ok(match s.to_lowercase().as_str() {
        "ctrl" | "control" => VK_CONTROL,
        "alt" => VK_MENU,
        "shift" => VK_SHIFT,
        "enter" | "return" => VK_RETURN,
        "tab" => VK_TAB,
        "escape" | "esc" => VK_ESCAPE,
        "backspace" => VK_BACK,
        "delete" => VK_DELETE,
        "space" => VK_SPACE,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "up" => VK_UP,
        "down" => VK_DOWN,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" => VK_PRIOR,
        "pagedown" => VK_NEXT,
        _ if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
            VIRTUAL_KEY(s.as_bytes()[0].to_ascii_uppercase() as u16)
        }
        _ => return Err(invalid("Клавиша недоступна; системные сочетания запрещены")),
    })
}

impl Desktop for NativeDesktop {
    fn validate_window(&mut self, w: &WindowInfo) -> Result<WindowInfo> {
        let current = info(hwnd(w)?)?;
        if current.id != w.id || current.pid != w.pid {
            return Err(Error::Denied);
        }
        Ok(current)
    }
    fn elements(
        &mut self,
        w: &WindowInfo,
        selector: &Selector,
        limit: usize,
    ) -> Result<Vec<Element>> {
        let w = self.validate_window(w)?;
        Ok(self
            .scan(&w, selector, limit)?
            .into_iter()
            .map(|(v, _)| v)
            .collect())
    }
    fn capture(&mut self, w: &WindowInfo) -> Result<Frame> {
        let w = self.validate_window(w)?;
        let mut frame = self.capture_raw(&w)?;
        self.redact(&w, &mut frame)?;
        security::interactive_desktop()?;
        Ok(frame)
    }
    fn capture_screen(&mut self, w: &WindowInfo) -> Result<Frame> {
        use windows::Win32::Graphics::Gdi::*;
        let w = self.validate_window(w)?;
        let handle = hwnd(&w)?;
        let monitor = unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONEAREST) };
        let mut metadata = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut metadata) }.as_bool() {
            return Err(Error::Unavailable);
        }
        let r = metadata.rcMonitor;
        let roi = Rect {
            x: w.bounds.x - r.left,
            y: w.bounds.y - r.top,
            width: w.bounds.width,
            height: w.bounds.height,
        };
        if roi.x < 0
            || roi.y < 0
            || i64::from(roi.x) + i64::from(roi.width) > i64::from(r.right - r.left)
            || i64::from(roi.y) + i64::from(roi.height) > i64::from(r.bottom - r.top)
        {
            return Err(invalid(
                "Окно пересекает несколько мониторов; используйте surface=window",
            ));
        }
        let factory: IGraphicsCaptureItemInterop = win(windows::core::factory::<
            GraphicsCaptureItem,
            IGraphicsCaptureItemInterop,
        >())?;
        let item: GraphicsCaptureItem = win(unsafe { factory.CreateForMonitor(monitor) })?;
        let mut frame = self.capture_item(item)?.crop(Some(roi))?;
        // Screen mode never returns pixels belonging to another application above the target.
        struct Occlusion {
            target: HWND,
            rects: Vec<RECT>,
            found: bool,
        }
        unsafe extern "system" fn collect(h: HWND, param: LPARAM) -> windows::core::BOOL {
            let state = unsafe { &mut *(param.0 as *mut Occlusion) };
            if h == state.target {
                state.found = true;
                return false.into();
            }
            if state.rects.len() >= 128 {
                return false.into();
            }
            if unsafe { IsWindowVisible(h) }.as_bool() {
                let mut rect = RECT::default();
                if unsafe { GetWindowRect(h, &mut rect) }.is_ok() {
                    state.rects.push(rect);
                }
            }
            true.into()
        }
        let mut occlusion = Occlusion {
            target: handle,
            rects: vec![],
            found: false,
        };
        let _ = unsafe {
            EnumWindows(
                Some(collect),
                LPARAM((&mut occlusion as *mut Occlusion) as isize),
            )
        };
        if !occlusion.found {
            return Err(Error::Denied);
        }
        for rect in occlusion.rects {
            mask(&mut frame, rect, w.bounds.x, w.bounds.y);
        }
        self.redact(&w, &mut frame)?;
        security::interactive_desktop()?;
        Ok(frame)
    }
    fn ocr(&mut self, frame: &Frame) -> Result<Value> {
        let max = win(OcrEngine::MaxImageDimension())?;
        if frame.width > max || frame.height > max {
            return Err(invalid("OCR: выберите меньшую область окна"));
        }
        let writer = win(DataWriter::new())?;
        win(writer.WriteBytes(&frame.rgba))?;
        let bitmap = win(SoftwareBitmap::CreateCopyFromBuffer(
            &win(writer.DetachBuffer())?,
            BitmapPixelFormat::Rgba8,
            frame.width as i32,
            frame.height as i32,
        ))?;
        let engine = win(OcrEngine::TryCreateFromUserProfileLanguages())?;
        let operation = win(engine.RecognizeAsync(&bitmap))?;
        let until = Instant::now() + Duration::from_secs(2);
        while win(operation.Status())?.0 == 0 {
            if Instant::now() >= until {
                let _ = operation.Cancel();
                return Err(Error::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        let result = win(operation.GetResults())?;
        let mut lines = vec![];
        for line in win(result.Lines())?.into_iter().take(80) {
            lines.push(shortened(win(line.Text())?.to_string(), 256));
        }
        let text = lines.join("\n");
        Ok(
            json!({"lines":lines,"text":shortened(text,MAX_TEXT),"local":true,"language":win(engine.RecognizerLanguage())?.LanguageTag().map(|s|s.to_string()).unwrap_or_default()}),
        )
    }
    fn act(
        &mut self,
        w: &WindowInfo,
        action: &Action,
        shared: &Shared,
        epoch: u64,
    ) -> Result<Option<bool>> {
        self.action_epoch = epoch;
        let w = self.validate_window(w)?;
        Self::running(shared)?;
        if let Action::Window { operation } = action {
            let h = hwnd(&w)?;
            self.authorized(&w, shared, false)?;
            match operation {
                WindowAction::Activate => {
                    if !unsafe { SetForegroundWindow(h) }.as_bool() {
                        return Err(Error::Denied);
                    }
                    return Ok(Some(unsafe { GetForegroundWindow() } == h));
                }
                WindowAction::Minimize => {
                    unsafe {
                        let _ = ShowWindow(h, SW_MINIMIZE);
                    }
                    return Ok(Some(unsafe { IsIconic(h) }.as_bool()));
                }
                WindowAction::Maximize => {
                    unsafe {
                        let _ = ShowWindow(h, SW_MAXIMIZE);
                    }
                    return Ok(Some(unsafe { IsZoomed(h) }.as_bool()));
                }
                WindowAction::Restore => {
                    unsafe {
                        let _ = ShowWindow(h, SW_RESTORE);
                    }
                    return Ok(Some(
                        !unsafe { IsIconic(h) }.as_bool() && !unsafe { IsZoomed(h) }.as_bool(),
                    ));
                }
                WindowAction::Close => {
                    win(unsafe { PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0)) })?;
                    return Ok(None);
                }
            }
        }
        self.focused(&w)?;
        match action {
            Action::Click { x, y, right } => {
                self.input(&w, shared, &[self.point(&w, *x, *y)?])?;
                let (down, up) = if *right {
                    (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
                } else {
                    (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
                };
                self.input(&w, shared, &[mouse(0, 0, 0, down), mouse(0, 0, 0, up)])?;
            }
            Action::DoubleClick { x, y } => {
                self.input(&w, shared, &[self.point(&w, *x, *y)?])?;
                self.input(
                    &w,
                    shared,
                    &[
                        mouse(0, 0, 0, MOUSEEVENTF_LEFTDOWN),
                        mouse(0, 0, 0, MOUSEEVENTF_LEFTUP),
                        mouse(0, 0, 0, MOUSEEVENTF_LEFTDOWN),
                        mouse(0, 0, 0, MOUSEEVENTF_LEFTUP),
                    ],
                )?;
            }
            Action::Move { x, y } => {
                self.input(&w, shared, &[self.point(&w, *x, *y)?])?;
            }
            Action::Scroll { x, y, dx, dy } => {
                self.input(&w, shared, &[self.point(&w, *x, *y)?])?;
                let mut inputs = vec![];
                if *dy != 0 {
                    inputs.push(mouse(0, 0, *dy as u32, MOUSEEVENTF_WHEEL));
                }
                if *dx != 0 {
                    inputs.push(mouse(0, 0, *dx as u32, MOUSEEVENTF_HWHEEL));
                }
                self.input(&w, shared, &inputs)?;
            }
            Action::Drag { points } => {
                for [x, y] in points {
                    self.point(&w, *x, *y)?;
                }
                self.input(
                    &w,
                    shared,
                    &[
                        self.point(&w, points[0][0], points[0][1])?,
                        mouse(0, 0, 0, MOUSEEVENTF_LEFTDOWN),
                    ],
                )?;
                let result: Result<()> = (|| {
                    for [x, y] in &points[1..] {
                        self.input(&w, shared, &[self.point(&w, *x, *y)?])?;
                        std::thread::sleep(Duration::from_millis(12));
                    }
                    Ok(())
                })();
                unsafe {
                    SendInput(
                        &[mouse(0, 0, 0, MOUSEEVENTF_LEFTUP)],
                        size_of::<INPUT>() as i32,
                    );
                };
                result?;
            }
            Action::Keypress { keys } => {
                let keys: Vec<_> = keys.iter().map(|k| virtual_key(k)).collect::<Result<_>>()?;
                if keys.contains(&VK_MENU) && keys.contains(&VK_TAB)
                    || keys.contains(&VK_CONTROL)
                        && keys.contains(&VK_MENU)
                        && keys.contains(&VK_DELETE)
                {
                    return Err(Error::Denied);
                }
                let inputs: Vec<_> = keys
                    .iter()
                    .map(|k| key(*k, 0, KEYBD_EVENT_FLAGS(0)))
                    .chain(keys.iter().rev().map(|k| key(*k, 0, KEYEVENTF_KEYUP)))
                    .collect();
                self.input(&w, shared, &inputs)?;
            }
            Action::Type { text } => {
                for unit in text.encode_utf16() {
                    self.input(
                        &w,
                        shared,
                        &[
                            key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE),
                            key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                        ],
                    )?;
                }
            }
            Action::Invoke { selector } => {
                let e = self.target(&w, selector)?;
                let p: IUIAutomationInvokePattern =
                    win(unsafe { e.GetCurrentPatternAs(UIA_InvokePatternId) })?;
                self.authorized(&w, shared, true)?;
                win(unsafe { p.Invoke() })?;
            }
            Action::SetValue { selector, text } => {
                let e = self.target(&w, selector)?;
                let p: IUIAutomationValuePattern =
                    win(unsafe { e.GetCurrentPatternAs(UIA_ValuePatternId) })?;
                self.authorized(&w, shared, true)?;
                win(unsafe { p.SetValue(&BSTR::from(text)) })?;
                return self.verified_effect(
                    &w,
                    shared,
                    win(unsafe { p.CurrentValue() })? == text.as_str(),
                );
            }
            Action::Select { selector } => {
                let e = self.target(&w, selector)?;
                let p: IUIAutomationSelectionItemPattern =
                    win(unsafe { e.GetCurrentPatternAs(UIA_SelectionItemPatternId) })?;
                self.authorized(&w, shared, true)?;
                win(unsafe { p.Select() })?;
                return self.verified_effect(
                    &w,
                    shared,
                    win(unsafe { p.CurrentIsSelected() })?.as_bool(),
                );
            }
            Action::Toggle { selector } => {
                let e = self.target(&w, selector)?;
                let p: IUIAutomationTogglePattern =
                    win(unsafe { e.GetCurrentPatternAs(UIA_TogglePatternId) })?;
                let before = win(unsafe { p.CurrentToggleState() })?;
                self.authorized(&w, shared, true)?;
                win(unsafe { p.Toggle() })?;
                return self.verified_effect(
                    &w,
                    shared,
                    win(unsafe { p.CurrentToggleState() })? != before,
                );
            }
            Action::Expand { selector, expanded } => {
                let e = self.target(&w, selector)?;
                let p: IUIAutomationExpandCollapsePattern =
                    win(unsafe { e.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) })?;
                self.authorized(&w, shared, true)?;
                if *expanded {
                    win(unsafe { p.Expand() })?;
                } else {
                    win(unsafe { p.Collapse() })?;
                }
                return self.verified_effect(
                    &w,
                    shared,
                    win(unsafe { p.CurrentExpandCollapseState() })?
                        == if *expanded {
                            ExpandCollapseState_Expanded
                        } else {
                            ExpandCollapseState_Collapsed
                        },
                );
            }
            Action::Unity { command } => {
                self.authorized(&w, shared, true)?;
                let verified = super::unity::act(&w, command, shared)?;
                Self::running(shared)?;
                self.focused(&w)?;
                return self.verified_effect(&w, shared, verified);
            }
            Action::Window { .. } => unreachable!(),
        }
        Self::running(shared)?;
        self.focused(&w)?;
        Ok(None)
    }
    fn condition(&mut self, w: &WindowInfo, condition: &Condition) -> Result<bool> {
        if matches!(condition, Condition::WindowClosed) {
            return Ok(
                info(hwnd(w)?).map_or(true, |current| current.id != w.id || current.pid != w.pid)
            );
        }
        let w = self.validate_window(w)?;
        match condition {
            Condition::WindowFocused => Ok(unsafe { GetForegroundWindow() } == hwnd(&w)?),
            Condition::ElementExists { selector } => Ok(!self.scan(&w, selector, 1)?.is_empty()),
            Condition::ElementAbsent { selector } => Ok(self.scan(&w, selector, 1)?.is_empty()),
            Condition::ElementFocused { selector } => Ok(self
                .scan(&w, selector, 1)?
                .first()
                .is_some_and(|(e, _)| e.focused)),
            Condition::TextContains { selector, text } => {
                let e = self.target(&w, selector)?;
                let value =
                    Self::element_text(&e)?.unwrap_or(win(unsafe { e.CurrentName() })?.to_string());
                Ok(value.contains(text))
            }
            _ => Err(invalid("Условие недоступно для UIA")),
        }
    }
    fn adapter_observe(
        &mut self,
        w: &WindowInfo,
        goal: Option<&str>,
        limit: usize,
        view: Option<AdapterView>,
        visual: bool,
        shared: &Shared,
    ) -> Result<Option<ToolResult>> {
        super::unity::observe(w, goal, limit, view, visual, shared)
    }
}

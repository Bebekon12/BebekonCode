use crate::{
    invalid,
    protocol::*,
    vision::{changes, Frame, FrameCache},
    Error, Result,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Default)]
pub struct HostState {
    pub desktop_ready: bool,
    pub indicator_ready: bool,
    pub status: Status,
    pub parent_pid: u32,
    pub agent_pid: u32,
    pub owner_account: String,
    pub read_only: bool,
    pub epoch: u64,
    pub decision: Option<(String, bool, bool)>,
}
pub type Shared = Arc<(Mutex<HostState>, Condvar)>;
pub fn shared(parent_pid: u32) -> Shared {
    Arc::new((
        Mutex::new(HostState {
            parent_pid,
            status: Status {
                available: cfg!(windows),
                detail:
                    "Локальный Desktop Host. Каждое наблюдение и действие требует подтверждения."
                        .into(),
                ..Status::default()
            },
            ..HostState::default()
        }),
        Condvar::new(),
    ))
}
pub fn stop(shared: &Shared) {
    if let Ok(mut state) = shared.0.lock() {
        state.status.stopped = true;
        state.status.enabled = false;
        state.status.pending = None;
        state.epoch += 1;
        state.decision = None;
    }
    shared.1.notify_all();
}
pub fn configure(
    shared: &Shared,
    account: String,
    session: String,
    windows: Vec<WindowInfo>,
    read_only: bool,
) -> Result<()> {
    if windows.is_empty() || windows.len() > 8 || account.len() > 64 || session.len() > 64 {
        return Err(invalid("Выберите от 1 до 8 окон и чат"));
    }
    let mut state = shared.0.lock().map_err(|_| Error::Busy)?;
    if state.status.enabled && state.status.owner_session.as_ref() != Some(&session) {
        return Err(Error::Busy);
    }
    state.owner_account = account;
    state.status.owner_session = Some(session);
    state.status.windows = windows;
    state.read_only = read_only;
    state.status.enabled = true;
    state.status.stopped = false;
    state.status.pending = None;
    state.decision = None;
    state.epoch += 1;
    shared.1.notify_all();
    Ok(())
}
pub fn approve(shared: &Shared, id: &str, allow: bool, sensitive: bool) -> Result<()> {
    let mut state = shared.0.lock().map_err(|_| Error::Busy)?;
    let pending = state.status.pending.as_ref().ok_or(Error::Denied)?;
    if pending.id != id || pending.sensitive && allow && !sensitive {
        return Err(Error::Denied);
    }
    state.decision = Some((id.into(), allow, sensitive));
    shared.1.notify_all();
    Ok(())
}

pub trait Desktop {
    fn validate_window(&mut self, window: &WindowInfo) -> Result<WindowInfo>;
    fn elements(
        &mut self,
        window: &WindowInfo,
        selector: &Selector,
        limit: usize,
    ) -> Result<Vec<Element>>;
    fn capture(&mut self, window: &WindowInfo) -> Result<Frame>;
    fn capture_screen(&mut self, _window: &WindowInfo) -> Result<Frame> {
        Err(Error::Unavailable)
    }
    fn ocr(&mut self, frame: &Frame) -> Result<Value>;
    fn act(
        &mut self,
        window: &WindowInfo,
        action: &Action,
        shared: &Shared,
        epoch: u64,
    ) -> Result<Option<bool>>;
    fn condition(&mut self, window: &WindowInfo, condition: &Condition) -> Result<bool>;
    fn adapter_observe(
        &mut self,
        _window: &WindowInfo,
        _goal: Option<&str>,
        _limit: usize,
        _view: Option<AdapterView>,
        _visual: bool,
        _shared: &Shared,
    ) -> Result<Option<ToolResult>> {
        Ok(None)
    }
}

pub struct Engine<D: Desktop> {
    pub desktop: D,
    pub shared: Shared,
    frames: FrameCache,
    audit: VecDeque<Audit>,
    frame_epoch: u64,
}
impl<D: Desktop> Engine<D> {
    pub fn new(desktop: D, shared: Shared) -> Self {
        Self {
            desktop,
            shared,
            frames: FrameCache::default(),
            audit: VecDeque::new(),
            frame_epoch: 0,
        }
    }
    fn lease(&self, account: &str, session: &str) -> Result<u64> {
        let state = self.shared.0.lock().map_err(|_| Error::Busy)?;
        if !state.status.enabled || state.status.stopped {
            return Err(Error::Unavailable);
        }
        if state.owner_account != account || state.status.owner_session.as_deref() != Some(session)
        {
            return Err(Error::Denied);
        }
        Ok(state.epoch)
    }
    fn valid_epoch(&self, epoch: u64) -> Result<()> {
        let state = self.shared.0.lock().map_err(|_| Error::Busy)?;
        if state.epoch != epoch || state.status.stopped || !state.status.enabled {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
    fn selected_metadata(&self, id: &str) -> Result<WindowInfo> {
        let window = self
            .shared
            .0
            .lock()
            .map_err(|_| Error::Busy)?
            .status
            .windows
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or(Error::Denied)?;
        Ok(window)
    }
    fn selected(&mut self, id: &str) -> Result<WindowInfo> {
        let window = self.selected_metadata(id)?;
        self.desktop.validate_window(&window)
    }
    fn permission(
        &self,
        account: &str,
        session: &str,
        window: Option<&str>,
        tool: &str,
        action: Option<&Action>,
        epoch: u64,
    ) -> Result<()> {
        self.valid_epoch(epoch)?;
        let (lock, cv) = &*self.shared;
        let mut state = lock.lock().map_err(|_| Error::Busy)?;
        if action.is_some() && state.read_only {
            return Err(Error::Denied);
        }
        let sensitive = action.is_some_and(Action::sensitive);
        let id = uuid::Uuid::new_v4().to_string();
        let description = if let Some(action) = action {
            match action{
            Action::Type{text}|Action::SetValue{text,..}=>format!("Ввод {} символов в выбранное окно. Содержимое доступно только в запросе агента.",text.chars().count()),
            Action::Click{x,y,..}|Action::DoubleClick{x,y}|Action::Move{x,y}=>format!("{} в выбранном окне: ({x}, {y}).",action.kind()),
            Action::Keypress{keys}=>format!("Клавиши: {}.",keys.join(" + ")),
            _=>format!("Действие {} в выбранном окне.",action.kind()),
        }
        } else {
            match tool{"computer_capture"=>"Получить кадр выбранного окна и передать изображение Codex.".into(),"computer_observe"=>"Прочитать выбранное окно: адаптер, UIA, при необходимости локальный OCR или изображение.".into(),_=>"Прочитать состояние выбранного окна.".into()}
        };
        let target = window
            .and_then(|id| state.status.windows.iter().find(|w| w.id == id))
            .map(|w| format!("{} · {}", w.application, w.title))
            .unwrap_or_else(|| "Выбранные приложения".into());
        let description = format!("{target}: {description}");
        state.status.pending = Some(Pending {
            id: id.clone(),
            account: account.into(),
            session: session.into(),
            window: window.map(str::to_string),
            tool: tool.into(),
            action: action.map(|a| a.kind().into()),
            sensitive,
            created_ms: now_ms(),
            description,
        });
        state.decision = None;
        let until = Instant::now() + Duration::from_secs(120);
        loop {
            if state.epoch != epoch || state.status.stopped || !state.status.enabled {
                state.status.pending = None;
                return Err(Error::Cancelled);
            }
            if let Some((decision_id, allow, extra)) = state.decision.take() {
                if decision_id == id {
                    state.status.pending = None;
                    if !allow || sensitive && !extra {
                        state.status.metrics.denied += 1;
                        return Err(Error::Denied);
                    }
                    return Ok(());
                }
            }
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                state.status.pending = None;
                return Err(Error::Cancelled);
            }
            state = cv
                .wait_timeout(state, remaining.min(Duration::from_millis(250)))
                .map_err(|_| Error::Busy)?
                .0;
        }
    }
    pub fn call(&mut self, account: &str, session: &str, request: Request) -> Result<ToolResult> {
        self.call_bound(account, session, request, None)
    }
    pub fn call_bound(
        &mut self,
        account: &str,
        session: &str,
        request: Request,
        expected_epoch: Option<u64>,
    ) -> Result<ToolResult> {
        request.validate()?;
        let start = Instant::now();
        let tool = request.name();
        if matches!(request, Request::ComputerStatus) {
            self.metric(|m| m.tools += 1);
            let state = self.shared.0.lock().map_err(|_| Error::Busy)?;
            return Ok(ToolResult {
                data: json!({"available":state.status.available,"enabled":state.status.enabled,"stopped":state.status.stopped,"controller":state.status.owner_session,"metrics":state.status.metrics,"requires_approval":"every_observation_and_action","coordinates":"window_relative_physical_pixels","sources":["application_adapter","uia","local_ocr","image"],"detail":state.status.detail}),
                image: None,
            });
        }
        let epoch = self.lease(account, session)?;
        if expected_epoch.is_some_and(|expected| expected != epoch) {
            return Err(Error::Cancelled);
        }
        if self.frame_epoch != epoch {
            self.frames.clear();
            self.frame_epoch = epoch;
        }
        self.shared
            .0
            .lock()
            .map_err(|_| Error::Busy)?
            .status
            .metrics
            .tools += 1;
        let answer = self.dispatch(account, session, request, epoch);
        let elapsed = start.elapsed().as_millis() as u64;
        {
            let mut state = self.shared.0.lock().map_err(|_| Error::Busy)?;
            state.status.metrics.latency_ms += elapsed;
            if let Ok(result) = &answer {
                state.status.metrics.context_bytes += result.data.to_string().len() as u64;
                if let Some(image) = &result.image {
                    state.status.metrics.images_sent += 1;
                    state.status.metrics.image_bytes += image.len() as u64;
                }
            } else {
                state.status.metrics.failures += 1;
            }
        }
        self.record(
            tool,
            None,
            if answer.is_ok() {
                "completed"
            } else {
                "failed"
            },
            "not_requested",
            elapsed,
        );
        answer
    }
    fn record(
        &mut self,
        tool: &str,
        action: Option<&str>,
        execution: &str,
        verification: &str,
        latency_ms: u64,
    ) {
        self.audit.push_back(Audit {
            at_ms: now_ms(),
            tool: tool.into(),
            action: action.map(str::to_string),
            execution: execution.into(),
            verification: verification.into(),
            latency_ms,
        });
        while self.audit.len() > 128 {
            self.audit.pop_front();
        }
        if let Ok(mut state) = self.shared.0.lock() {
            state.status.audit = self.audit.iter().cloned().collect();
        }
    }
    fn dispatch(
        &mut self,
        account: &str,
        session: &str,
        request: Request,
        epoch: u64,
    ) -> Result<ToolResult> {
        let tool = request.name();
        let id = request.window().map(str::to_string);
        if !matches!(
            request,
            Request::ComputerAct { .. } | Request::ComputerSequence { .. }
        ) {
            self.permission(account, session, id.as_deref(), tool, None, epoch)?;
        }
        self.valid_epoch(epoch)?;
        let data = match request {
            Request::ComputerWindows => {
                let windows = self
                    .shared
                    .0
                    .lock()
                    .map_err(|_| Error::Busy)?
                    .status
                    .windows
                    .clone();
                let mut actual = vec![];
                for window in windows {
                    if let Ok(window) = self.desktop.validate_window(&window) {
                        actual.push(window);
                    }
                }
                json!({"windows":actual,"only_selected":true})
            }
            Request::ComputerFind {
                window,
                selector,
                limit,
            } => {
                let window = self.selected(&window)?;
                let elements = self.desktop.elements(&window, &selector, limit)?;
                self.metric(|m| m.uia_calls += 1);
                json!({"source":"uia","elements":elements,"limit":limit})
            }
            Request::ComputerObserve {
                adapter_view,
                window,
                visual,
                roi,
                goal,
                selector,
                limit,
            } => {
                let window = self.selected(&window)?;
                if !visual || adapter_view.is_some() {
                    if let Some(result) = self.desktop.adapter_observe(
                        &window,
                        goal.as_deref(),
                        limit,
                        adapter_view,
                        visual,
                        &self.shared,
                    )? {
                        self.valid_epoch(epoch)?;
                        if result.image.is_some() {
                            self.metric(|m| m.screenshots += 1);
                        }
                        return Ok(result);
                    }
                    if adapter_view.is_some() {
                        return Err(invalid(
                            "Application Adapter недоступен для выбранного окна",
                        ));
                    }
                }
                if visual {
                    return self.picture(&window, roi, 1280, None, epoch, Surface::Window);
                }
                let elements = self.desktop.elements(&window, &selector, limit)?;
                self.metric(|m| m.uia_calls += 1);
                if elements.iter().any(|e| {
                    !matches!(e.control_type.as_str(), "window" | "pane")
                        && (!e.name.is_empty() || !e.patterns.is_empty())
                }) {
                    json!({"source":"uia","window":window.id,"elements":elements,"limit":limit,"graphics":"Use visual=true or computer_capture for graphical content."})
                } else {
                    let frame = self.desktop.capture(&window)?.crop(roi)?;
                    self.valid_epoch(epoch)?;
                    self.metric(|m| {
                        m.screenshots += 1;
                        m.ocr_calls += 1;
                    });
                    let text = self.desktop.ocr(&frame)?;
                    json!({"source":"ocr","window":window.id,"result":text,"graphics":"OCR is text only. Request an image if text is insufficient."})
                }
            }
            Request::ComputerCapture {
                surface,
                window,
                roi,
                max_edge,
                if_changed,
            } => {
                let window = self.selected(&window)?;
                return self.picture(
                    &window,
                    roi,
                    max_edge,
                    if_changed.as_deref(),
                    epoch,
                    surface,
                );
            }
            Request::ComputerAct {
                window,
                action,
                verify,
            } => {
                let window = self.selected(&window)?;
                self.step(account, session, &window, Step { action, verify }, epoch)?
            }
            Request::ComputerSequence { window, steps } => {
                let mut results = vec![];
                for (index, step) in steps.into_iter().enumerate() {
                    let window = self.selected(&window)?;
                    let result = self.step(account, session, &window, step, epoch);
                    match result {
                        Ok(value) => {
                            let halt = value["execution"] != "completed"
                                || value["verification"] == "failed";
                            results.push(json!({"step":index+1,"result":value}));
                            if halt {
                                break;
                            }
                        }
                        Err(error) => {
                            results.push(json!({"step":index+1,"result":{"execution":"failed","verification":"not_performed","error":error.to_string()}}));
                            break;
                        }
                    }
                }
                json!({"steps":results,"no_automatic_retry":true})
            }
            Request::ComputerWait {
                window,
                condition,
                timeout_ms,
            } => {
                let until = Instant::now() + Duration::from_millis(timeout_ms);
                let mut met = false;
                let window = if matches!(condition, Condition::WindowClosed) {
                    self.selected_metadata(&window)?
                } else {
                    self.selected(&window)?
                };
                loop {
                    self.valid_epoch(epoch)?;
                    if matches!(condition, Condition::ImageChanged { .. }) {
                        while Instant::now() < until {
                            self.valid_epoch(epoch)?;
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        self.permission(
                            account,
                            session,
                            Some(&window.id),
                            "computer_capture",
                            None,
                            epoch,
                        )?;
                        met = self.check(&window, &condition)?;
                        break;
                    }
                    if self.check(&window, &condition)? {
                        met = true;
                        break;
                    }
                    if Instant::now() >= until {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(150));
                }
                json!({"execution":"completed","verification":if met{"verified"}else{"failed"},"condition_met":met})
            }
            Request::ComputerStatus => unreachable!(),
        };
        self.valid_epoch(epoch)?;
        Ok(ToolResult { data, image: None })
    }
    fn metric(&self, change: impl FnOnce(&mut Metrics)) {
        if let Ok(mut state) = self.shared.0.lock() {
            change(&mut state.status.metrics);
        }
    }
    fn step(
        &mut self,
        account: &str,
        session: &str,
        window: &WindowInfo,
        step: Step,
        epoch: u64,
    ) -> Result<Value> {
        let start = Instant::now();
        self.permission(
            account,
            session,
            Some(&window.id),
            "computer_act",
            Some(&step.action),
            epoch,
        )?;
        self.valid_epoch(epoch)?;
        let window = self.desktop.validate_window(window)?;
        let effect = self
            .desktop
            .act(&window, &step.action, &self.shared, epoch)?;
        self.valid_epoch(epoch)?;
        self.frames.invalidate_reuse();
        self.metric(|m| m.actions += 1);
        let verification = if let Some(condition) = step.verify {
            if matches!(condition, Condition::ImageChanged { .. }) {
                self.permission(
                    account,
                    session,
                    Some(&window.id),
                    "computer_capture",
                    None,
                    epoch,
                )?;
            }
            if self.check(&window, &condition)? {
                "verified"
            } else {
                "failed"
            }
        } else {
            match effect {
                Some(true) => "verified",
                Some(false) => "failed",
                None => "not_requested",
            }
        };
        if verification == "verified" {
            self.metric(|m| m.verified += 1);
        }
        self.record(
            "computer_act",
            Some(step.action.kind()),
            "completed",
            verification,
            start.elapsed().as_millis() as u64,
        );
        Ok(json!({"execution":"completed","verification":verification,"action":step.action.kind()}))
    }
    fn check(&mut self, window: &WindowInfo, condition: &Condition) -> Result<bool> {
        if let Condition::ImageChanged { frame_id } = condition {
            let Some(before) = self.frames.get(&window.id, frame_id).cloned() else {
                return Err(invalid("Нет актуального исходного кадра"));
            };
            let frame = self.desktop.capture(window)?;
            self.metric(|m| m.screenshots += 1);
            return Ok(!changes(&before, &frame).is_empty());
        }
        self.metric(|m| m.uia_calls += 1);
        self.desktop.condition(window, condition)
    }
    fn picture(
        &mut self,
        window: &WindowInfo,
        roi: Option<Rect>,
        max_edge: u32,
        previous: Option<&str>,
        epoch: u64,
        surface: Surface,
    ) -> Result<ToolResult> {
        let frame = if let Some(cached) = self.frames.latest(
            &window.id,
            if previous.is_none() && matches!(surface, Surface::Window) {
                200
            } else {
                0
            },
        ) {
            self.metric(|m| m.cache_hits += 1);
            cached.frame.clone()
        } else {
            let frame = match surface {
                Surface::Window => self.desktop.capture(window)?,
                Surface::Screen => self.desktop.capture_screen(window)?,
            };
            self.metric(|m| m.screenshots += 1);
            frame
        };
        self.valid_epoch(epoch)?;
        let regions = previous
            .and_then(|id| self.frames.get(&window.id, id))
            .map(|before| changes(before, &frame));
        let id = self.frames.insert(&window.id, frame.clone());
        if regions.as_ref().is_some_and(Vec::is_empty) {
            return Ok(ToolResult {
                data: json!({"source":"image","changed":false,"frame_id":id,"image_sent":false}),
                image: None,
            });
        }
        let image = frame.crop(roi)?;
        let (png, width, height) = image.png(max_edge)?;
        self.valid_epoch(epoch)?;
        Ok(ToolResult {
            data: json!({"source":"image","frame_id":id,"changed":true,"surface":surface,"change_regions":regions,"width":width,"height":height,"original_width":frame.width,"original_height":frame.height,"roi":roi,"scale_x":image.width as f64/width as f64,"scale_y":image.height as f64/height as f64,"coordinate_space":"window_relative_physical_pixels"}),
            image: Some(STANDARD.encode(png)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        actions: usize,
    }
    impl Desktop for Fixture {
        fn validate_window(&mut self, w: &WindowInfo) -> Result<WindowInfo> {
            Ok(w.clone())
        }
        fn elements(&mut self, _: &WindowInfo, _: &Selector, _: usize) -> Result<Vec<Element>> {
            Ok(vec![])
        }
        fn capture(&mut self, _: &WindowInfo) -> Result<Frame> {
            Ok(Frame {
                width: 10,
                height: 10,
                rgba: vec![0; 400],
            })
        }
        fn ocr(&mut self, _: &Frame) -> Result<Value> {
            Ok(json!({"text":"test"}))
        }
        fn act(&mut self, _: &WindowInfo, _: &Action, _: &Shared, _: u64) -> Result<Option<bool>> {
            self.actions += 1;
            Ok(None)
        }
        fn condition(&mut self, _: &WindowInfo, _: &Condition) -> Result<bool> {
            Ok(false)
        }
    }
    fn test_engine() -> Engine<Fixture> {
        let state = shared(1);
        configure(
            &state,
            "a".into(),
            "s".into(),
            vec![WindowInfo {
                id: "w".into(),
                title: "fixture".into(),
                application: "test".into(),
                pid: 1,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                },
                dpi: 96,
                focused: true,
            }],
            false,
        )
        .unwrap();
        Engine::new(Fixture { actions: 0 }, state)
    }
    #[test]
    fn lease_and_readonly_are_enforced() {
        let mut engine = test_engine();
        assert!(engine.call("other", "s", Request::ComputerWindows).is_err());
        let windows = engine.shared.0.lock().unwrap().status.windows.clone();
        assert!(configure(&engine.shared, "a".into(), "other".into(), windows, false).is_err());
        engine.shared.0.lock().unwrap().read_only = true;
        let request = Request::ComputerAct {
            window: "w".into(),
            action: Action::Type {
                text: "not logged".into(),
            },
            verify: None,
        };
        assert!(engine.call("a", "s", request).is_err());
        assert_eq!(engine.desktop.actions, 0);
        assert!(engine
            .shared
            .0
            .lock()
            .unwrap()
            .status
            .audit
            .iter()
            .all(|a| !serde_json::to_string(a).unwrap().contains("not logged")));
    }
    #[test]
    fn stop_revokes_pending_permission() {
        let mut engine = test_engine();
        let shared = engine.shared.clone();
        let worker = std::thread::spawn(move || {
            engine.call(
                "a",
                "s",
                Request::ComputerCapture {
                    surface: Surface::Window,
                    window: "w".into(),
                    roi: None,
                    max_edge: 256,
                    if_changed: None,
                },
            )
        });
        for _ in 0..100 {
            if shared.0.lock().unwrap().status.pending.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        stop(&shared);
        assert!(worker.join().unwrap().is_err());
        assert_eq!(shared.0.lock().unwrap().status.metrics.screenshots, 0);
    }
    #[test]
    fn every_sequence_step_requires_new_permission_and_verification_stops_failure() {
        let mut engine = test_engine();
        let shared = engine.shared.clone();
        let worker = std::thread::spawn(move || {
            let answer = engine
                .call(
                    "a",
                    "s",
                    Request::ComputerSequence {
                        window: "w".into(),
                        steps: vec![
                            Step {
                                action: Action::Type { text: "one".into() },
                                verify: None,
                            },
                            Step {
                                action: Action::Type { text: "two".into() },
                                verify: Some(Condition::WindowFocused),
                            },
                            Step {
                                action: Action::Type {
                                    text: "three".into(),
                                },
                                verify: None,
                            },
                        ],
                    },
                )
                .unwrap();
            (answer, engine.desktop.actions)
        });
        let mut ids = vec![];
        for _ in 0..500 {
            let pending = shared.0.lock().unwrap().status.pending.clone();
            if let Some(pending) = pending {
                if !ids.contains(&pending.id) {
                    assert!(approve(&shared, &pending.id, true, false).is_err());
                    approve(&shared, &pending.id, true, true).unwrap();
                    ids.push(pending.id);
                }
            }
            if worker.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(3));
        }
        let (answer, count) = worker.join().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(count, 2);
        assert_eq!(answer.data["steps"][1]["result"]["verification"], "failed");
    }
}

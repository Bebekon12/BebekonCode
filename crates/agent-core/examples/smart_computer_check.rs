//! Reproducible native validation. Default run never reads a window or sends input.
//! --read-once / --capture-once / --set-value-once consume distinct explicit test consents.
//! --codex-status DATABASE ACCOUNT uses the application's existing official subscription login.
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows 11 is required");
}

#[cfg(windows)]
fn main() {
    if let Some(code) = agent_core::computer::entry() {
        std::process::exit(code);
    }
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|v| v == "--fixture-agent") {
        use std::io::Read;
        let mut line = vec![];
        loop {
            let mut b = [0u8];
            if std::io::stdin().read_exact(&mut b).is_err() {
                return;
            }
            if b[0] == b'\n' {
                break;
            }
            line.push(b[0]);
            if line.len() > 4096 {
                return;
            }
        }
        let proxy_args: Vec<String> = serde_json::from_slice(&line).expect("test configuration");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(proxy_args)
            .status()
            .unwrap();
        std::process::exit(status.code().unwrap_or(1));
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    if let Err(error) = runtime.block_on(native::run(&args)) {
        eprintln!("Smart Computer validation failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod native {
    use serde_json::{json, Value};
    use smart_computer::{manager::Manager, protocol::Pending};
    use std::{sync::Arc, time::Duration};
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        process::Command,
    };
    use windows::{
        core::{w, PCWSTR},
        Win32::{
            Foundation::*,
            Graphics::Gdi::*,
            UI::{HiDpi::*, WindowsAndMessaging::*},
        },
    };
    type TestResult<T> = Result<T, Box<dyn std::error::Error>>;
    unsafe extern "system" fn procedure(h: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if message == WM_PAINT {
            let mut paint = PAINTSTRUCT::default();
            let dc = unsafe { BeginPaint(h, &mut paint) };
            let blue = unsafe { CreateSolidBrush(COLORREF(0xff5500)) };
            let red = unsafe { CreateSolidBrush(COLORREF(0x3333ee)) };
            unsafe {
                FillRect(
                    dc,
                    &RECT {
                        left: 35,
                        top: 120,
                        right: 250,
                        bottom: 230,
                    },
                    blue,
                );
                let old = SelectObject(dc, HGDIOBJ(red.0));
                let _ = Ellipse(dc, 350, 130, 455, 235);
                SelectObject(dc, old);
                let _ = DeleteObject(HGDIOBJ(blue.0));
                let _ = DeleteObject(HGDIOBJ(red.0));
                let _ = EndPaint(h, &paint);
            }
            return LRESULT(0);
        }
        if message == WM_DESTROY {
            unsafe { PostQuitMessage(0) };
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(h, message, wp, lp) }
    }
    fn fixture() -> TestResult<usize> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = (|| -> windows::core::Result<usize> {
                unsafe {
                    let _ =
                        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                    let class = WNDCLASSW {
                        lpfnWndProc: Some(procedure),
                        lpszClassName: w!("BebekonSmartComputerFixture"),
                        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
                        ..Default::default()
                    };
                    RegisterClassW(&class);
                    let h = CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        class.lpszClassName,
                        w!("Smart Computer Fixture"),
                        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                        90,
                        120,
                        660,
                        390,
                        None,
                        None,
                        None,
                        None,
                    )?;
                    CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        w!("STATIC"),
                        w!("LOCAL OCR 1234"),
                        WS_CHILD | WS_VISIBLE,
                        35,
                        30,
                        400,
                        35,
                        Some(h),
                        None,
                        None,
                        None,
                    )?;
                    CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        w!("STATIC"),
                        w!("Input"),
                        WS_CHILD | WS_VISIBLE,
                        35,
                        280,
                        60,
                        28,
                        Some(h),
                        None,
                        None,
                        None,
                    )?;
                    CreateWindowExW(
                        WS_EX_CLIENTEDGE,
                        w!("EDIT"),
                        PCWSTR::null(),
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                        100,
                        275,
                        390,
                        30,
                        Some(h),
                        Some(HMENU(1000usize as *mut _)),
                        None,
                        None,
                    )?;
                    Ok(h.0 as usize)
                }
            })();
            let ok = result.is_ok();
            let _ = send.send(result.map_err(|e| e.to_string()));
            if ok {
                let mut message = MSG::default();
                unsafe {
                    while GetMessageW(&mut message, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
        });
        receive.recv()?.map_err(Into::into)
    }
    struct FixtureWindow(usize);
    impl Drop for FixtureWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = PostMessageW(Some(HWND(self.0 as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
    struct Mcp {
        child: tokio::process::Child,
        input: tokio::process::ChildStdin,
        output: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    }
    impl Mcp {
        async fn start(manager: &Manager) -> TestResult<Self> {
            let mut child = Command::new(std::env::current_exe()?)
                .arg("--fixture-agent")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()?;
            let config = manager
                .config(
                    "fixture",
                    "fixture",
                    child.id().ok_or("agent process missing")?,
                )
                .await?
                .ok_or("no MCP configuration")?;
            let mut input = child.stdin.take().unwrap();
            let output = BufReader::new(child.stdout.take().unwrap()).lines();
            input
                .write_all(format!("{}\n", config["args"]).as_bytes())
                .await?;
            let mut mcp = Self {
                child,
                input,
                output,
            };
            mcp.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"native-validation","version":"1"}}})).await?;
            let initialized = mcp.next().await?;
            assert_eq!(
                initialized["result"]["serverInfo"]["name"],
                "smart-computer"
            );
            Ok(mcp)
        }
        async fn send(&mut self, value: Value) -> TestResult<()> {
            self.input
                .write_all(format!("{value}\n").as_bytes())
                .await?;
            Ok(())
        }
        async fn next(&mut self) -> TestResult<Value> {
            let line = tokio::time::timeout(Duration::from_secs(130), self.output.next_line())
                .await??
                .ok_or("MCP process exited")?;
            Ok(serde_json::from_str(&line)?)
        }
        async fn call(&mut self, id: u32, name: &str, args: Value) -> TestResult<()> {
            self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await
        }
        async fn finish(mut self) -> TestResult<()> {
            drop(self.input);
            let status = tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await??;
            if !status.success() {
                return Err("MCP shutdown failed".into());
            }
            Ok(())
        }
    }
    async fn pending(manager: &Manager) -> TestResult<Pending> {
        for _ in 0..200 {
            if let Some(p) = manager.status().await?.pending {
                return Ok(p);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Err("Host did not request confirmation".into())
    }
    async fn select(manager: &Manager) -> TestResult<String> {
        for _ in 0..100 {
            let choices = manager.choices().await?;
            if manager.status().await?.available {
                if let Some(window) = choices
                    .into_iter()
                    .find(|w| w.title == "Smart Computer Fixture" && w.pid == std::process::id())
                {
                    return Ok(window.id);
                }
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        Err("Fixture or native Desktop Host unavailable".into())
    }
    pub async fn run(args: &[String]) -> TestResult<()> {
        let _window = FixtureWindow(fixture()?);
        let manager = Arc::new(Manager::new(std::env::current_exe()?));
        let result = protocol(&manager, args).await;
        manager.shutdown_async().await;
        result?;
        if let Some(index) = args
            .iter()
            .position(|v| v == "--codex-status" || v == "--codex-image-once")
        {
            let database = args.get(index + 1).ok_or("DATABASE required")?;
            let account = args.get(index + 2).ok_or("ACCOUNT required")?;
            codex(database, account, args[index] == "--codex-image-once").await?;
        }
        Ok(())
    }
    async fn protocol(manager: &Manager, args: &[String]) -> TestResult<()> {
        let window = select(manager).await?;
        manager
            .configure(
                "fixture".into(),
                "fixture".into(),
                vec![window.clone()],
                false,
            )
            .await?;
        let mut mcp = Mcp::start(manager).await?;
        mcp.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
            .await?;
        assert_eq!(
            mcp.next().await?["result"]["tools"]
                .as_array()
                .ok_or("missing tools")?
                .len(),
            8
        );
        mcp.call(3, "computer_status", json!({})).await?;
        assert_eq!(
            mcp.next().await?["result"]["structuredContent"]["enabled"],
            true
        );
        mcp.call(4, "computer_capture", json!({"window":window}))
            .await?;
        let p = pending(manager).await?;
        manager.approve(p.id, false, false).await?;
        assert_eq!(
            mcp.next().await?["result"]["structuredContent"]["execution"],
            "denied"
        );
        assert_eq!(manager.status().await?.metrics.screenshots, 0);
        println!(
            "PASS: native stdio initialization, 8 tools, protected IPC, denial without capture"
        );
        for (flag, tool, arguments) in [
            (
                "--read-once",
                "computer_observe",
                json!({"window":window,"limit":12}),
            ),
            (
                "--capture-once",
                "computer_capture",
                json!({"window":window,"max_edge":640}),
            ),
            (
                "--set-value-once",
                "computer_act",
                json!({"window":window,"action":{"kind":"set_value","selector":{"automation_id":"1000"},"text":"MCP test 123"},"verify":{"kind":"text_contains","selector":{"automation_id":"1000"},"text":"MCP test 123"}}),
            ),
        ] {
            if !args.iter().any(|v| v == flag) {
                continue;
            }
            mcp.call(5, tool, arguments).await?;
            let p = pending(manager).await?;
            if p.window.as_deref() != Some(&window) || p.tool != tool {
                return Err("unexpected permission target".into());
            }
            manager.approve(p.id, true, p.sensitive).await?;
            let answer = mcp.next().await?;
            if answer["result"]["isError"] == true {
                return Err(answer["result"]["structuredContent"].to_string().into());
            }
            if tool == "computer_capture" {
                assert_eq!(answer["result"]["content"][1]["type"], "image");
                assert_eq!(answer["result"]["content"][1]["mimeType"], "image/png");
            }
            if tool == "computer_act" {
                assert_eq!(
                    answer["result"]["structuredContent"]["verification"],
                    "verified"
                );
            }
            println!("PASS: {tool}, one explicit fixture consent");
        }
        mcp.call(6, "computer_capture", json!({"window":window}))
            .await?;
        let _ = pending(manager).await?;
        mcp.send(
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":6}}),
        )
        .await?;
        assert_eq!(
            mcp.next().await?["result"]["structuredContent"]["execution"],
            "stopped"
        );
        for _ in 0..100 {
            if manager.status().await?.stopped {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(manager.status().await?.stopped);
        println!(
            "PASS: MCP cancellation revokes Host control; metrics {}",
            serde_json::to_string(&manager.status().await?.metrics)?
        );
        mcp.finish().await
    }
    async fn codex(database: &str, account: &str, image: bool) -> TestResult<()> {
        use agent_core::{
            model::{AgentConfig, CreateChat, EventPayload, ToolPolicy},
            Core,
        };
        let core = Core::open(std::path::Path::new(database)).await?;
        let result=async{
            let models=core.account_models(account).await?;
            let model=models.iter().find(|m|m.is_default).or_else(||models.first()).ok_or("No model for this explicitly selected account")?;
            let chat=core.create_chat(CreateChat{workspace_id:None,mode:"single".into(),agents:vec![AgentConfig{provider:"openai".into(),account_profile_id:account.into(),model:model.id.clone(),reasoning_effort:None,fast_mode:false,permission_profile:"read_only".into(),tools:ToolPolicy{plugins:Some(vec![]),mcp_servers:Some(vec![]),skills:Some(vec![])},role:String::new()}]}).await?;
            let window=select(&core.computer).await?;
            core.configure_computer(&chat.id,vec![window.clone()]).await?;
            let mut events=core.subscribe();
            let prompt=if image{format!("Native MCP validation. Call ONLY computer_capture from smart-computer exactly once, window={window}, max_edge=640. Describe the text and colored shapes in the returned image in one sentence. Do not call any other tool, read any file, or take another capture. If the tool is unavailable or denied, report that; do not guess.")}
                else{"Native MCP validation. Call ONLY computer_status from smart-computer exactly once. Report whether enabled=true. Do not call any other tool, read files, or observe the desktop. If unavailable, report that.".into()};
            let run=core.send_message(&chat.id,prompt).await?;
            let mut approved=false;let mut saw_tool=false;let mut text=String::new();
            let until=tokio::time::Instant::now()+Duration::from_secs(150);
            loop{
                if tokio::time::Instant::now()>until{core.cancel(&chat.id)?;return Err("Codex validation timeout".into());}
                if let Some(p)=core.computer.status().await?.pending{
                    if image&&!approved&&p.tool=="computer_capture"&&p.window.as_deref()==Some(&window){core.approve_computer(p.id,true,false).await?;approved=true;}
                    else{core.approve_computer(p.id,false,false).await?;}
                }
                if let Ok(Ok(event))=tokio::time::timeout(Duration::from_millis(50),events.recv()).await{
                    if event.run_id!=run{continue;}
                    match event.payload{
                        EventPayload::ToolActivity{label,detail}=>{if label.contains("computer_")||detail.contains("computer_"){saw_tool=true;}},
                        EventPayload::AssistantTextDelta{text:delta}=>text.push_str(&delta),
                        EventPayload::ProviderError{message,..}=>return Err(agent_core::redaction::redact(&message).into()),
                        EventPayload::ApprovalRequested{..}=>return Err("Provider requires an additional approval; native validation did not bypass it".into()),
                        EventPayload::TurnCompleted=>break,
                        _=>{}
                    }
                }
            }
            let metrics=core.computer.status().await?.metrics;
            if !saw_tool||metrics.tools==0{return Err("Codex did not call the real MCP tool".into());}
            if image&&(!approved||metrics.images_sent!=1){return Err("Codex image delivery was not verified".into());}
            println!("PASS: real Codex subscription MCP {}; metrics {}",if image{"image"}else{"status"},serde_json::to_string(&metrics)?);
            println!("Codex fixture result: {}",agent_core::redaction::redact(&text));
            Ok(())
        }.await;
        core.shutdown().await;
        result
    }
}

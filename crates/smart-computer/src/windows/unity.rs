//! Optional Unity Editor adapter. Native host identity is checked by both pipe endpoints.
use super::{security, win};
use crate::{engine::Shared, invalid, protocol::*, Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{os::windows::io::AsRawHandle, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::ClientOptions,
};
use windows::Win32::{Foundation::HANDLE, System::Pipes::GetNamedPipeServerProcessId};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    state: Option<Value>,
    image: Option<String>,
    error: Option<String>,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
}
fn call(window: &WindowInfo, request: Value, shared: &Shared) -> Result<Option<Reply>> {
    if !window.application.eq_ignore_ascii_case("Unity.exe") {
        return Ok(None);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async{
        let pipe=format!(r"\\.\pipe\BebekonUnity-{}",window.pid);
        let mut stream=match ClientOptions::new().open(pipe){Ok(s)=>s,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),Err(e)=>return Err(e.into())};
        let mut pid=0;win(unsafe{GetNamedPipeServerProcessId(HANDLE(stream.as_raw_handle()),&mut pid)})?;if pid!=window.pid{return Err(Error::Denied);}security::same_user(pid)?;
        let bytes=serde_json::to_vec(&request)?;if bytes.len()>16384{return Err(invalid("Unity: слишком большой запрос"));}stream.write_u32_le(bytes.len() as u32).await?;stream.write_all(&bytes).await?;
        let epoch=shared.0.lock().map_err(|_|Error::Busy)?.epoch;
        let response=async{let size=stream.read_u32_le().await? as usize;if size>MAX_MESSAGE{return Err(invalid("Unity: слишком большой ответ"));}let mut bytes=vec![0;size];stream.read_exact(&mut bytes).await?;let reply:Reply=serde_json::from_slice(&bytes)?;if reply.error.as_ref().is_some_and(|s|!s.is_empty()){return Err(invalid(reply.error.as_deref().unwrap_or("Unity request failed")));}Ok(Some(reply))};
        let cancelled=async{loop{tokio::time::sleep(Duration::from_millis(20)).await;let s=shared.0.lock().map_err(|_|Error::Busy)?;if s.epoch!=epoch||!s.status.enabled||s.status.stopped{return Err(Error::Cancelled);}}};
        tokio::select!{value=tokio::time::timeout(Duration::from_secs(5),response)=>value.map_err(|_|Error::Cancelled)?,value=cancelled=>value}
    })
}
pub fn observe(
    window: &WindowInfo,
    goal: Option<&str>,
    limit: usize,
    view: Option<AdapterView>,
    visual: bool,
    shared: &Shared,
) -> Result<Option<ToolResult>> {
    let view = view.unwrap_or(AdapterView::State);
    if visual && !matches!(view, AdapterView::Scene | AdapterView::Game) {
        return Err(invalid("Unity: для изображения выберите scene или game"));
    }
    if !visual && matches!(view, AdapterView::Scene | AdapterView::Game) {
        return Err(invalid("Unity: кадр Scene/Game требует visual=true"));
    }
    let Some(reply) = call(
        window,
        json!({"op":"observe","view":view,"visual":visual,"goal":goal,"limit":limit}),
        shared,
    )?
    else {
        return Ok(None);
    };
    if let Some(encoded) = reply.image.filter(|s| !s.is_empty()) {
        if !visual
            || encoded.len() > 2_800_000
            || reply.width == 0
            || reply.height == 0
            || reply.width.max(reply.height) > 1280
        {
            return Err(invalid("Unity: некорректный кадр"));
        }
        let png = STANDARD
            .decode(&encoded)
            .map_err(|_| invalid("Unity: некорректное изображение"))?;
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(&png), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(1280);
        limits.max_image_height = Some(1280);
        limits.max_alloc = Some(16 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|_| invalid("Unity: некорректный PNG"))?;
        if image.width() != reply.width || image.height() != reply.height {
            return Err(invalid("Unity: размеры кадра не совпадают"));
        }
        Ok(Some(ToolResult {
            data: json!({"source":"unity_adapter","view":view,"width":reply.width,"height":reply.height,"coordinate_space":"unity_render_pixels","pointer_coordinates":"Use computer_capture for window-relative input coordinates.","scene_frame":"Scene camera output; use window capture for Editor overlays/gizmos."}),
            image: Some(encoded),
        }))
    } else {
        let state = reply
            .state
            .ok_or_else(|| invalid("Unity: нет данных наблюдения"))?;
        if state.to_string().len() > 64 * 1024 {
            return Err(invalid("Unity: уменьшите limit"));
        }
        Ok(Some(ToolResult {
            data: json!({"source":"unity_adapter","view":view,"state":state,"console_scope":"Messages since adapter enablement; no private Console APIs."}),
            image: None,
        }))
    }
}
pub fn act(window: &WindowInfo, command: &UnityCommand, shared: &Shared) -> Result<bool> {
    let reply = call(
        window,
        json!({"op":"act","command":command,"limit":1}),
        shared,
    )?
    .ok_or_else(|| invalid("Включите Unity Adapter: Tools → Bebekon → Smart Computer"))?;
    Ok(reply.verified)
}

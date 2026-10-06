//! Newline-delimited JSON-RPC peer for `codex app-server` over stdio.

use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// A single protocol line can carry a whole file diff; anything larger is treated as corrupt.
const MAX_LINE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub enum Incoming {
    Notification {
        method: String,
        params: Value,
    },
    Request {
        id: Value,
        method: String,
        params: Value,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RpcError {
    /// The server answered with a JSON-RPC error object.
    Server {
        code: i64,
        message: String,
    },
    Closed,
    Timeout,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>>;

pub struct RpcPeer {
    outgoing: mpsc::Sender<String>,
    pending: Pending,
    next_id: AtomicU64,
    incoming: broadcast::Sender<Incoming>,
    closed: CancellationToken,
}

impl RpcPeer {
    /// Async transport for in-memory peers in tests.
    #[cfg(test)]
    pub fn start<R, W>(reader: R, writer: W) -> Arc<Self>
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
        W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        use futures_util::{SinkExt, StreamExt};
        use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};
        let (outgoing, mut queue) = mpsc::channel::<String>(256);
        let (lines_tx, lines) = mpsc::channel::<String>(1024);
        let closed = CancellationToken::new();

        let write_closed = closed.clone();
        tokio::spawn(async move {
            let mut sink = FramedWrite::new(writer, LinesCodec::new());
            loop {
                tokio::select! {
                    _ = write_closed.cancelled() => break,
                    line = queue.recv() => match line {
                        Some(line) => if sink.send(line).await.is_err() { break },
                        None => break,
                    },
                }
            }
            write_closed.cancel();
        });
        let read_closed = closed.clone();
        tokio::spawn(async move {
            let mut framed = FramedRead::new(reader, LinesCodec::new_with_max_length(MAX_LINE));
            loop {
                let line = tokio::select! {
                    _ = read_closed.cancelled() => break,
                    line = framed.next() => line,
                };
                let Some(Ok(line)) = line else { break };
                if lines_tx.send(line).await.is_err() {
                    break;
                }
            }
        });
        Self::with_lines(outgoing, lines, closed)
    }

    /// Child-process transport. Pipes are read and written on dedicated OS threads, never on the
    /// async runtime's blocking pool, so a helper process that inherited a pipe can never stall
    /// runtime shutdown or app exit.
    pub fn from_process(
        stdout: std::process::ChildStdout,
        stdin: std::process::ChildStdin,
    ) -> std::io::Result<Arc<Self>> {
        let (outgoing, mut queue) = mpsc::channel::<String>(256);
        let (lines_tx, lines) = mpsc::channel::<String>(1024);
        let closed = CancellationToken::new();
        std::thread::Builder::new()
            .name("codex-rpc-writer".into())
            .spawn(move || {
                use std::io::Write;
                let mut stdin = std::io::BufWriter::new(stdin);
                while let Some(line) = queue.blocking_recv() {
                    let written = stdin
                        .write_all(line.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush());
                    if written.is_err() {
                        break;
                    }
                }
            })?;
        std::thread::Builder::new()
            .name("codex-rpc-reader".into())
            .spawn(move || {
                use std::io::{BufRead, Read};
                let mut reader = std::io::BufReader::new(stdout);
                loop {
                    let mut buffer = Vec::new();
                    match reader
                        .by_ref()
                        .take(MAX_LINE as u64 + 1)
                        .read_until(b'\n', &mut buffer)
                    {
                        Ok(0) | Err(_) => break,
                        Ok(_) if buffer.len() > MAX_LINE => break,
                        Ok(_) => {}
                    }
                    let line = String::from_utf8_lossy(&buffer).trim_end().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if lines_tx.blocking_send(line).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self::with_lines(outgoing, lines, closed))
    }

    fn with_lines(
        outgoing: mpsc::Sender<String>,
        mut lines: mpsc::Receiver<String>,
        closed: CancellationToken,
    ) -> Arc<Self> {
        let (incoming, _) = broadcast::channel(4096);
        let pending: Pending = Arc::default();
        let read_pending = Arc::clone(&pending);
        let read_incoming = incoming.clone();
        let read_closed = closed.clone();
        tokio::spawn(async move {
            loop {
                let line = tokio::select! {
                    _ = read_closed.cancelled() => break,
                    line = lines.recv() => line,
                };
                let Some(line) = line else { break };
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    // Non-protocol output is ignored rather than logged: it may contain secrets.
                    continue;
                };
                dispatch(message, &read_pending, &read_incoming);
            }
            read_closed.cancel();
            if let Ok(mut pending) = read_pending.lock() {
                for (_, waiter) in pending.drain() {
                    let _ = waiter.send(Err(RpcError::Closed));
                }
            }
        });
        Arc::new(Self {
            outgoing,
            pending,
            next_id: AtomicU64::new(1),
            incoming,
            closed,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Incoming> {
        self.incoming.subscribe()
    }

    pub fn is_closed(&self) -> bool {
        self.closed.is_cancelled()
    }

    pub fn closed(&self) -> CancellationToken {
        self.closed.clone()
    }

    pub fn close(&self) {
        self.closed.cancel();
    }

    pub async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, RpcError> {
        if self.is_closed() {
            return Err(RpcError::Closed);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| RpcError::Closed)?
            .insert(id, tx);
        // Parameterless methods omit `params`, matching the official client.
        let line = if params.is_null() {
            json!({ "id": id, "method": method })
        } else {
            json!({ "id": id, "method": method, "params": params })
        }
        .to_string();
        if self.outgoing.send(line).await.is_err() {
            self.forget(id);
            return Err(RpcError::Closed);
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(RpcError::Closed),
            Err(_) => {
                self.forget(id);
                Err(RpcError::Timeout)
            }
        }
    }

    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), RpcError> {
        let message = match params {
            Some(params) => json!({ "method": method, "params": params }),
            None => json!({ "method": method }),
        };
        self.outgoing
            .send(message.to_string())
            .await
            .map_err(|_| RpcError::Closed)
    }

    pub async fn respond(&self, id: Value, result: Value) -> Result<(), RpcError> {
        self.outgoing
            .send(json!({ "id": id, "result": result }).to_string())
            .await
            .map_err(|_| RpcError::Closed)
    }

    pub async fn respond_error(&self, id: Value, code: i64, message: &str) -> Result<(), RpcError> {
        self.outgoing
            .send(json!({ "id": id, "error": { "code": code, "message": message } }).to_string())
            .await
            .map_err(|_| RpcError::Closed)
    }

    fn forget(&self, id: u64) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&id);
        }
    }
}

fn dispatch(message: Value, pending: &Pending, incoming: &broadcast::Sender<Incoming>) {
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").filter(|id| !id.is_null()).cloned();
    match (method, id) {
        (Some(method), Some(id)) => {
            let _ = incoming.send(Incoming::Request {
                id,
                method: method.into(),
                params: message.get("params").cloned().unwrap_or(Value::Null),
            });
        }
        (Some(method), None) => {
            let _ = incoming.send(Incoming::Notification {
                method: method.into(),
                params: message.get("params").cloned().unwrap_or(Value::Null),
            });
        }
        (None, Some(id)) => {
            let Some(id) = id.as_u64() else { return };
            let waiter = pending.lock().ok().and_then(|mut map| map.remove(&id));
            let Some(waiter) = waiter else { return };
            let result = match message.get("error") {
                Some(error) => Err(RpcError::Server {
                    code: error.get("code").and_then(Value::as_i64).unwrap_or(-32000),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Неизвестная ошибка")
                        .chars()
                        .take(500)
                        .collect(),
                }),
                None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
            };
            let _ = waiter.send(result);
        }
        (None, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn correlates_responses_and_routes_server_traffic() {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, mut server_write) = tokio::io::split(server_io);
        let peer = RpcPeer::start(client_read, client_write);
        let mut incoming = peer.subscribe();

        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_read).lines();
            let request: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(request["method"], "account/read");
            let id = request["id"].clone();
            let out = format!(
                "{}\n{}\n{}\n",
                json!({"method": "turn/started", "params": {"threadId": "t"}}),
                json!({"id": "srv-1", "method": "item/commandExecution/requestApproval", "params": {}}),
                json!({"id": id, "result": {"requiresOpenaiAuth": true}}),
            );
            server_write.write_all(out.as_bytes()).await.unwrap();
            let reply: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(
                reply,
                json!({"id": "srv-1", "result": {"decision": "decline"}})
            );
        });

        let result = peer
            .request("account/read", json!({}), Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(result["requiresOpenaiAuth"], true);
        assert!(matches!(
            incoming.recv().await.unwrap(),
            Incoming::Notification { ref method, .. } if method == "turn/started"
        ));
        let Incoming::Request { id, .. } = incoming.recv().await.unwrap() else {
            panic!("expected server request");
        };
        peer.respond(id, json!({"decision": "decline"}))
            .await
            .unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn pending_requests_fail_when_the_process_exits() {
        let (client_io, server_io) = tokio::io::duplex(1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let peer = RpcPeer::start(client_read, client_write);
        let waiting = {
            let peer = Arc::clone(&peer);
            tokio::spawn(async move {
                peer.request("model/list", json!({}), Duration::from_secs(5))
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(server_io);
        assert_eq!(waiting.await.unwrap(), Err(RpcError::Closed));
        assert!(peer.is_closed());
    }

    #[tokio::test]
    async fn server_errors_are_reported_without_hanging() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, mut server_write) = tokio::io::split(server_io);
        let peer = RpcPeer::start(client_read, client_write);
        tokio::spawn(async move {
            let mut lines = BufReader::new(server_read).lines();
            let request: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let reply =
                json!({"id": request["id"], "error": {"code": -32600, "message": "auth required"}});
            server_write
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
        });
        let error = peer
            .request(
                "account/rateLimits/read",
                Value::Null,
                Duration::from_secs(5),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error,
            RpcError::Server {
                code: -32600,
                message: "auth required".into()
            }
        );
    }
}

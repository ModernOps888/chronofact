use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::ChildStdin;
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};
use tracing::{debug, error, info, warn};

use super::types::UpstreamServerConfig;

/// Outcome channel type: Ok(result_value) or Err(error_message)
pub type PendingResult = Result<serde_json::Value, String>;

/// Channel types for server death notifications
pub type DeathSender = mpsc::UnboundedSender<String>;
pub type DeathReceiver = mpsc::UnboundedReceiver<String>;

/// A persistent, multiplexed connection to an upstream stdio MCP server.
/// Holds a long-lived child process with piped stdin/stdout.
/// A background reader task correlates responses by JSON-RPC request ID,
/// enabling concurrent multiplexed requests over a single connection.
pub struct StdioConnection {
    writer: AsyncMutex<BufWriter<ChildStdin>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>>,
    next_id: AtomicI64,
    server_name: String,
    request_timeout: Duration,
    _reader_handle: tokio::task::JoinHandle<()>,
    _child_handle: tokio::task::JoinHandle<()>,
}

impl StdioConnection {
    /// Spawns the child process, performs the MCP handshake (`initialize` + `notifications/initialized`),
    /// and returns the persistent connection along with declared server capabilities.
    pub async fn connect(
        config: &UpstreamServerConfig,
        death_tx: DeathSender,
        request_timeout: Duration,
        handshake_timeout: Duration,
    ) -> anyhow::Result<(Self, serde_json::Value)> {
        let command = config
            .command
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No command configured for server '{}'", config.name))?;

        info!(
            "🔌 ChronoFact Gateway: Spawning stdio server '{}': {} {:?}",
            config.name, command, config.args
        );

        let mut cmd = tokio::process::Command::new(command);
        cmd.args(&config.args);

        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            anyhow::anyhow!(
                "Failed to spawn upstream server '{}' ({} {:?}): {}",
                config.name,
                command,
                config.args,
                e
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdin for server '{}'", config.name))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout for server '{}'", config.name))?;

        // Drain stderr asynchronously to prevent OS pipe buffer exhaustion deadlocks
        if let Some(stderr) = child.stderr.take() {
            let stderr_server_name = config.name.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    debug!(target: "gateway_child_stderr", server = %stderr_server_name, "{}", line);
                }
            });
        }

        let writer = AsyncMutex::new(BufWriter::new(stdin));
        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let server_name = config.name.clone();

        // Spawn background reader loop
        let pending_for_reader = Arc::clone(&pending);
        let name_for_reader = server_name.clone();
        let _reader_handle = tokio::spawn(async move {
            reader_loop(stdout, pending_for_reader, name_for_reader).await;
        });

        // Spawn child exit monitor
        let name_for_child = server_name.clone();
        let pending_for_child = Arc::clone(&pending);
        let _child_handle = tokio::spawn(async move {
            let status = child.wait().await;
            match status {
                Ok(s) => warn!("⚠️ ChronoFact Gateway: Stdio server '{}' exited: {}", name_for_child, s),
                Err(e) => error!("❌ ChronoFact Gateway: Stdio server '{}' wait error: {}", name_for_child, e),
            }

            // Drain all pending requests with error
            if let Ok(mut map) = pending_for_child.lock() {
                let count = map.len();
                for (id, sender) in map.drain() {
                    let _ = sender.send(Err(format!(
                        "Server '{}' terminated while request id={} was in-flight",
                        name_for_child, id
                    )));
                }
                if count > 0 {
                    warn!(
                        "⚠️ ChronoFact Gateway: Drained {} in-flight requests for exited server '{}'",
                        count, name_for_child
                    );
                }
            }

            let _ = death_tx.send(name_for_child);
        });

        let conn = Self {
            writer,
            pending,
            next_id: AtomicI64::new(10), // 1-9 reserved for initial handshake
            server_name,
            request_timeout,
            _reader_handle,
            _child_handle,
        };

        // Perform MCP Handshake
        let init_result: serde_json::Value = conn
            .send_request_with_timeout(
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "chronofact-gateway",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
                handshake_timeout,
            )
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "MCP initialize handshake failed for server '{}': {}",
                    conn.server_name,
                    e
                )
            })?;

        conn.send_notification("notifications/initialized").await?;

        let capabilities = init_result.get("capabilities").cloned().unwrap_or_default();

        info!(
            "🤝 ChronoFact Gateway: MCP handshake successful for '{}': capabilities: {}",
            conn.server_name,
            serde_json::to_string(&capabilities).unwrap_or_default()
        );

        Ok((conn, capabilities))
    }

    /// Send a JSON-RPC request and wait for the correlated response using standard request timeout
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        self.send_request_with_timeout(method, params, self.request_timeout)
            .await
    }

    /// Send a JSON-RPC request with custom timeout override
    pub async fn send_request_with_timeout(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> anyhow::Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        // Register pending channel
        {
            let mut map = self.pending.lock().map_err(|_| {
                anyhow::anyhow!("Pending map lock poisoned for '{}'", self.server_name)
            })?;
            map.insert(id, tx);
        }

        // Build request payload
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut request_line = serde_json::to_string(&request)?;
        request_line.push('\n');

        // Write to stdin with write lock protected by timeout
        let write_fut = async {
            let mut writer = self.writer.lock().await;
            writer.write_all(request_line.as_bytes()).await?;
            writer.flush().await?;
            Ok::<(), std::io::Error>(())
        };

        if let Err(_) = tokio::time::timeout(Duration::from_secs(5), write_fut).await {
            self.remove_pending(id);
            return Err(anyhow::anyhow!(
                "Timed out writing to stdin of '{}' (child process may be unresponsive)",
                self.server_name
            ));
        }

        debug!("📤 [{}] → {} (id={})", self.server_name, method, id);

        // Await correlated response
        let recv_result = if timeout.is_zero() {
            rx.await.map_err(|_| {
                anyhow::anyhow!(
                    "Response channel for '{}' dropped (child process may have terminated)",
                    self.server_name
                )
            })
        } else {
            tokio::time::timeout(timeout, rx)
                .await
                .map_err(|_| {
                    self.remove_pending(id);
                    anyhow::anyhow!(
                        "Request to '{}' timed out after {}s (method={}, id={})",
                        self.server_name,
                        timeout.as_secs(),
                        method,
                        id
                    )
                })?
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Response channel for '{}' dropped (child process may have terminated)",
                        self.server_name
                    )
                })
        };

        match recv_result? {
            Ok(val) => {
                debug!("📥 [{}] ← {} (id={}) OK", self.server_name, method, id);
                Ok(val)
            }
            Err(err_msg) => {
                warn!(
                    "📥 [{}] ← {} (id={}) ERROR: {}",
                    self.server_name, method, id, err_msg
                );
                Err(anyhow::anyhow!(
                    "Upstream server '{}' returned error: {}",
                    self.server_name,
                    err_msg
                ))
            }
        }
    }

    /// Send a fire-and-forget notification
    pub async fn send_notification(&self, method: &str) -> anyhow::Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
        });

        let mut line = serde_json::to_string(&notification)?;
        line.push('\n');

        let mut writer = self.writer.lock().await;
        writer.write_all(line.as_bytes()).await?;
        writer.flush().await?;

        debug!("📤 [{}] → {} (notification)", self.server_name, method);
        Ok(())
    }

    fn remove_pending(&self, id: i64) {
        if let Ok(mut map) = self.pending.lock() {
            map.remove(&id);
        }
    }
}

/// Continuous reader loop that decodes JSON-RPC messages from the child stdout
async fn reader_loop(
    stdout: tokio::process::ChildStdout,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>>,
    server_name: String,
) {
    let mut reader = BufReader::new(stdout);
    let mut buf = Vec::new();

    loop {
        buf.clear();
        let bytes_read = match reader.read_until(b'\n', &mut buf).await {
            Ok(n) => n,
            Err(e) => {
                error!("[{}] Error reading child stdout: {}", server_name, e);
                break;
            }
        };

        if bytes_read == 0 {
            info!("📡 [{}] stdout EOF reached", server_name);
            break;
        }

        let text = String::from_utf8_lossy(&buf);
        let trimmed = text.trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            continue;
        }

        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(id) = parsed.get("id").and_then(|i| i.as_i64()) {
            let result = if let Some(res) = parsed.get("result") {
                Ok(res.clone())
            } else if let Some(err) = parsed.get("error") {
                let msg = err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown upstream error");
                let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                Err(format!("{} (code: {})", msg, code))
            } else {
                Err("Invalid JSON-RPC: missing both result and error".to_string())
            };

            if let Ok(mut map) = pending.lock() {
                if let Some(sender) = map.remove(&id) {
                    let _ = sender.send(result);
                } else {
                    debug!("[{}] Received response for unknown/expired id={}", server_name, id);
                }
            }
        } else if let Some(method) = parsed.get("method").and_then(|m| m.as_str()) {
            debug!("[{}] 📣 Upstream notification: {}", server_name, method);
        }
    }

    // Drain remaining pending requests
    if let Ok(mut map) = pending.lock() {
        let count = map.len();
        for (id, sender) in map.drain() {
            let _ = sender.send(Err(format!(
                "Upstream connection to '{}' closed while request id={} was pending",
                server_name, id
            )));
        }
        if count > 0 {
            warn!("[{}] Drained {} requests following stdout closure", server_name, count);
        }
    }
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use crate::core::file_logger::FileLogger;

pub struct AgentWebSocket;

impl AgentWebSocket {
    pub async fn run_loop(
        server_url: String,
        agent_id: u64,
        token: String,
        running: Arc<AtomicBool>,
    ) {
        let ws_base = if server_url.starts_with("https://") {
            server_url.replacen("https://", "wss://", 1)
        } else {
            server_url.replacen("http://", "ws://", 1)
        };
        let ws_url = format!("{}/agent-ws?agentId={}&token={}", ws_base.trim_end_matches('/'), agent_id, token);

        while running.load(Ordering::SeqCst) {
            FileLogger::info(&format!("[AgentWS] Connecting to WebSocket: {}", ws_url));

            match tokio::time::timeout(Duration::from_secs(10), connect_async(&ws_url)).await {
                Ok(Ok((ws_stream, _))) => {
                    FileLogger::info("[AgentWS] ✅ WebSocket connected to central server");
                    let (mut write, mut read) = ws_stream.split();

                    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(25));

                    loop {
                        tokio::select! {
                            _ = heartbeat_interval.tick() => {
                                let hb = serde_json::json!({
                                    "type": "HEARTBEAT",
                                    "agentId": agent_id,
                                    "timestamp": chrono::Utc::now().to_rfc3339()
                                });
                                if let Err(e) = write.send(Message::Text(hb.to_string().into())).await {
                                    FileLogger::warn(&format!("[AgentWS] Heartbeat send failed: {}", e));
                                    break;
                                }
                            }
                            msg_opt = read.next() => {
                                match msg_opt {
                                    Some(Ok(Message::Text(text))) => {
                                        Self::handle_message(&text, &mut write).await;
                                    }
                                    Some(Ok(Message::Ping(data))) => {
                                        let _ = write.send(Message::Pong(data)).await;
                                    }
                                    Some(Ok(Message::Close(frame))) => {
                                        FileLogger::info(&format!("[AgentWS] Connection closed by server: {:?}", frame));
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        FileLogger::warn(&format!("[AgentWS] Receive error: {}", e));
                                        break;
                                    }
                                    None => {
                                        FileLogger::info("[AgentWS] Stream ended");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    FileLogger::warn(&format!("[AgentWS] Connection failed: {}", e));
                }
                Err(_) => {
                    FileLogger::warn("[AgentWS] Connection timed out after 10s");
                }
            }

            if !running.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn handle_message<S>(text: &str, write: &mut S)
    where
        S: SinkExt<Message> + Unpin,
        S::Error: std::fmt::Display,
    {
        let msg: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return,
        };

        let req_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let request_id = msg.get("requestId").and_then(|v| v.as_str()).unwrap_or("");

        if request_id.is_empty() {
            return;
        }

        let resp = match req_type {
            "EXPLORER_GET_DRIVES" => {
                serde_json::json!({
                    "type": "EXPLORER_GET_DRIVES_RESPONSE",
                    "requestId": request_id,
                    "success": true,
                    "data": [
                        { "id": 1, "name": "Macintosh HD (/)" },
                        { "id": 2, "name": "User Home (/Users)" },
                        { "id": 3, "name": "Volumes & Removable (/Volumes)" }
                    ]
                })
            }
            "EXPLORER_GET_NODE" | "EXPLORER_LIST_CHILDREN" => {
                serde_json::json!({
                    "type": format!("{}_RESPONSE", req_type),
                    "requestId": request_id,
                    "success": true,
                    "data": { "children": [] }
                })
            }
            other => {
                serde_json::json!({
                    "type": format!("{}_RESPONSE", other),
                    "requestId": request_id,
                    "success": true,
                    "data": {}
                })
            }
        };

        let _ = write.send(Message::Text(resp.to_string().into())).await;
    }
}

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::error::{AppError, Result};
use crate::ipc::IpcMessage;

pub struct IpcServer {
    pub socket_path: PathBuf,
    pub rx: Option<mpsc::Receiver<IpcMessage>>,
    task: tokio::task::JoinHandle<()>,
}

impl IpcServer {
    pub async fn bind(socket_path: PathBuf) -> Result<Self> {
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        // Clean up stale existing socket
        if socket_path.exists() {
            let _ = tokio::fs::remove_file(&socket_path).await;
        }
        let listener = UnixListener::bind(&socket_path)
            .map_err(|e| AppError::ipc(format!("bind {} failed: {e}", socket_path.display())))?;
        // Set permissions to 0600
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&socket_path, perms)?;

        let (tx, rx) = mpsc::channel::<IpcMessage>(64);

        let task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_conn(stream, tx).await {
                                tracing::warn!(error = %e, "ipc conn error");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "ipc accept failed");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            socket_path,
            rx: Some(rx),
            task,
        })
    }

    /// Take ownership of the receive channel. Can only be called once, immediately after `bind`.
    pub fn take_rx(&mut self) -> Option<mpsc::Receiver<IpcMessage>> {
        self.rx.take()
    }

    /// Delete the socket file. Also done automatically by `Drop`, but available for explicit cleanup.
    pub fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}

impl Drop for IpcServer {
    /// Stop the accept loop and delete the Unix socket file.
    /// As a guarantee of FR-13 "App termination", ensures the socket file
    /// is removed regardless of how `run` completes (normal exit or panic).
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

async fn handle_conn(stream: tokio::net::UnixStream, tx: mpsc::Sender<IpcMessage>) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half).lines();
    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<IpcMessage>(&line) {
            Ok(msg) => {
                let response = match &msg {
                    IpcMessage::Prompt { .. } | IpcMessage::PromptReply { .. } => Some(IpcMessage::Ack { id: 0 }),
                    IpcMessage::Ping => Some(IpcMessage::Pong),
                    _ => None,
                };
                if tx.send(msg).await.is_err() {
                    return Ok(());
                }
                if let Some(resp) = response {
                    let line = serde_json::to_string(&resp)?;
                    write_half.write_all(line.as_bytes()).await?;
                    write_half.write_all(b"\n").await?;
                }
            }
            Err(e) => {
                let err = IpcMessage::Error {
                    message: format!("parse error: {e}"),
                };
                let line = serde_json::to_string(&err)?;
                let _ = write_half.write_all(line.as_bytes()).await;
                let _ = write_half.write_all(b"\n").await;
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::id::AgentId;
    use crate::ipc::client;

    #[tokio::test]
    async fn server_receives_prompt() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sock");
        let mut server = IpcServer::bind(path.clone()).await.unwrap();
        let mut rx = server.take_rx().expect("rx not taken yet");
        let from = AgentId::new();
        let msg = IpcMessage::Prompt {
            from: from.clone(),
            from_name: Some("tester".into()),
            text: "hello".into(),
            reply_to: None,
        };
        let resp = client::send(&path, &msg).await.unwrap();
        assert!(matches!(resp, IpcMessage::Ack { .. }));
        let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("recv timeout")
            .expect("channel closed");
        match received {
            IpcMessage::Prompt { text, .. } => assert_eq!(text, "hello"),
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn drop_removes_socket_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("drop.sock");
        {
            let _server = IpcServer::bind(path.clone()).await.unwrap();
            assert!(path.exists(), "socket should exist while server is alive");
        }
        // Brief wait: Drop is synchronous, but just in case
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !path.exists(),
            "socket file should be removed by Drop, but {} still exists",
            path.display()
        );
    }

    #[tokio::test]
    async fn server_responds_ack_to_prompt_reply() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("reply_test.sock");
        let mut server = IpcServer::bind(path.clone()).await.unwrap();
        let mut rx = server.take_rx().expect("rx not taken yet");

        let msg = IpcMessage::PromptReply {
            from: AgentId::new(),
            text: "hello reply".into(),
        };
        let resp = client::send(&path, &msg).await.unwrap();
        assert!(
            matches!(resp, IpcMessage::Ack { .. }),
            "expected Ack for PromptReply, got {:?}",
            resp
        );

        // Verify the PromptReply is forwarded to the channel
        let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("recv timeout")
            .expect("channel closed");
        match received {
            IpcMessage::PromptReply { text, .. } => assert_eq!(text, "hello reply"),
            other => panic!("unexpected message: {:?}", other),
        }
    }
}

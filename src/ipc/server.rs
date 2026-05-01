use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::error::{AppError, Result};
use crate::ipc::IpcMessage;

pub struct IpcServer {
    #[allow(dead_code)]
    pub socket_path: PathBuf,
    pub rx: mpsc::Receiver<IpcMessage>,
    _task: tokio::task::JoinHandle<()>,
}

impl IpcServer {
    pub async fn bind(socket_path: PathBuf) -> Result<Self> {
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        // 既存ソケット（stale）の掃除
        if socket_path.exists() {
            let _ = tokio::fs::remove_file(&socket_path).await;
        }
        let listener = UnixListener::bind(&socket_path)
            .map_err(|e| AppError::ipc(format!("bind {} failed: {e}", socket_path.display())))?;
        // パーミッション 0600
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
            rx,
            _task: task,
        })
    }

    pub fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
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
                    IpcMessage::Prompt { .. } => Some(IpcMessage::Ack { id: 0 }),
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
        let from = AgentId::new();
        let msg = IpcMessage::Prompt {
            from: from.clone(),
            from_name: Some("tester".into()),
            text: "hello".into(),
        };
        let resp = client::send(&path, &msg).await.unwrap();
        assert!(matches!(resp, IpcMessage::Ack { .. }));
        let received = tokio::time::timeout(std::time::Duration::from_secs(2), server.rx.recv())
            .await
            .expect("recv timeout")
            .expect("channel closed");
        match received {
            IpcMessage::Prompt { text, .. } => assert_eq!(text, "hello"),
            other => panic!("unexpected message: {:?}", other),
        }
    }
}

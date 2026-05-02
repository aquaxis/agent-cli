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
            rx: Some(rx),
            task,
        })
    }

    /// 受信チャネルを所有権ごと取り出す。`bind` 直後に一度だけ呼べる。
    pub fn take_rx(&mut self) -> Option<mpsc::Receiver<IpcMessage>> {
        self.rx.take()
    }

    /// ソケットファイルを削除する。`Drop` でも自動的に行われるが、明示削除したい場合用。
    pub fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}

impl Drop for IpcServer {
    /// accept ループを停止し、Unix ソケットファイルを削除する。
    /// FR-13「アプリ終了」の保証として、`run` 完了経路（正常終了／パニック）いずれでも socket が残らないことを担保する。
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
        let mut rx = server.take_rx().expect("rx not taken yet");
        let from = AgentId::new();
        let msg = IpcMessage::Prompt {
            from: from.clone(),
            from_name: Some("tester".into()),
            text: "hello".into(),
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
        // 短い待ち：Drop は同期的だが、念のため
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !path.exists(),
            "socket file should be removed by Drop, but {} still exists",
            path.display()
        );
    }
}

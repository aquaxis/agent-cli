use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::{AppError, Result};
use crate::ipc::IpcMessage;

pub async fn send(socket: &Path, msg: &IpcMessage) -> Result<IpcMessage> {
    let stream = tokio::time::timeout(Duration::from_secs(5), UnixStream::connect(socket))
        .await
        .map_err(|_| AppError::ipc("connect timed out"))?
        .map_err(|e| AppError::ipc(format!("connect {} failed: {e}", socket.display())))?;
    let (read_half, mut write_half) = stream.into_split();
    let line = serde_json::to_string(msg)?;
    write_half.write_all(line.as_bytes()).await?;
    write_half.write_all(b"\n").await?;
    write_half.shutdown().await.ok();
    let mut reader = BufReader::new(read_half).lines();
    if let Some(line) = reader.next_line().await? {
        let resp: IpcMessage = serde_json::from_str(&line)?;
        Ok(resp)
    } else {
        Err(AppError::ipc("no response from peer"))
    }
}

use serde::{Deserialize, Serialize};

use crate::id::AgentId;

pub mod client;
pub mod registry;
pub mod server;

/// プロセス間通信で使う JSON Lines メッセージ。
///
/// Unix ドメインソケット経由で 1 行 1 メッセージとして送受信する。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IpcMessage {
    /// 別エージェントに送るプロンプト。
    Prompt {
        /// 送信元 AgentId。
        from: AgentId,
        /// 送信元の表示名（任意）。
        from_name: Option<String>,
        /// 本文。
        text: String,
    },
    /// 受信成功の確認応答。
    Ack {
        /// メッセージ識別子（将来拡張用、現状 0 固定）。
        id: u64,
    },
    /// エラー応答。
    Error {
        /// 人間可読のエラーメッセージ。
        message: String,
    },
    /// 疎通確認（要求）。
    Ping,
    /// 疎通確認（応答）。
    Pong,
}

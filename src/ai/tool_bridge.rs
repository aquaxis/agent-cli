use serde_json::{json, Value};

use crate::ai::ToolSpec;

/// Anthropic 形式のツール定義（input_schema）に変換。
pub fn to_anthropic_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.schema,
            })
        })
        .collect()
}

/// OpenAI 互換のツール定義（function calling）に変換。
pub fn to_openai_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.schema,
                }
            })
        })
        .collect()
}

/// Ollama `/api/chat` のツール定義は OpenAI 形式とほぼ同一。
pub fn to_ollama_tools(tools: &[ToolSpec]) -> Vec<Value> {
    to_openai_tools(tools)
}

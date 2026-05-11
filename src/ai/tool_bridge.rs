use serde_json::{json, Value};

use crate::ai::ToolSpec;

/// Convert to Anthropic-style tool definition (input_schema).
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

/// Convert to OpenAI-compatible tool definition (function calling).
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

/// Ollama `/api/chat` tool definitions are nearly identical to the OpenAI format.
pub fn to_ollama_tools(tools: &[ToolSpec]) -> Vec<Value> {
    to_openai_tools(tools)
}

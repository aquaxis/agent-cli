//! Pure, I/O-free helpers for the MCP client: tool-name mangling, JSON-RPC 2.0
//! message construction/parsing, and shaping of `tools/list` / `tools/call`
//! results. Everything here is unit-testable without spawning a process.

use serde_json::{json, Value};

use crate::error::Result;
use crate::tools::ToolOutput;

/// A tool as advertised by an MCP server's `tools/list`.
#[derive(Debug, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Fold any character that is not `[A-Za-z0-9_]` to `_`, so a server- or
/// tool-name is safe to embed in a namespaced identifier and in the provider
/// wire format. An all-invalid (or empty) input yields a single `_`.
pub fn sanitize_token(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if t.is_empty() {
        "_".to_string()
    } else {
        t
    }
}

/// Namespaced registry key / spec name for an MCP tool: `mcp__<server>__<tool>`.
pub fn mcp_tool_name(server: &str, tool: &str) -> String {
    format!("mcp__{}__{}", sanitize_token(server), sanitize_token(tool))
}

/// A JSON-RPC 2.0 request object.
pub fn jsonrpc_request(id: i64, method: &str, params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

/// A JSON-RPC 2.0 notification (no `id`, no reply expected).
pub fn jsonrpc_notification(method: &str, params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

/// Parse one line of server output. Returns the correlation `id` (when present)
/// and either the `result` value or a formatted `error` message. A line that is
/// not a reply (e.g. a server-initiated notification with no `id`/`result`)
/// yields `(id, Ok(Value::Null))` and is ignored by the reader.
pub fn parse_jsonrpc_reply(line: &str) -> Result<(Option<i64>, std::result::Result<Value, String>)> {
    let v: Value = serde_json::from_str(line)?;
    let id = v.get("id").and_then(|i| i.as_i64());
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        let full = match err.get("code").and_then(|c| c.as_i64()) {
            Some(code) => format!("{msg} (code {code})"),
            None => msg.to_string(),
        };
        return Ok((id, Err(full)));
    }
    if let Some(result) = v.get("result") {
        return Ok((id, Ok(result.clone())));
    }
    Ok((id, Ok(Value::Null)))
}

/// Extract the tool definitions from a `tools/list` result. Tools without a
/// non-empty `name` are skipped; a missing `inputSchema` defaults to an empty
/// object schema.
pub fn parse_tools_list(result: &Value) -> Vec<McpToolDef> {
    let mut defs = Vec::new();
    if let Some(arr) = result.get("tools").and_then(|t| t.as_array()) {
        for t in arr {
            let name = match t.get("name").and_then(|n| n.as_str()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => continue,
            };
            let description = t
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let input_schema = t
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object" }));
            defs.push(McpToolDef {
                name,
                description,
                input_schema,
            });
        }
    }
    defs
}

/// Flatten a `tools/call` result's `content` blocks into a `ToolOutput`. Text
/// blocks are concatenated; non-text blocks are noted by type. An `isError:
/// true` result becomes `ToolOutput::err`.
pub fn flatten_tool_result(result: &Value) -> ToolOutput {
    let is_error = result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut parts: Vec<String> = Vec::new();
    if let Some(arr) = result.get("content").and_then(|c| c.as_array()) {
        for block in arr {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(s) = block.get("text").and_then(|t| t.as_str()) {
                        parts.push(s.to_string());
                    }
                }
                Some(other) => parts.push(format!("[{other} content]")),
                None => {}
            }
        }
    }

    let text = if !parts.is_empty() {
        parts.join("\n")
    } else if let Some(sc) = result.get("structuredContent") {
        sc.to_string()
    } else {
        String::new()
    };

    if is_error {
        ToolOutput::err(if text.is_empty() {
            "tool reported an error".to_string()
        } else {
            text
        })
    } else {
        ToolOutput::ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name_mangling_and_sanitize() {
        assert_eq!(mcp_tool_name("fs", "read_file"), "mcp__fs__read_file");
        assert_eq!(mcp_tool_name("my server.v2", "do it"), "mcp__my_server_v2__do_it");
        assert_eq!(sanitize_token("a.b-c"), "a_b_c");
        assert_eq!(sanitize_token(""), "_");
        assert_eq!(sanitize_token("!!!"), "___");
    }

    #[test]
    fn jsonrpc_request_shape() {
        let r = jsonrpc_request(1, "tools/list", &json!({}));
        assert_eq!(r["jsonrpc"], "2.0");
        assert_eq!(r["id"], 1);
        assert_eq!(r["method"], "tools/list");
        assert_eq!(r["params"], json!({}));

        let n = jsonrpc_notification("notifications/initialized", &json!({}));
        assert_eq!(n["jsonrpc"], "2.0");
        assert!(n.get("id").is_none());
        assert_eq!(n["method"], "notifications/initialized");
    }

    #[test]
    fn parse_reply_result_and_error() {
        let (id, payload) =
            parse_jsonrpc_reply(r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#).unwrap();
        assert_eq!(id, Some(1));
        assert_eq!(payload.unwrap(), json!({"tools":[]}));

        let (id, payload) =
            parse_jsonrpc_reply(r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"x"}}"#)
                .unwrap();
        assert_eq!(id, Some(2));
        assert_eq!(payload.unwrap_err(), "x (code -32601)");

        // A server notification (no id / result) is a no-op reply.
        let (id, payload) =
            parse_jsonrpc_reply(r#"{"jsonrpc":"2.0","method":"notifications/message"}"#).unwrap();
        assert_eq!(id, None);
        assert_eq!(payload.unwrap(), Value::Null);

        // Malformed JSON is an error, not a panic.
        assert!(parse_jsonrpc_reply("not json").is_err());
    }

    #[test]
    fn tools_list_extraction() {
        let result = json!({
            "tools": [
                {"name": "t1", "description": "d1", "inputSchema": {"type": "object"}},
                {"name": "", "description": "skip me"},
                {"name": "t2"}
            ]
        });
        let defs = parse_tools_list(&result);
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "t1");
        assert_eq!(defs[0].description, "d1");
        assert_eq!(defs[1].name, "t2");
        assert_eq!(defs[1].description, "");
        assert_eq!(defs[1].input_schema, json!({"type": "object"}));
    }

    #[test]
    fn tool_result_flattening() {
        let ok = flatten_tool_result(&json!({
            "content": [{"type": "text", "text": "hi"}, {"type": "text", "text": "there"}]
        }));
        assert!(ok.ok);
        assert_eq!(ok.content, "hi\nthere");

        let err = flatten_tool_result(&json!({
            "isError": true,
            "content": [{"type": "text", "text": "boom"}]
        }));
        assert!(!err.ok);
        assert_eq!(err.content, "boom");

        let nontext = flatten_tool_result(&json!({
            "content": [{"type": "image", "data": "…"}]
        }));
        assert!(nontext.ok);
        assert_eq!(nontext.content, "[image content]");

        let empty_err = flatten_tool_result(&json!({"isError": true}));
        assert!(!empty_err.ok);
        assert_eq!(empty_err.content, "tool reported an error");
    }
}

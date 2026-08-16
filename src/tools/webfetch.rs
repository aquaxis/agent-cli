use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct WebFetchTool;

#[derive(Debug, Deserialize)]
struct WebFetchArgs {
    url: String,
    prompt: String,
}

/// Minimal HTML-to-text conversion: strip script/style blocks, drop tags,
/// collapse whitespace, decode a few common entities. Not a full converter,
/// but enough for the LLM to reason over fetched page text.
fn html_to_text(html: &str) -> String {
    // Remove script/style/noscript blocks entirely (no backreferences — the Rust
    // regex crate does not support them, so list the closing tags explicitly).
    let strip_re = regex::Regex::new(
        r"(?si)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<noscript[^>]*>.*?</noscript>",
    )
    .unwrap();
    let s = strip_re.replace_all(html, " ");
    // Drop all remaining tags.
    let tag_re = regex::Regex::new("(?s)<[^>]+>").unwrap();
    let s = tag_re.replace_all(&s, " ");
    // Decode a handful of common entities.
    let s = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    let ws_re = regex::Regex::new(r"[ \t\f\v]+").unwrap();
    let s = ws_re.replace_all(&s, " ");
    let s = s.replace(" \n", "\n").replace("\n ", "\n");
    let nl_re = regex::Regex::new(r"\n{3,}").unwrap();
    let s = nl_re.replace_all(&s, "\n\n");
    s.trim().to_string()
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "webfetch"
    }

    fn description(&self) -> &'static str {
        "Fetch a URL, convert HTML to readable text, and return it so the model can answer a prompt against the content."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "URL to fetch (http is upgraded to https)"},
                "prompt": {"type": "string", "description": "Question to answer against the fetched content"}
            },
            "required": ["url", "prompt"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: WebFetchArgs = serde_json::from_value(args)?;
        let url = if parsed.url.starts_with("http://") {
            format!("https://{}", &parsed.url[7..])
        } else {
            parsed.url.clone()
        };
        if !url.starts_with("https://") && !url.starts_with("file://") {
            return Ok(ToolOutput::err(format!(
                "webfetch requires an https:// (or file://) url: {}",
                parsed.url
            )));
        }

        let text = if let Some(rest) = url.strip_prefix("file://") {
            match tokio::fs::read_to_string(rest).await {
                Ok(c) => c,
                Err(e) => return Ok(ToolOutput::err(format!("read error: {e}"))),
            }
        } else {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .map_err(|e| crate::error::AppError::Other(e.to_string()))?;
            let resp = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => return Ok(ToolOutput::err(format!("fetch failed: {e}"))),
            };
            let status = resp.status();
            let ctype = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let body = match resp.text().await {
                Ok(t) => t,
                Err(e) => return Ok(ToolOutput::err(format!("read failed: {e}"))),
            };
            if !status.is_success() {
                return Ok(ToolOutput::err(format!(
                    "HTTP {status}: {}",
                    body.chars().take(500).collect::<String>()
                )));
            }
            if !ctype.contains("text") && !ctype.contains("html") && !ctype.contains("xml") {
                return Ok(ToolOutput::err(format!(
                    "non-text content-type: {ctype}"
                )));
            }
            body
        };

        let converted = html_to_text(&text);
        // Cap to a reasonable size to avoid blowing context.
        let max = 32_000usize;
        let converted = if converted.len() > max {
            let mut s = converted[..max].to_string();
            s.push_str("\n...[truncated]");
            s
        } else {
            converted
        };
        // Return the converted text; the main loop's LLM answers `prompt`.
        // Prefix with the prompt for traceability.
        let out = format!("prompt: {}\n\n{}", parsed.prompt, converted);
        Ok(ToolOutput::ok(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_text_strips_tags() {
        let html = "<html><head><script>bad()</script></head><body><p>Hello <b>world</b> &amp; friends</p></body></html>";
        let t = html_to_text(html);
        assert!(t.contains("Hello world & friends"));
        assert!(!t.contains("bad()"));
        assert!(!t.contains("<"));
    }

    #[test]
    fn html_to_text_collapses_whitespace() {
        let html = "<p>a</p>\n\n\n<p>b</p>";
        let t = html_to_text(html);
        assert!(!t.contains("\n\n\n"));
    }
}
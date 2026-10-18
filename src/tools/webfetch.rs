use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

/// The whole request — connect, response and body read — is bounded so a
/// stalled or silent server cannot park the turn forever. A turn that never
/// ends is the worst failure mode this tool can cause: the agent's input is
/// strictly serial, so every later prompt (including peer prompts) sits
/// unprocessed for as long as the invocation blocks.
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

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

/// Fetch one http(s) URL to text, bounded by `timeout` across connect,
/// response and body read. Errors keep the report texts the tool showed
/// before ("fetch failed", "read failed", "HTTP <status>", "non-text
/// content-type") so a caller cannot tell the inline path from this one.
async fn http_body(url: &str, timeout: Duration) -> Result<String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        // Covers connect, response and the (streamed) body read; a server
        // that accepts and then trickles or stalls forever surfaces here as
        // "timed out" instead of an immortal call.
        .timeout(timeout)
        .build()
        .map_err(|e| crate::error::AppError::Other(e.to_string()))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| crate::error::AppError::Other(format!("fetch failed: {e}")))?;
    let status = resp.status();
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp
        .text()
        .await
        .map_err(|e| crate::error::AppError::Other(format!("read failed: {e}")))?;
    if !status.is_success() {
        return Err(crate::error::AppError::Other(format!(
            "HTTP {status}: {}",
            body.chars().take(500).collect::<String>()
        )));
    }
    if !ctype.contains("text") && !ctype.contains("html") && !ctype.contains("xml") {
        return Err(crate::error::AppError::Other(format!(
            "non-text content-type: {ctype}"
        )));
    }
    Ok(body)
}

#[async_trait]
impl Tool for WebFetchTool {    fn name(&self) -> &str {
        "webfetch"
    }

    fn description(&self) -> &str {
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
            match http_body(&url, FETCH_TIMEOUT).await {
                Ok(t) => t,
                Err(e) => return Ok(ToolOutput::err(e.to_string())),
            }
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

    /// A server that accepts, answers headers, and then never completes the
    /// body must surface as "timed out" within the bounded window — the
    /// pin for the fetch timeout's reason to exist (a stall here used to
    /// park the whole turn, and with it every queued prompt behind it).
    #[tokio::test]
    async fn http_body_times_out_on_a_stalled_body() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/html\r\n\r\n<html>half</")
                .unwrap();
            sock.flush().unwrap();
            // Hold the connection open forever: the body never completes, and
            // the thread is parked rather than shut down so the client's
            // timeout — not this thread's death — is what ends the exchange
            // deterministically. The handle is never joined; detaching it
            // leaves one parked thread behind for the test binary's lifetime.
            loop {
                std::thread::park();
            }
        });
        let started = std::time::Instant::now();
        let out = http_body(&format!("http://127.0.0.1:{port}/"), Duration::from_secs(1)).await;
        let elapsed = started.elapsed();
        // The 1 s window is 60x smaller than the production constant; the
        // generous wall-clock bound only excludes an unbounded hang (what the
        // call did before the timeout), not scheduling jitter under a fully
        // parallel test run.
        assert!(
            elapsed < Duration::from_secs(30),
            "expected the bounded timeout to fire, took {elapsed:?}"
        );
        let message = if let Err(e) = out {
            e.to_string()
        } else {
            panic!("expected the stalled body to fail, not succeed")
        };
        // reqwest reports the elapsed timeout as a request/body error; the
        // assertion that matters is only that the call ENDED bounded and with
        // the fetch-failed report instead of hanging.
        assert!(
            message.contains("fetch failed") || message.contains("read failed"),
            "expected the bounded fetch to fail, got {message}"
        );
        std::mem::forget(server); // parked; never joined
    }
}
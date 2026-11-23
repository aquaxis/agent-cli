use futures::stream::{Stream, StreamExt};

use crate::ai::{EventStream, ProviderEvent};

/// Result of interpreting one SSE frame: the events it produced and whether
/// the stream is finished (no further frames should be read).
pub(crate) struct FrameOutcome {
    pub events: Vec<ProviderEvent>,
    pub done: bool,
}

/// Turn an HTTP body byte stream into a `ProviderEvent` stream: each chunk is
/// decoded (lossily) as UTF-8, split into SSE frames by `SseAccumulator`, and
/// every frame handed to `parse`. Events are yielded in order; a frame whose
/// outcome is `done` ends the stream immediately. A transport error yields one
/// `Error` (with `provider`/`model` context) and ends the stream; a body that
/// ends without a terminating frame yields a final `Done`.
pub(crate) fn sse_events<'a, S, B, E, F>(
    bytes: S,
    provider: &'a str,
    model: &'a str,
    mut parse: F,
) -> EventStream<'a>
where
    S: Stream<Item = std::result::Result<B, E>> + Send + 'a,
    B: AsRef<[u8]> + Send + 'a,
    E: std::fmt::Display + Send + 'a,
    F: FnMut(&str) -> FrameOutcome + Send + 'a,
{
    Box::pin(async_stream::stream! {
        let mut acc = SseAccumulator::new();
        let mut utf8 = Utf8Carry::default();
        let mut bytes = Box::pin(bytes);
        while let Some(chunk) = bytes.next().await {
            match chunk {
                Ok(chunk) => {
                    let s = utf8.push(chunk.as_ref());
                    acc.push(&s);
                    for frame in acc.drain_frames() {
                        let outcome = parse(&frame);
                        for ev in outcome.events {
                            yield ev;
                        }
                        if outcome.done {
                            return;
                        }
                    }
                }
                Err(e) => {
                    yield ProviderEvent::Error {
                        message: crate::ai::stream_mid_body(provider, model, &e.to_string()),
                    };
                    return;
                }
            }
        }
        yield ProviderEvent::Done;
    })
}

/// Decodes a byte stream as UTF-8 across chunk boundaries. A network chunk can end in the
/// middle of a multi-byte character (any CJK character is three bytes); decoding each
/// chunk on its own turned both halves into U+FFFD, and the replacement characters landed
/// in the model's text and in the files an agent wrote from it. The incomplete tail is
/// held back and completed by the next chunk; only bytes that can never form a character
/// become U+FFFD.
#[derive(Debug, Default)]
pub(crate) struct Utf8Carry {
    pending: Vec<u8>,
}

impl Utf8Carry {
    /// The text the bytes received so far complete.
    pub(crate) fn push(&mut self, chunk: &[u8]) -> String {
        self.pending.extend_from_slice(chunk);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    out.push_str(text);
                    self.pending.clear();
                    return out;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    out.push_str(std::str::from_utf8(&self.pending[..valid]).unwrap_or_default());
                    match error.error_len() {
                        // An incomplete sequence at the end: wait for the next chunk.
                        None => {
                            self.pending.drain(..valid);
                            return out;
                        }
                        // Bytes that can never be a character: replace them, go on.
                        Some(bad) => {
                            out.push(char::REPLACEMENT_CHARACTER);
                            self.pending.drain(..valid + bad);
                        }
                    }
                }
            }
        }
    }
}

/// Iterator-style helper that extracts SSE frames.
/// Accumulates bytes from the server line by line and yields "data: ..." blocks separated by blank lines.
pub struct SseAccumulator {
    buf: String,
}

impl SseAccumulator {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    pub fn push(&mut self, chunk: &str) {
        self.buf.push_str(chunk);
    }

    /// Return completed frames in order. Each element is the concatenation of `data:` lines with the prefix stripped.
    pub fn drain_frames(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(idx) = self.buf.find("\n\n") {
            let frame = self.buf[..idx].to_string();
            self.buf.drain(..=idx + 1);
            let mut data = String::new();
            for line in frame.lines() {
                if let Some(rest) = line.strip_prefix("data:") {
                    let rest = rest.strip_prefix(' ').unwrap_or(rest);
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(rest);
                }
            }
            if !data.is_empty() {
                out.push(data);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    /// A character split across two chunks is decoded whole, never as U+FFFD.
    #[test]
    fn a_character_split_across_chunks_is_decoded_whole() {
        let text = "節末の演習";
        let bytes = text.as_bytes();
        let mut carry = super::Utf8Carry::default();
        let mut out = String::new();
        for chunk in bytes.chunks(2) {
            out.push_str(&carry.push(chunk));
        }
        assert_eq!(out, text);
        // Bytes that can never form a character still become one U+FFFD each.
        let mut carry = super::Utf8Carry::default();
        assert_eq!(carry.push(&[b'a', 0xFF, b'b']), "a\u{FFFD}b");
    }

    use super::*;

    #[test]
    fn sse_accumulator_extracts_frames() {
        let mut a = SseAccumulator::new();
        a.push("data: hello\n\ndata: world\n\n");
        let frames = a.drain_frames();
        assert_eq!(frames, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn sse_accumulator_handles_partial() {
        let mut a = SseAccumulator::new();
        a.push("data: par");
        assert!(a.drain_frames().is_empty());
        a.push("tial\n\n");
        let frames = a.drain_frames();
        assert_eq!(frames, vec!["partial".to_string()]);
    }

    #[test]
    fn sse_accumulator_strips_data_prefix_with_no_space() {
        let mut a = SseAccumulator::new();
        a.push("data:no-space\n\n");
        let frames = a.drain_frames();
        assert_eq!(frames, vec!["no-space".to_string()]);
    }

    #[test]
    fn sse_accumulator_concatenates_multi_data_lines() {
        let mut a = SseAccumulator::new();
        a.push("data: foo\ndata: bar\n\n");
        let frames = a.drain_frames();
        assert_eq!(frames, vec!["foo\nbar".to_string()]);
    }

    #[test]
    fn sse_accumulator_ignores_event_only_frames() {
        let mut a = SseAccumulator::new();
        a.push("event: ping\n\n");
        let frames = a.drain_frames();
        assert!(frames.is_empty());
    }
}

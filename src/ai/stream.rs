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

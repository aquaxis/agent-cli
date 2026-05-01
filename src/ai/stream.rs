use bytes::Bytes;
use futures::stream::{Stream, StreamExt};

/// SSE フレームを抽出するイテレータ風ヘルパー。
/// サーバから送られたバイト列を行単位で連結し、空行で区切られた "data: ..." を取り出す。
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

    /// 完成したフレームを順に返す。各要素は `data:` 行を連結した文字列（プレフィックスは除去済み）。
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

#[allow(dead_code)]
pub fn bytes_to_lines<S>(s: S) -> impl Stream<Item = std::io::Result<String>>
where
    S: Stream<Item = reqwest::Result<Bytes>>,
{
    let mut buf = String::new();
    s.flat_map(move |chunk| {
        let mut out: Vec<std::io::Result<String>> = Vec::new();
        match chunk {
            Ok(bytes) => match std::str::from_utf8(&bytes) {
                Ok(s) => {
                    buf.push_str(s);
                    while let Some(idx) = buf.find('\n') {
                        let line: String = buf.drain(..=idx).collect();
                        out.push(Ok(line.trim_end_matches('\n').to_string()));
                    }
                }
                Err(e) => out.push(Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))),
            },
            Err(e) => out.push(Err(std::io::Error::other(e.to_string()))),
        }
        futures::stream::iter(out)
    })
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

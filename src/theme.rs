//! Terminal colour scheme.
//!
//! The palette is written down exactly once, as the mapping from a [`Role`] —
//! *what a piece of text is* — to an ANSI 16 colour and a set of attributes.
//! Call sites name the role, never the colour, so the whole scheme can be read
//! (and changed) in one table.
//!
//! Two rules govern how it is used:
//!
//! * **Paint last.** The column maths in [`crate::editor`] counts every
//!   character, so an escape sequence reaching it would corrupt truncation,
//!   wrapping and the prompt's cursor column. Text is therefore styled only
//!   after it has been measured, cut and wrapped.
//! * **Self-closing.** Every styled string ends with a reset, so no colour can
//!   leak into later output or into a region the display erases.
//!
//! Colour is decided once per output stream at startup ([`Theme::from_env`]);
//! when a stream has no colour, styling is the identity function and not one
//! byte changes.

use std::borrow::Cow;

/// What a piece of terminal text *is*. Each variant maps to one ANSI 16 colour
/// and a set of attributes; that mapping ([`sgr`]) is the entire palette.
///
/// There is deliberately no role without a call site: elements the REPL does
/// not render (markdown emphasis, code blocks, diffs, separators) get no entry
/// here until something actually draws them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The prompt symbol (`> `) at the start of an input line.
    PromptSymbol,
    /// The line being edited, and its echo when submitted.
    InputText,
    /// A past input replayed by `/history`.
    HistoryEntry,
    /// The `[answer]` marker opening the model's reply.
    AnswerMarker,
    /// `[tool-call]` up to and including the tool name: what is happening now.
    ToolName,
    /// The arguments of a tool call.
    ToolArgs,
    /// A `[tool-result …]` line: skimmable output.
    ToolOutput,
    /// Reasoning — the `[thinking]` marker, its inline text, and the live rows
    /// under the progress indicator.
    Thinking,
    /// The spinner, the elapsed time and the `… +N more` markers.
    Progress,
    /// A completed turn (`✔`).
    Success,
    /// A failed turn (`✗`) and `[error]` messages.
    Failure,
    /// `[info]` and `[auto]` notices.
    Info,
    /// Anything asking the user to answer: `[tool approval]` and `approve?`.
    Confirm,
    /// `[cancelled]`.
    Cancelled,
    /// The slash-command suggestion row above the prompt.
    Hint,
    /// The startup banner.
    Banner,
    /// The banner's detail rows (id, name, provider, …).
    BannerDetail,
}

/// SGR parameters of a role: the foreground colour code (ANSI 16 only — 30–37
/// and 90–97) and the attributes applied with it (`1` bold, `2` dim).
///
/// No background colour and no `reverse`: only the foreground is set, so the
/// terminal's own palette decides the shades and the scheme stays readable on
/// both light and dark backgrounds.
fn sgr(role: Role) -> (Option<u8>, &'static [u8]) {
    const BOLD: &[u8] = &[1];
    const DIM: &[u8] = &[2];
    const NONE: &[u8] = &[];

    const RED: u8 = 31;
    const GREEN: u8 = 32;
    const YELLOW: u8 = 33;
    const BLUE: u8 = 34;
    const MAGENTA: u8 = 35;
    const CYAN: u8 = 36;
    const BRIGHT_BLACK: u8 = 90;

    match role {
        Role::PromptSymbol => (Some(CYAN), BOLD),
        Role::InputText => (None, BOLD),
        Role::HistoryEntry => (None, DIM),
        Role::AnswerMarker => (Some(MAGENTA), BOLD),
        Role::ToolName => (Some(CYAN), BOLD),
        Role::ToolArgs => (Some(BRIGHT_BLACK), NONE),
        Role::ToolOutput => (Some(BRIGHT_BLACK), DIM),
        Role::Thinking => (Some(BRIGHT_BLACK), DIM),
        Role::Progress => (Some(BRIGHT_BLACK), DIM),
        Role::Success => (Some(GREEN), BOLD),
        Role::Failure => (Some(RED), BOLD),
        Role::Info => (Some(BLUE), NONE),
        Role::Confirm => (Some(YELLOW), BOLD),
        Role::Cancelled => (Some(BRIGHT_BLACK), DIM),
        Role::Hint => (Some(BRIGHT_BLACK), DIM),
        Role::Banner => (Some(MAGENTA), BOLD),
        Role::BannerDetail => (None, DIM),
    }
}

/// Wrap `text` in the SGR sequence of `role`, closing it with a reset.
///
/// Attributes come first, then the colour, so the parameters read the same way
/// the palette table does. Empty text yields an empty string: a bare
/// set/reset pair would be written for nothing.
pub fn paint(role: Role, text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let (color, attrs) = sgr(role);
    let mut params = String::new();
    for a in attrs {
        if !params.is_empty() {
            params.push(';');
        }
        params.push_str(&a.to_string());
    }
    if let Some(c) = color {
        if !params.is_empty() {
            params.push(';');
        }
        params.push_str(&c.to_string());
    }
    format!("\u{1b}[{params}m{text}\u{1b}[0m")
}

/// `[ui] color`: when colour is written at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Colour an output stream when it is an interactive terminal (and the
    /// environment does not object).
    #[default]
    Auto,
    /// Always colour, even when the output is redirected.
    Always,
    /// Never colour.
    Never,
}

impl ColorMode {
    /// Parse the config value. An unrecognised string falls back to the
    /// default, exactly as `show_thinking` does for an unknown mode.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "always" => ColorMode::Always,
            "never" => ColorMode::Never,
            _ => ColorMode::Auto,
        }
    }
}

/// Whether colour is written to one stream.
///
/// `Always` and `Never` are explicit instructions from the user's own config
/// and are obeyed as given. `Auto` asks the environment: the stream has to be a
/// terminal, `NO_COLOR` has to be unset or empty, and `TERM` has to name a
/// terminal that can do more than teletype output.
///
/// Pure, so the whole truth table is a unit test.
pub fn resolve(mode: ColorMode, is_tty: bool, no_color: Option<&str>, term: Option<&str>) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            if !is_tty {
                return false;
            }
            if no_color.is_some_and(|v| !v.is_empty()) {
                return false;
            }
            matches!(term, Some(t) if !t.is_empty() && t != "dumb")
        }
    }
}

/// The colour decision for the two output streams, made once at startup.
///
/// The streams are decided separately: `agent-cli run > answer.txt` from a
/// terminal keeps a coloured status display on stderr while the captured answer
/// stays clean.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub stdout: bool,
    pub stderr: bool,
}

impl Theme {
    /// Resolve `mode` against this process' own streams and environment.
    pub fn from_env(mode: ColorMode) -> Self {
        use std::io::IsTerminal;
        let no_color = std::env::var("NO_COLOR").ok();
        let term = std::env::var("TERM").ok();
        Self {
            stdout: resolve(
                mode,
                std::io::stdout().is_terminal(),
                no_color.as_deref(),
                term.as_deref(),
            ),
            stderr: resolve(
                mode,
                std::io::stderr().is_terminal(),
                no_color.as_deref(),
                term.as_deref(),
            ),
        }
    }

    /// A theme that never colours anything: serve mode, piped output, tests.
    pub fn plain() -> Self {
        Self {
            stdout: false,
            stderr: false,
        }
    }

    /// Style `text` for stdout. Returns it borrowed and unchanged when stdout
    /// has no colour, so the disabled path allocates nothing.
    pub fn out<'a>(&self, role: Role, text: &'a str) -> Cow<'a, str> {
        Self::apply(self.stdout, role, text)
    }

    /// Style `text` for stderr.
    pub fn err<'a>(&self, role: Role, text: &'a str) -> Cow<'a, str> {
        Self::apply(self.stderr, role, text)
    }

    fn apply(on: bool, role: Role, text: &str) -> Cow<'_, str> {
        if on {
            Cow::Owned(paint(role, text))
        } else {
            Cow::Borrowed(text)
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::plain()
    }
}

/// Remove every SGR (colour/attribute) sequence, leaving the printable text and
/// any other escape sequence — cursor moves and erases — in place. Test-only:
/// it lets an assertion separate "what was drawn" from "how it was coloured".
#[cfg(test)]
pub(crate) fn strip_sgr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(esc) = rest.find('\u{1b}') {
        out.push_str(&rest[..esc]);
        let after = &rest[esc..];
        let params_and_final = after.strip_prefix("\u{1b}[").and_then(|body| {
            let end = body.find(|c: char| !c.is_ascii_digit() && c != ';')?;
            Some((&body[..end], body.as_bytes()[end], end))
        });
        match params_and_final {
            // A colour/attribute sequence: drop it.
            Some((_, b'm', end)) => rest = &after[2 + end + 1..],
            // Any other escape (cursor move, erase) is part of the drawing.
            _ => {
                out.push('\u{1b}');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Whether `s` contains any SGR sequence at all.
#[cfg(test)]
pub(crate) fn has_sgr(s: &str) -> bool {
    strip_sgr(s) != s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every role, so the palette table itself is covered.
    const ALL_ROLES: [Role; 17] = [
        Role::PromptSymbol,
        Role::InputText,
        Role::HistoryEntry,
        Role::AnswerMarker,
        Role::ToolName,
        Role::ToolArgs,
        Role::ToolOutput,
        Role::Thinking,
        Role::Progress,
        Role::Success,
        Role::Failure,
        Role::Info,
        Role::Confirm,
        Role::Cancelled,
        Role::Hint,
        Role::Banner,
        Role::BannerDetail,
    ];

    #[test]
    fn strip_sgr_removes_colours_and_keeps_the_drawing() {
        assert_eq!(strip_sgr(&paint(Role::Success, "✔ 1.0s")), "✔ 1.0s");
        assert_eq!(strip_sgr("plain"), "plain");
        // Cursor moves and erases are not colour and must survive.
        assert_eq!(strip_sgr("\u{1b}[2A\u{1b}[0Jx"), "\u{1b}[2A\u{1b}[0Jx");
        assert_eq!(
            strip_sgr(&format!("\u{1b}[0J{}", paint(Role::Info, "hi"))),
            "\u{1b}[0Jhi"
        );
        assert!(has_sgr(&paint(Role::Info, "hi")));
        assert!(!has_sgr("\u{1b}[2Aplain"));
    }

    #[test]
    fn paint_renders_the_palette_entry_of_each_role() {
        assert_eq!(paint(Role::PromptSymbol, "> "), "\u{1b}[1;36m> \u{1b}[0m");
        assert_eq!(paint(Role::InputText, "hi"), "\u{1b}[1mhi\u{1b}[0m");
        assert_eq!(paint(Role::HistoryEntry, "hi"), "\u{1b}[2mhi\u{1b}[0m");
        assert_eq!(
            paint(Role::AnswerMarker, "[answer]"),
            "\u{1b}[1;35m[answer]\u{1b}[0m"
        );
        assert_eq!(paint(Role::ToolArgs, "{}"), "\u{1b}[90m{}\u{1b}[0m");
        assert_eq!(paint(Role::ToolOutput, "x"), "\u{1b}[2;90mx\u{1b}[0m");
        assert_eq!(paint(Role::Success, "✔"), "\u{1b}[1;32m✔\u{1b}[0m");
        assert_eq!(paint(Role::Failure, "✗"), "\u{1b}[1;31m✗\u{1b}[0m");
        assert_eq!(paint(Role::Info, "i"), "\u{1b}[34mi\u{1b}[0m");
        assert_eq!(paint(Role::Confirm, "y/N"), "\u{1b}[1;33my/N\u{1b}[0m");
        assert_eq!(paint(Role::Banner, "b"), "\u{1b}[1;35mb\u{1b}[0m");
        assert_eq!(paint(Role::BannerDetail, "d"), "\u{1b}[2md\u{1b}[0m");
    }

    #[test]
    fn every_role_is_self_closing_and_uses_ansi_16_only() {
        for role in ALL_ROLES {
            let painted = paint(role, "x");
            assert!(
                painted.starts_with("\u{1b}[") && painted.ends_with("\u{1b}[0m"),
                "{role:?} must open and close its sequence: {painted:?}"
            );
            let (color, _) = sgr(role);
            if let Some(c) = color {
                assert!(
                    (30..=37).contains(&c) || (90..=97).contains(&c),
                    "{role:?} must use an ANSI 16 foreground colour, got {c}"
                );
            }
        }
    }

    #[test]
    fn paint_of_empty_text_writes_nothing() {
        for role in ALL_ROLES {
            assert_eq!(paint(role, ""), "");
        }
    }

    #[test]
    fn styling_never_changes_the_printable_width() {
        for role in ALL_ROLES {
            for s in ["", "hello", "日本語のテキスト", "⠹ 12.4s", "… +3 more"] {
                assert_eq!(
                    crate::editor::str_display_width(&strip_sgr(&paint(role, s))),
                    crate::editor::str_display_width(s),
                    "{role:?} changed the printable width of {s:?}"
                );
            }
        }
    }

    #[test]
    fn color_mode_parses_and_falls_back_to_auto() {
        assert_eq!(ColorMode::parse("auto"), ColorMode::Auto);
        assert_eq!(ColorMode::parse("always"), ColorMode::Always);
        assert_eq!(ColorMode::parse("never"), ColorMode::Never);
        assert_eq!(ColorMode::parse("ALWAYS"), ColorMode::Always);
        assert_eq!(ColorMode::parse(" never "), ColorMode::Never);
        assert_eq!(ColorMode::parse("rainbow"), ColorMode::Auto);
        assert_eq!(ColorMode::parse(""), ColorMode::Auto);
        assert_eq!(ColorMode::default(), ColorMode::Auto);
    }

    #[test]
    fn resolve_always_and_never_ignore_the_environment() {
        let term = Some("xterm-256color");
        assert!(resolve(ColorMode::Always, false, Some("1"), Some("dumb")));
        assert!(!resolve(ColorMode::Never, true, None, term));
    }

    #[test]
    fn resolve_auto_requires_a_terminal() {
        let term = Some("xterm-256color");
        assert!(resolve(ColorMode::Auto, true, None, term));
        assert!(!resolve(ColorMode::Auto, false, None, term));
    }

    #[test]
    fn resolve_auto_honours_no_color_and_term() {
        let term = Some("xterm-256color");
        // NO_COLOR set to anything non-empty disables colour.
        assert!(!resolve(ColorMode::Auto, true, Some("1"), term));
        assert!(!resolve(ColorMode::Auto, true, Some("0"), term));
        // Set but empty is not a request to disable.
        assert!(resolve(ColorMode::Auto, true, Some(""), term));
        // A terminal that cannot do more than teletype output.
        assert!(!resolve(ColorMode::Auto, true, None, Some("dumb")));
        assert!(!resolve(ColorMode::Auto, true, None, Some("")));
        assert!(!resolve(ColorMode::Auto, true, None, None));
    }

    #[test]
    fn a_plain_theme_returns_the_text_untouched() {
        let theme = Theme::plain();
        for role in ALL_ROLES {
            assert_eq!(theme.out(role, "text"), "text");
            assert_eq!(theme.err(role, "text"), "text");
            assert!(matches!(theme.out(role, "text"), Cow::Borrowed(_)));
        }
    }

    #[test]
    fn the_streams_are_coloured_independently() {
        let theme = Theme {
            stdout: false,
            stderr: true,
        };
        assert_eq!(theme.out(Role::Info, "x"), "x");
        assert_eq!(theme.err(Role::Info, "x"), paint(Role::Info, "x"));
    }
}

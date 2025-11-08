use std::path::Path;

use once_cell::sync::Lazy;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
    util::as_24_bit_terminal_escaped,
};

// Load the built-in syntax/theme sets once per process.
static SYNTAXES: Lazy<SyntaxSet> =
    Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME: Lazy<Theme> = Lazy::new(|| {
    let themes = ThemeSet::load_defaults();
    themes
        .themes
        .get("base16-ocean.dark")
        .cloned()
        .or_else(|| themes.themes.values().next().cloned())
        .unwrap_or_else(Theme::default)
});

pub struct CodeHighlighter<'a> {
    inner: HighlightLines<'a>,
}

impl CodeHighlighter<'static> {
    pub fn new(filename_hint: Option<&str>) -> Self {
        let syntax = syntax_for_hint(filename_hint);
        Self {
            inner: HighlightLines::new(syntax, &THEME),
        }
    }

    pub fn highlight_line(&mut self, line: &str) -> String {
        let ranges = self
            .inner
            .highlight_line(line, &SYNTAXES)
            .unwrap_or_else(|_| vec![(Style::default(), line)]);
        // Use standard 8/16 ANSI colors so user terminal themes stay in control.
        as_24_bit_terminal_escaped(&ranges, false)
    }
}

fn syntax_for_hint(hint: Option<&str>) -> &'static SyntaxReference {
    let Some(name) = hint else {
        return SYNTAXES.find_syntax_plain_text();
    };
    if let Some(syntax) = SYNTAXES.find_syntax_by_path(name) {
        return syntax;
    }
    if let Some(ext) = Path::new(name).extension().and_then(|s| s.to_str()) {
        if let Some(syntax) = SYNTAXES.find_syntax_by_extension(ext) {
            return syntax;
        }
    }
    SYNTAXES.find_syntax_plain_text()
}

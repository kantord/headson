#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct OutputStats {
    pub bytes: usize,
    pub chars: usize,
    pub lines: usize,
}

#[inline]
fn count_lines_from_bytes(b: &[u8]) -> usize {
    if b.is_empty() {
        return 0;
    }
    let mut i = 0usize;
    let mut breaks = 0usize;
    while i < b.len() {
        match b[i] {
            b'\n' => {
                breaks += 1;
                i += 1;
            }
            b'\r' => {
                breaks += 1;
                if i + 1 < b.len() && b[i + 1] == b'\n' {
                    i += 2; // treat CRLF as a single break
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    breaks + 1
}

/// Count bytes and logical lines in a string, normalizing CRLF/CR/LF.
///
/// Rules:
/// - An empty string has 0 lines.
/// - Otherwise, lines = number of line break sequences + 1.
/// - A CRLF pair counts as a single line break.
pub(crate) fn count_output_stats(s: &str, want_chars: bool) -> OutputStats {
    let bytes = s.len();
    let chars = if want_chars { s.chars().count() } else { 0 };
    let lines = count_lines_from_bytes(s.as_bytes());
    OutputStats {
        bytes,
        chars,
        lines,
    }
}

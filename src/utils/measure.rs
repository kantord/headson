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
    let mut lines = count_line_breaks(b).saturating_add(1);
    if ends_with_break(b) && lines > 0 {
        lines -= 1;
    }
    lines
}

fn count_line_breaks(b: &[u8]) -> usize {
    let mut breaks = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'\n' => {
                breaks += 1;
                i += 1;
            }
            b'\r' => {
                breaks += 1;
                if i + 1 < b.len() && b[i + 1] == b'\n' {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    breaks
}

fn ends_with_break(b: &[u8]) -> bool {
    b.ends_with(b"\n") || (b.ends_with(b"\r") && !b.ends_with(b"\r\n"))
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

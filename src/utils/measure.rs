#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct OutputStats {
    pub bytes: usize,
    pub lines: usize,
}

/// Count bytes and logical lines in a string, normalizing CRLF/CR/LF.
///
/// Rules:
/// - An empty string has 0 lines.
/// - Otherwise, lines = number of line break sequences + 1.
/// - A CRLF pair counts as a single line break.
pub(crate) fn count_output_stats(s: &str) -> OutputStats {
    let bytes = s.len();
    if bytes == 0 {
        return OutputStats { bytes, lines: 0 };
    }
    let b = s.as_bytes();
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
    OutputStats {
        bytes,
        lines: breaks + 1,
    }
}

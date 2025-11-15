use unicode_segmentation::UnicodeSegmentation;

/// Return the first `n` Unicode grapheme clusters of `s` without splitting
/// user‑visible characters (e.g., emoji, combining marks).
pub(crate) fn take_n_graphemes(s: &str, n: usize) -> String {
    let mut out = String::new();
    for (i, g) in UnicodeSegmentation::graphemes(s, true).enumerate() {
        if i >= n {
            break;
        }
        out.push_str(g);
    }
    out
}

/// Truncate to at most `n` graphemes, appending a Unicode ellipsis when input
/// is longer.
pub(crate) fn truncate_at_n_graphemes(s: &str, n: usize) -> String {
    let mut out = String::new();
    let mut iter = UnicodeSegmentation::graphemes(s, true);
    for (i, g) in iter.by_ref().enumerate() {
        if i >= n {
            break;
        }
        out.push_str(g);
    }
    if iter.next().is_some() {
        out.push('…');
    }
    out
}

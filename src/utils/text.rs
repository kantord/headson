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

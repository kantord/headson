/// Produce a valid JSON string literal for `s` (including surrounding quotes
/// and necessary escapes). In practice serde_json should not fail here, but we
/// keep a conservative fallback to plain quoting.
pub(crate) fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

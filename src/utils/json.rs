/// Produce a valid JSON string literal for `s` (including surrounding quotes
/// and necessary escapes). Serialization for `&str` is infallible; treat
/// failures as unreachable to avoid masking logic bugs.
pub(crate) fn json_string(s: &str) -> String {
    match serde_json::to_string(s) {
        Ok(v) => v,
        Err(_) => {
            unreachable!("serde_json::to_string(&str) should be infallible")
        }
    }
}

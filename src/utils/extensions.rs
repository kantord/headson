pub fn is_code_like_name(name: &str) -> bool {
    // Determine if a filename looks like source code we want to treat as
    // atomic lines (no mid-line truncation) in text mode.
    let lower_ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    matches!(
        lower_ext.as_deref(),
        Some("c")
            | Some("h")
            | Some("cpp")
            | Some("cc")
            | Some("cxx")
            | Some("hpp")
            | Some("py")
            | Some("java")
            | Some("js")
            | Some("ts")
            | Some("tsx")
            | Some("go")
            | Some("sh")
            | Some("bash")
    )
}

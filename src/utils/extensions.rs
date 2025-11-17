// Popular programming language extensions (lowercase only).
const CODE_EXTS: &[&str] = &[
    // C/C++ and variants
    "c", "h", "cc", "cpp", "cxx", "c++", "hh", "hpp", "hxx", "h++", "ipp",
    "inl", "tpp", "cu", "cuh", // Objective‑C / Objective‑C++
    "m", "mm", // Rust
    "rs", // Go
    "go", // Java & JVM family
    "java", "kt", "kts", "scala", "sc", "groovy", "gvy",   // Swift
    "swift", // .NET languages
    "cs", "fs", "fsx", "fsi", "vb", // Scripting languages
    "py", "pyw", "pyi", "rb", "rake", "php", "phpt", "phtml", "php3", "php4",
    "php5", "php7", "pl", "pm", "t", "sh", "bash", "bsh", "zsh", "ksh", "ps1",
    "psm1", "psd1", // JavaScript / TypeScript and friends
    "js", "mjs", "cjs", "jsx", "ts", "tsx", "coffee", "cjsx",
    // Functional languages
    "hs", "lhs", "erl", "hrl", "ex", "exs", "clj", "cljs", "cljc", "lisp",
    "el", "scm", "ss", "rkt", // ML family
    "ml", "mli", "mll", "mly", "re", "rei", "sml", "sig",
    // Systems / compiled languages
    "d", "di", "nim", "zig", "v", "vsh", "cr", "vala", "vapi", "hx", "chpl",
    "idr", "fut", "sol", "move",
    // Data/query languages often used as code
    "sql", "psql", "plsql", "graphql", "gql", "proto", "thrift",
    // Config-like but code-y
    "lua", "elm", "purs", "dart", "tf", "hcl",
    // Scientific / scripting
    "r", "jl", // Fortran
    "f", "for", "f77", "f90", "f95", "f03", "f08", // Assembly
    "asm", "s",
    // Docs with code blocks; keep them in code mode for highlighting/layout
    "md", "markdown", "mdown", "mkdn", "mkd", "mdwn", "mdtext",
];

pub fn is_code_like_name(name: &str) -> bool {
    // Determine if a filename looks like source code we want to treat as
    // atomic lines (no mid-line truncation) in text mode.
    let lower_ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match lower_ext.as_deref() {
        Some(ext) => CODE_EXTS.contains(&ext),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_code_like_name;

    #[test]
    fn recognizes_markdown_as_code_like() {
        assert!(is_code_like_name("README.md"));
        assert!(is_code_like_name("notes.MARKDOWN"));
    }

    #[test]
    fn non_code_like_text_stays_false() {
        assert!(!is_code_like_name("notes.txt"));
        assert!(!is_code_like_name("no_extension"));
    }
}

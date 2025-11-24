const CODE_EXTS: &[&str] = &[
    "c", "h", "cc", "cpp", "cxx", "c++", "hh", "hpp", "hxx", "h++", "ipp",
    "inl", "tpp", "cu", "cuh", "m", "mm", "rs", "go", "java", "kt", "kts",
    "scala", "sc", "groovy", "gvy", "swift", "cs", "fs", "fsx", "fsi", "vb",
    "py", "pyw", "pyi", "rb", "rake", "php", "phpt", "phtml", "php3", "php4",
    "php5", "php7", "pl", "pm", "t", "sh", "bash", "bsh", "zsh", "ksh", "ps1",
    "psm1", "psd1", "js", "mjs", "cjs", "jsx", "ts", "tsx", "coffee", "cjsx",
    "hs", "lhs", "erl", "hrl", "ex", "exs", "clj", "cljs", "cljc", "lisp",
    "el", "scm", "ss", "rkt", "ml", "mli", "mll", "mly", "re", "rei", "sml",
    "sig", "d", "di", "nim", "zig", "v", "vsh", "cr", "vala", "vapi", "hx",
    "chpl", "idr", "fut", "sol", "move", "sql", "psql", "plsql", "graphql",
    "gql", "proto", "thrift", "lua", "elm", "purs", "dart", "tf", "hcl", "r",
    "jl", "f", "for", "f77", "f90", "f95", "f03", "f08", "asm", "s", "md",
    "markdown", "mdown", "mkdn", "mkd", "mdwn", "mdtext",
];

pub fn is_code_like_name(name: &str) -> bool {
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

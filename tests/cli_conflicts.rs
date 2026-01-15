mod common;

/// Tests for clap `conflicts_with` constraints
macro_rules! conflict_test {
    ($name:ident, $args:expr) => {
        #[test]
        fn $name() {
            let out = common::run_cli_expect_fail($args, None, None);
            let err_l = out.stderr.to_ascii_lowercase();
            assert!(
                err_l.contains("conflict")
                    || err_l.contains("cannot be used with")
                    || err_l.contains("cannot be used together"),
                "expected conflict error, got: {}",
                out.stderr
            );
        }
    };
}

/// Tests for clap `requires` constraints
macro_rules! requires_test {
    ($name:ident, $args:expr) => {
        #[test]
        fn $name() {
            let out = common::run_cli_expect_fail($args, None, None);
            let err_l = out.stderr.to_ascii_lowercase();
            assert!(
                err_l.contains("require") || err_l.contains("missing"),
                "expected requires error, got: {}",
                out.stderr
            );
        }
    };
}

/// Tests where clap may report either conflict or requires error (order-dependent)
macro_rules! conflict_or_requires_test {
    ($name:ident, $args:expr) => {
        #[test]
        fn $name() {
            let out = common::run_cli_expect_fail($args, None, None);
            let err_l = out.stderr.to_ascii_lowercase();
            assert!(
                err_l.contains("conflict")
                    || err_l.contains("cannot be used with")
                    || err_l.contains("cannot be used together")
                    || err_l.contains("require")
                    || err_l.contains("missing"),
                "expected conflict or requires error, got: {}",
                out.stderr
            );
        }
    };
}

// Category 1: Clap conflicts
conflict_test!(
    head_and_tail,
    &["--head", "--tail", "-n", "20", "-f", "json"]
);
conflict_test!(
    compact_and_no_newline,
    &["--compact", "--no-newline", "-c", "100", "-f", "json"]
);
conflict_test!(
    lines_and_no_newline,
    &["--no-newline", "-n", "3", "-f", "json"]
);
conflict_test!(
    global_lines_and_no_newline,
    &["--no-newline", "-N", "5", "-f", "json"]
);
conflict_test!(
    weak_grep_with_strong_grep,
    &[
        "--grep",
        "foo",
        "--weak-grep",
        "foo",
        "tests/fixtures/explicit/object_small.json"
    ]
);
conflict_test!(
    tree_with_no_header,
    &[
        "--tree",
        "--no-header",
        "tests/fixtures/explicit/object_small.json"
    ]
);
conflict_test!(
    tree_with_compact,
    &[
        "--tree",
        "--compact",
        "tests/fixtures/explicit/object_small.json"
    ]
);
conflict_test!(
    tree_with_no_newline,
    &[
        "--tree",
        "--no-newline",
        "tests/fixtures/explicit/object_small.json"
    ]
);
conflict_test!(recursive_with_glob, &["--recursive", "-g", "*.json"]);
conflict_test!(
    igrep_with_weak_grep,
    &[
        "--igrep",
        "foo",
        "--weak-grep",
        "bar",
        "tests/fixtures/explicit/object_small.json"
    ]
);
conflict_test!(
    igrep_with_iweak_grep,
    &[
        "--igrep",
        "foo",
        "--iweak-grep",
        "bar",
        "tests/fixtures/explicit/object_small.json"
    ]
);
conflict_test!(
    iweak_grep_with_grep,
    &[
        "--grep",
        "foo",
        "--iweak-grep",
        "bar",
        "tests/fixtures/explicit/object_small.json"
    ]
);
// Category 2: Clap requires
requires_test!(
    grep_show_requires_strong_grep,
    &[
        "--grep-show",
        "all",
        "tests/fixtures/explicit/object_small.json"
    ]
);

// Category 3: Clap conflict or requires (order-dependent)
conflict_or_requires_test!(
    grep_show_with_weak_grep,
    &[
        "--weak-grep",
        "foo",
        "--grep-show",
        "all",
        "tests/fixtures/explicit/object_small.json"
    ]
);
conflict_or_requires_test!(
    grep_show_with_iweak_grep,
    &[
        "--iweak-grep",
        "foo",
        "--grep-show",
        "all",
        "tests/fixtures/explicit/object_small.json"
    ]
);

// Category 4: Runtime validation (manual tests)
#[test]
fn tree_rejected_for_stdin() {
    let out = common::run_cli_expect_fail(&["--tree"], Some(b"{}"), None);
    let err_l = out.stderr.to_ascii_lowercase();
    assert!(
        err_l.contains("tree")
            && (err_l.contains("stdin") || err_l.contains("input")),
        "expected tree+stdin error, got: {}",
        out.stderr
    );
}

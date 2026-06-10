//! Explore-session behaviour of the library pipeline: fileset round-robin
//! fairness must survive penalty application, shown-leaf recording must
//! reflect the actually-selected node set, and breadcrumb identity must be
//! per file (resolved absolute path + in-file dot-path).

use headson::{
    Breadcrumb, Budget, BudgetKind, Budgets, ColorMode, ExploreContext,
    FilesetInput, FilesetInputKind, GrepConfig, InputKind, OutputTemplate,
    PriorityConfig, RenderConfig, RenderOutput, Style, headson,
    resolve_breadcrumb_file,
};

fn render_config() -> RenderConfig {
    RenderConfig {
        template: OutputTemplate::Pseudo,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        color_mode: ColorMode::Off,
        color_enabled: false,
        style: Style::Default,
        prefer_tail_arrays: false,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: false,
        fileset_tree: false,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    }
}

const FILE_A: &[u8] = br#"{"a1": 1, "a2": 2, "a3": 3}"#;
const FILE_B: &[u8] = br#"{"b1": 1, "b2": 2, "b3": 3}"#;

fn two_file_fileset() -> InputKind {
    InputKind::Fileset(vec![
        FilesetInput {
            name: "a.json".to_string(),
            bytes: FILE_A.to_vec(),
            kind: FilesetInputKind::Json,
        },
        FilesetInput {
            name: "b.json".to_string(),
            bytes: FILE_B.to_vec(),
            kind: FilesetInputKind::Json,
        },
    ])
}

fn run_input(
    input: InputKind,
    explore: Option<ExploreContext>,
    budgets: Budgets,
) -> RenderOutput {
    let mut prio = PriorityConfig::new(usize::MAX, usize::MAX);
    prio.explore = explore;
    headson(
        input,
        &render_config(),
        &prio,
        &GrepConfig::default(),
        budgets,
    )
    .expect("headson must succeed")
}

fn run_fileset(
    explore: Option<ExploreContext>,
    budgets: Budgets,
) -> RenderOutput {
    run_input(two_file_fileset(), explore, budgets)
}

fn fresh_session(file: Option<String>) -> ExploreContext {
    ExploreContext {
        breadcrumbs: vec![],
        current_step: 0,
        alpha: 0.5,
        file,
    }
}

fn tight_line_budget() -> Budgets {
    Budgets {
        global: Some(Budget {
            kind: BudgetKind::Lines,
            cap: 8,
        }),
        per_slot: None,
    }
}

/// Shown-leaf key for the leaf whose in-file dot-path equals `dot_path`,
/// captured by running the fileset with a fresh (breadcrumb-free) session.
fn captured_key_for(dot_path: &str) -> (String, String) {
    let out = run_fileset(Some(fresh_session(None)), Budgets::default());
    out.shown_leaves
        .iter()
        .find_map(|(file, path)| {
            let prefix = path.split_once('#').map(|(p, _)| p)?;
            (prefix == dot_path).then(|| (file.clone(), path.clone()))
        })
        .unwrap_or_else(|| {
            panic!("no shown leaf at {dot_path:?}; got {:?}", out.shown_leaves)
        })
}

/// A session whose breadcrumbs match nothing must not perturb the fileset
/// round-robin interleave: output is byte-identical to the no-session run.
#[test]
fn fileset_interleave_unchanged_by_non_matching_breadcrumb() {
    let baseline = run_fileset(None, tight_line_budget());
    let with_session = run_fileset(
        Some(ExploreContext {
            breadcrumbs: vec![Breadcrumb {
                file: "/nonexistent/zz.json".to_string(),
                path: "zz.nonexistent#0000000000000000".to_string(),
                count: 3,
                last_step: 1,
            }],
            current_step: 2,
            alpha: 0.5,
            file: None,
        }),
        tight_line_budget(),
    );
    assert_eq!(
        baseline.text, with_session.text,
        "non-matching breadcrumbs must leave fileset output byte-identical"
    );
}

/// An active session with zero breadcrumbs (first invocation) must also
/// leave the output byte-identical to the no-session run.
#[test]
fn fileset_interleave_unchanged_by_empty_breadcrumbs() {
    let baseline = run_fileset(None, tight_line_budget());
    let with_session = run_fileset(
        Some(ExploreContext {
            breadcrumbs: vec![],
            current_step: 1,
            alpha: 0.5,
            file: None,
        }),
        tight_line_budget(),
    );
    assert_eq!(
        baseline.text, with_session.text,
        "empty breadcrumbs must leave fileset output byte-identical"
    );
}

/// With a matching breadcrumb penalizing one leaf in file a, the round-robin
/// interleave must still alternate between files: both files keep content in
/// the output, the penalized leaf yields to its unpenalized siblings.
#[test]
fn fileset_interleave_survives_matching_breadcrumb() {
    let (file, path) = captured_key_for("a1");
    assert_eq!(
        file,
        resolve_breadcrumb_file("a.json"),
        "fileset leaf must carry the resolved absolute path of its file"
    );
    let out = run_fileset(
        Some(ExploreContext {
            breadcrumbs: vec![Breadcrumb {
                file,
                path,
                count: 5,
                last_step: 1,
            }],
            current_step: 2,
            alpha: 0.9,
            file: None,
        }),
        tight_line_budget(),
    );
    assert!(
        out.text.contains("b1"),
        "file b content must survive penalty re-sort; got: {:?}",
        out.text
    );
    assert!(
        out.text.contains("a2") || out.text.contains("a3"),
        "unpenalized file a content must still render; got: {:?}",
        out.text
    );
    assert!(
        !out.text.contains("a1"),
        "penalized leaf must yield under a tight budget; got: {:?}",
        out.text
    );
}

/// Under per-slot caps the budget search selects from a custom round-robin
/// ordering, not `by_priority`. Recorded shown leaves must agree exactly
/// with the leaves visible in the rendered text.
#[test]
fn per_slot_caps_record_exactly_the_rendered_leaves() {
    // Cap 4 fits exactly one leaf per file ({, key, omission, }); the custom
    // per-slot selection order diverges from by_priority here, so recording
    // from by_priority[..top_k] would miss file b's rendered leaf.
    let budgets = Budgets {
        global: None,
        per_slot: Some(Budget {
            kind: BudgetKind::Lines,
            cap: 4,
        }),
    };
    let out = run_fileset(Some(fresh_session(None)), budgets);
    let recorded: Vec<(&str, &str)> = out
        .shown_leaves
        .iter()
        .filter_map(|(file, path)| {
            path.split_once('#').map(|(p, _)| (file.as_str(), p))
        })
        .collect();
    for key in ["a1", "a2", "a3", "b1", "b2", "b3"] {
        let rendered = out.text.contains(key);
        let file = resolve_breadcrumb_file(&format!(
            "{}.json",
            key.chars().next().map(String::from).unwrap_or_default()
        ));
        let was_recorded = recorded.contains(&(file.as_str(), key));
        assert_eq!(
            rendered, was_recorded,
            "leaf {key:?}: rendered={rendered} but recorded={was_recorded}\n\
             text: {:?}\nrecorded: {recorded:?}",
            out.text
        );
    }
    assert!(
        !out.shown_leaves.is_empty(),
        "per-slot run must record at least one shown leaf"
    );
}

// ── Per-file breadcrumb identity ───────────────────────────────────────────

const SHARED_CONTENT: &[u8] = br#"{"a": 1, "b": 2}"#;

fn tight_byte_budget() -> Budgets {
    Budgets {
        global: Some(Budget {
            kind: BudgetKind::Bytes,
            cap: 20,
        }),
        per_slot: None,
    }
}

/// Regression (issue #513 review): breadcrumbs recorded while looking at one
/// file must not penalize an identical-valued leaf in a *different* file on
/// the user's first look at it — and the same breadcrumbs must still
/// penalize the file they were recorded for.
#[test]
fn breadcrumbs_do_not_cross_penalize_identical_files() {
    let file_a = "/abs/fixtures/a.json".to_string();
    let file_b = "/abs/fixtures/b.json".to_string();

    let first = run_input(
        InputKind::Json(SHARED_CONTENT.to_vec()),
        Some(fresh_session(Some(file_a.clone()))),
        tight_byte_budget(),
    );
    assert!(
        first.shown_leaves.iter().all(|(file, _)| file == &file_a),
        "single-file leaves must carry the context file; got {:?}",
        first.shown_leaves
    );
    let crumbs: Vec<Breadcrumb> = first
        .shown_leaves
        .iter()
        .map(|(file, path)| Breadcrumb {
            file: file.clone(),
            path: path.clone(),
            count: 1,
            last_step: 1,
        })
        .collect();
    assert!(!crumbs.is_empty(), "first render must record shown leaves");

    // First look at identically-structured file B: output must match a
    // breadcrumb-free render exactly — no cross-file penalty.
    let b_fresh = run_input(
        InputKind::Json(SHARED_CONTENT.to_vec()),
        Some(fresh_session(Some(file_b.clone()))),
        tight_byte_budget(),
    );
    let b_with_a_crumbs = run_input(
        InputKind::Json(SHARED_CONTENT.to_vec()),
        Some(ExploreContext {
            breadcrumbs: crumbs.clone(),
            current_step: 2,
            alpha: 0.5,
            file: Some(file_b),
        }),
        tight_byte_budget(),
    );
    assert_eq!(
        b_fresh.text, b_with_a_crumbs.text,
        "breadcrumbs from file A must not affect the first look at file B"
    );

    // Sanity: the same breadcrumbs DO penalize file A itself.
    let a_again = run_input(
        InputKind::Json(SHARED_CONTENT.to_vec()),
        Some(ExploreContext {
            breadcrumbs: crumbs,
            current_step: 2,
            alpha: 0.5,
            file: Some(file_a),
        }),
        tight_byte_budget(),
    );
    assert_ne!(
        b_fresh.text, a_again.text,
        "breadcrumbs must still penalize the file they were recorded for"
    );
}

/// The same file produces identical breadcrumb keys whether rendered as a
/// single input or inside a fileset, and regardless of relative path
/// spelling — penalty continuity must survive `hson a.json` vs `hson .`.
#[test]
fn keys_stable_across_single_file_and_fileset_invocations() {
    let bytes = br#"{"k1": "v1", "k2": 42}"#;
    let name = "stable_fixture.json";

    let single = run_input(
        InputKind::Json(bytes.to_vec()),
        Some(fresh_session(Some(resolve_breadcrumb_file(name)))),
        Budgets::default(),
    );
    let fileset_keys_for = |spelled: &str| {
        let out = run_input(
            InputKind::Fileset(vec![FilesetInput {
                name: spelled.to_string(),
                bytes: bytes.to_vec(),
                kind: FilesetInputKind::Json,
            }]),
            Some(fresh_session(None)),
            Budgets::default(),
        );
        let mut keys = out.shown_leaves;
        keys.sort();
        keys
    };

    let mut single_keys = single.shown_leaves;
    single_keys.sort();
    assert!(!single_keys.is_empty(), "must record shown leaves");
    assert_eq!(
        single_keys,
        fileset_keys_for(name),
        "single-file and fileset invocations must produce identical keys"
    );
    assert_eq!(
        single_keys,
        fileset_keys_for(&format!("./{name}")),
        "a different relative spelling must produce identical keys"
    );
}

//! Explore-session behaviour of the library pipeline: fileset round-robin
//! fairness must survive penalty application, and shown-leaf recording must
//! reflect the actually-selected node set.

use headson::{
    Breadcrumb, Budget, BudgetKind, Budgets, ColorMode, ExploreContext,
    FilesetInput, FilesetInputKind, GrepConfig, InputKind, OutputTemplate,
    PriorityConfig, RenderConfig, RenderOutput, Style, headson,
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

fn run_fileset(
    explore: Option<ExploreContext>,
    budgets: Budgets,
) -> RenderOutput {
    let mut prio = PriorityConfig::new(usize::MAX, usize::MAX);
    prio.explore = explore;
    headson(
        two_file_fileset(),
        &render_config(),
        &prio,
        &GrepConfig::default(),
        budgets,
    )
    .expect("headson must succeed")
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

/// Shown-leaf key for the leaf whose dot-path equals `dot_path`, captured by
/// running the fileset with a fresh (breadcrumb-free) session.
fn captured_key_for(dot_path: &str) -> String {
    let out = run_fileset(
        Some(ExploreContext {
            breadcrumbs: vec![],
            current_step: 0,
            alpha: 0.5,
        }),
        Budgets::default(),
    );
    out.shown_leaves
        .iter()
        .find_map(|(_, path)| {
            let prefix = path.split_once('#').map(|(p, _)| p)?;
            (prefix == dot_path).then(|| path.clone())
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
                file: String::new(),
                path: "zz.nonexistent#0000000000000000".to_string(),
                count: 3,
                last_step: 1,
            }],
            current_step: 2,
            alpha: 0.5,
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
    let key_a1 = captured_key_for("a.json.a1");
    let out = run_fileset(
        Some(ExploreContext {
            breadcrumbs: vec![Breadcrumb {
                file: String::new(),
                path: key_a1,
                count: 5,
                last_step: 1,
            }],
            current_step: 2,
            alpha: 0.9,
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
    let out = run_fileset(
        Some(ExploreContext {
            breadcrumbs: vec![],
            current_step: 0,
            alpha: 0.5,
        }),
        budgets,
    );
    let recorded: Vec<&str> = out
        .shown_leaves
        .iter()
        .filter_map(|(_, path)| path.split_once('#').map(|(p, _)| p))
        .collect();
    for key in ["a1", "a2", "a3", "b1", "b2", "b3"] {
        let rendered = out.text.contains(key);
        let dot_path = format!(
            "{}.json.{key}",
            key.chars().next().map(String::from).unwrap_or_default()
        );
        let was_recorded = recorded.contains(&dot_path.as_str());
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

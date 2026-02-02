use anyhow::Result;

use crate::order::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

use crate::InputKind;
use crate::grep::GrepConfig;

pub mod fileset;
pub mod format;
pub mod formats;

pub mod sampling;

#[allow(
    unused_imports,
    reason = "Re-exported helpers need to stay public even when unused internally"
)]
pub use formats::{
    parse_json_one, parse_jsonl_one, parse_text_one_with_mode, parse_yaml_one,
};

#[derive(Debug)]
pub(crate) struct IngestOutput {
    pub arena: TreeArena,
    pub warnings: Vec<String>,
}

/// Return a copy of `cfg` with array sampling disabled when strong grep is
/// active. Non-JSONL formats need this to avoid sampling away matches;
/// JSONL handles it via `merge_required` in the sampler instead.
pub(crate) fn grep_adjusted_cfg(
    cfg: &PriorityConfig,
    grep: &GrepConfig,
) -> PriorityConfig {
    if grep.has_strong() {
        let mut c = *cfg;
        c.array_max_items = usize::MAX;
        c
    } else {
        *cfg
    }
}

/// Build a predicate that returns true for JSONL line indices matching the
/// strong grep pattern. When no grep is active, returns a no-op.
///
/// Uses a single regex scan over the entire text and maps match positions
/// back to line indices, avoiding per-line regex overhead.
pub(crate) fn jsonl_grep_predicate(
    bytes: &[u8],
    grep: &GrepConfig,
) -> Box<dyn Fn(usize) -> bool> {
    let Some(re) = grep.patterns.strong() else {
        return Box::new(|_| false);
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Box::new(|_| false);
    };
    let offsets = formats::json::jsonl_line_offsets(text);
    if offsets.is_empty() {
        return Box::new(|_| false);
    }
    // Single regex pass: find all match positions and map to line indices.
    let mut matching = vec![false; offsets.len()];
    for m in re.find_iter(text) {
        let pos = m.start();
        // Binary search for the line containing this byte position.
        let idx = offsets.partition_point(|&(start, _)| start <= pos);
        if idx > 0 {
            matching[idx - 1] = true;
        }
    }
    Box::new(move |i: usize| matching.get(i).copied().unwrap_or(false))
}

/// Dispatch the appropriate ingest path for any supported input kind.
pub(crate) fn ingest_into_arena(
    input: InputKind,
    priority_cfg: &PriorityConfig,
    grep: &GrepConfig,
) -> Result<IngestOutput> {
    match input {
        InputKind::Json(bytes) => {
            let cfg = grep_adjusted_cfg(priority_cfg, grep);
            parse_json_one(bytes, &cfg).map(|arena| IngestOutput {
                arena,
                warnings: Vec::new(),
            })
        }
        InputKind::Jsonl(bytes) => {
            let must_include = jsonl_grep_predicate(&bytes, grep);
            parse_jsonl_one(&bytes, priority_cfg, &*must_include).map(
                |arena| IngestOutput {
                    arena,
                    warnings: Vec::new(),
                },
            )
        }
        InputKind::Yaml(bytes) => {
            let cfg = grep_adjusted_cfg(priority_cfg, grep);
            parse_yaml_one(&bytes, &cfg).map(|arena| IngestOutput {
                arena,
                warnings: Vec::new(),
            })
        }
        InputKind::Text { bytes, mode } => {
            let cfg = grep_adjusted_cfg(priority_cfg, grep);
            let atomic = matches!(mode, crate::TextMode::CodeLike);
            parse_text_one_with_mode(bytes, &cfg, atomic).map(|arena| {
                IngestOutput {
                    arena,
                    warnings: Vec::new(),
                }
            })
        }
        InputKind::Fileset(inputs) => {
            Ok(fileset::parse_fileset_multi(inputs, priority_cfg, grep))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grep::{GrepConfig, GrepPatterns, GrepShow};
    use crate::order::NodeKind;

    #[test]
    fn parse_one_basic_shape() {
        let IngestOutput { arena, warnings } = parse_json_one(
            b"{\"a\":1}".to_vec(),
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .map(|arena| IngestOutput {
            arena,
            warnings: Vec::new(),
        })
        .unwrap();
        assert!(warnings.is_empty(), "single input should be silent");
        assert!(
            !arena.is_fileset,
            "single input should not be marked fileset"
        );
        let root = arena.root_id;
        assert_eq!(arena.nodes[root].kind, NodeKind::Object);
        assert_eq!(arena.nodes[root].object_len.unwrap_or(1), 1);
    }

    #[test]
    fn parse_many_sets_fileset_root() {
        let inputs = vec![
            ("a.json".to_string(), b"{}".to_vec()),
            ("b.json".to_string(), b"[]".to_vec()),
        ];
        let arena = formats::json::build_json_tree_arena_from_many(
            inputs,
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert!(arena.is_fileset, "multi input should be marked fileset");
        let root = arena.root_id;
        assert_eq!(arena.nodes[root].kind, NodeKind::Object);
        // Expect two top-level entries
        assert_eq!(arena.nodes[root].object_len.unwrap_or(0), 2);
    }

    fn grep_with_strong(pattern: &str) -> GrepConfig {
        GrepConfig {
            patterns: GrepPatterns::StrongOnly(
                regex::Regex::new(pattern).unwrap(),
            ),
            show: GrepShow::Matching,
        }
    }

    #[test]
    fn jsonl_grep_predicate_marks_matching_lines() {
        let input = b"{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n";
        let grep = grep_with_strong("b");
        let pred = jsonl_grep_predicate(input, &grep);
        assert!(!pred(0), "line 0 should not match");
        assert!(pred(1), "line 1 should match 'b'");
        assert!(!pred(2), "line 2 should not match");
    }

    #[test]
    fn jsonl_grep_predicate_multiple_matches() {
        let input = b"{\"x\":1}\n{\"x\":2}\n{\"y\":3}\n{\"x\":4}\n";
        let grep = grep_with_strong("x");
        let pred = jsonl_grep_predicate(input, &grep);
        assert!(pred(0));
        assert!(pred(1));
        assert!(!pred(2));
        assert!(pred(3));
    }

    #[test]
    fn jsonl_grep_predicate_no_strong_pattern_returns_noop() {
        let input = b"{\"a\":1}\n{\"b\":2}\n";
        let grep = GrepConfig::default(); // no patterns
        let pred = jsonl_grep_predicate(input, &grep);
        assert!(!pred(0));
        assert!(!pred(1));
    }

    #[test]
    fn jsonl_grep_predicate_skips_empty_lines() {
        // Empty lines are excluded from offsets, so indices are dense
        let input = b"{\"a\":1}\n\n{\"b\":2}\n";
        let grep = grep_with_strong("b");
        let pred = jsonl_grep_predicate(input, &grep);
        // Only 2 non-empty lines: index 0 = {"a":1}, index 1 = {"b":2}
        assert!(!pred(0));
        assert!(pred(1));
    }

    #[test]
    fn jsonl_grep_predicate_match_on_first_line() {
        let input = b"{\"needle\":true}\n{\"other\":false}\n";
        let grep = grep_with_strong("needle");
        let pred = jsonl_grep_predicate(input, &grep);
        assert!(pred(0), "match on first line should work");
        assert!(!pred(1));
    }

    #[test]
    fn jsonl_grep_predicate_match_on_last_line() {
        let input = b"{\"a\":1}\n{\"needle\":true}";
        let grep = grep_with_strong("needle");
        let pred = jsonl_grep_predicate(input, &grep);
        assert!(!pred(0));
        assert!(
            pred(1),
            "match on last line (no trailing newline) should work"
        );
    }

    #[test]
    fn jsonl_grep_predicate_out_of_bounds_returns_false() {
        let input = b"{\"a\":1}\n{\"b\":2}\n";
        let grep = grep_with_strong("a");
        let pred = jsonl_grep_predicate(input, &grep);
        assert!(!pred(99), "out of bounds index should return false");
    }

    #[test]
    fn fileset_ingest_surfaces_parse_warnings() {
        let inputs = vec![fileset::FilesetInput {
            name: "bad.json".to_string(),
            bytes: b"{".to_vec(),
            kind: fileset::FilesetInputKind::Json,
        }];
        let IngestOutput { arena, warnings } = ingest_into_arena(
            InputKind::Fileset(inputs),
            &PriorityConfig::new(usize::MAX, usize::MAX),
            &GrepConfig::default(),
        )
        .unwrap();
        assert!(arena.is_fileset, "fileset input should mark arena");
        assert!(
            warnings.iter().any(|n| n.contains("Failed to parse")),
            "expected parse warning: {warnings:?}"
        );
    }
}

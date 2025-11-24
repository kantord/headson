use anyhow::Result;

use crate::order::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

use crate::InputKind;

pub mod fileset;
pub mod format;
pub mod formats;

pub mod sampling;

#[allow(
    unused_imports,
    reason = "Re-exported helpers need to stay public even when unused internally"
)]
pub use formats::{parse_json_one, parse_text_one_with_mode, parse_yaml_one};

/// Dispatch the appropriate ingest path for any supported input kind.
pub fn ingest_into_arena(
    input: InputKind,
    priority_cfg: &PriorityConfig,
) -> Result<TreeArena> {
    match input {
        InputKind::Json(bytes) => parse_json_one(bytes, priority_cfg),
        InputKind::Yaml(bytes) => parse_yaml_one(bytes, priority_cfg),
        InputKind::Text { bytes, mode } => {
            let atomic = matches!(mode, crate::TextMode::CodeLike);
            parse_text_one_with_mode(bytes, priority_cfg, atomic)
        }
        InputKind::Fileset(inputs) => {
            fileset::parse_fileset_multi(inputs, priority_cfg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::NodeKind;

    #[test]
    fn parse_one_basic_shape() {
        let arena = parse_json_one(
            b"{\"a\":1}".to_vec(),
            &PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
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
}

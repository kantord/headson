use anyhow::Result;

use crate::order::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

/// Format-agnostic ingest boundary. Other formats can implement this trait
/// to produce the neutral TreeArena without going through JSON first.
pub trait Ingest {
    fn parse_one(bytes: Vec<u8>, cfg: &PriorityConfig) -> Result<TreeArena>;
    fn parse_many(
        inputs: Vec<(String, Vec<u8>)>,
        cfg: &PriorityConfig,
    ) -> Result<TreeArena>;
}

// Submodules for per-format adapters
pub mod json;
pub mod text;
pub mod yaml;

// Re-export commonly used helpers for convenience (keep adapter types private)
pub use json::{parse_json_many, parse_json_one};
pub use text::{parse_text_many, parse_text_one};
pub use yaml::{parse_yaml_many, parse_yaml_one};

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
        let arena = parse_json_many(
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

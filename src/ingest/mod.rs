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

/// JSON adapter for the ingest boundary. Delegates to the existing
/// JSON builder to produce the neutral TreeArena.
pub struct JsonIngest;

impl Ingest for JsonIngest {
    fn parse_one(bytes: Vec<u8>, cfg: &PriorityConfig) -> Result<TreeArena> {
        crate::json_ingest::build_json_tree_arena_from_bytes(bytes, cfg)
    }

    fn parse_many(
        inputs: Vec<(String, Vec<u8>)>,
        cfg: &PriorityConfig,
    ) -> Result<TreeArena> {
        crate::json_ingest::build_json_tree_arena_from_many(inputs, cfg)
    }
}

/// Convenience functions for the default (JSON) ingest path.
pub fn parse_json_one(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    JsonIngest::parse_one(bytes, cfg)
}

pub fn parse_json_many(
    inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    JsonIngest::parse_many(inputs, cfg)
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

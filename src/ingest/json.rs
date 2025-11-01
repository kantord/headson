use anyhow::Result;

use super::Ingest;
use crate::order::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

/// JSON adapter for the ingest boundary. Delegates to the existing
/// JSON builder to produce the neutral `TreeArena`.
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

/// Convenience functions for the JSON ingest path.
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

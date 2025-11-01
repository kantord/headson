use anyhow::Result;

use super::Ingest;
use crate::order::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

/// YAML adapter for the ingest boundary. Parses YAML using `yaml-rust2`
/// and builds the neutral `TreeArena`. Multi-document YAML in a single
/// input is wrapped in an array; multi-file inputs produce a fileset
/// object whose values may be arrays when a file contains multiple docs.
pub struct YamlIngest;

impl Ingest for YamlIngest {
    fn parse_one(bytes: Vec<u8>, cfg: &PriorityConfig) -> Result<TreeArena> {
        crate::yaml_ingest::build_yaml_tree_arena_from_bytes(bytes, cfg)
    }

    fn parse_many(
        inputs: Vec<(String, Vec<u8>)>,
        cfg: &PriorityConfig,
    ) -> Result<TreeArena> {
        crate::yaml_ingest::build_yaml_tree_arena_from_many(inputs, cfg)
    }
}

/// Convenience functions for the YAML ingest path.
pub fn parse_yaml_one(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    YamlIngest::parse_one(bytes, cfg)
}

pub fn parse_yaml_many(
    inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    YamlIngest::parse_many(inputs, cfg)
}

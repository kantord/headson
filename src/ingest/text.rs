use anyhow::Result;

use super::Ingest;
use crate::order::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

/// Text adapter for the ingest boundary.
///
/// Behavior:
/// - Decodes input as UTF-8 using `String::from_utf8_lossy`, which replaces
///   any invalid UTF-8 byte sequences with the Unicode replacement character
///   U+FFFD (this is what “lossy” means here).
/// - Normalizes newlines to `\n`, splits into logical lines, and builds an
///   array of string nodes.
/// - For multi-file inputs, produces a fileset object where each value is the
///   corresponding file’s array of lines.
pub struct TextIngest;

impl Ingest for TextIngest {
    fn parse_one(bytes: Vec<u8>, cfg: &PriorityConfig) -> Result<TreeArena> {
        crate::text_ingest::build_text_tree_arena_from_bytes(bytes, cfg)
    }

    fn parse_many(
        inputs: Vec<(String, Vec<u8>)>,
        cfg: &PriorityConfig,
    ) -> Result<TreeArena> {
        crate::text_ingest::build_text_tree_arena_from_many(inputs, cfg)
    }
}

/// Convenience functions for the Text ingest path.
pub fn parse_text_one(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    TextIngest::parse_one(bytes, cfg)
}

pub fn parse_text_many(
    inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    TextIngest::parse_many(inputs, cfg)
}

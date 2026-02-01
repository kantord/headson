mod builder;
mod samplers;

use anyhow::Result;
use builder::JsonTreeBuilder;
use serde::de::DeserializeSeed;

use crate::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

#[cfg(test)]
pub(crate) fn build_json_tree_arena(
    input: &str,
    config: &PriorityConfig,
) -> Result<TreeArena> {
    build_json_tree_arena_from_bytes(input.as_bytes().to_vec(), config)
}

pub(crate) fn build_json_tree_arena_from_bytes(
    mut bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<TreeArena> {
    build_json_tree_arena_from_slice(&mut bytes, config)
}

pub(crate) fn build_json_tree_arena_from_slice(
    bytes: &mut [u8],
    config: &PriorityConfig,
) -> Result<TreeArena> {
    let mut de = simd_json::Deserializer::from_slice(bytes)?;
    let builder = JsonTreeBuilder::new(
        config.array_max_items,
        config.array_sampler.into(),
    );
    let root_id: usize = {
        let seed = builder.seed();
        seed.deserialize(&mut de)?
    };
    let mut arena = builder.finish();
    arena.root_id = root_id;
    Ok(arena)
}

#[cfg(test)]
pub(crate) fn build_json_tree_arena_from_many(
    mut inputs: Vec<(String, Vec<u8>)>,
    config: &PriorityConfig,
) -> Result<TreeArena> {
    let builder = JsonTreeBuilder::new(
        config.array_max_items,
        config.array_sampler.into(),
    );
    let mut child_ids: Vec<usize> = Vec::with_capacity(inputs.len());
    let mut keys: Vec<String> = Vec::with_capacity(inputs.len());
    for (key, mut bytes) in inputs.drain(..) {
        let mut de = simd_json::Deserializer::from_slice(&mut bytes)?;
        let seed = builder.seed();
        let root_id: usize = seed.deserialize(&mut de)?;
        child_ids.push(root_id);
        keys.push(key);
    }
    let root_id = builder.push_object_root(keys, child_ids);
    let mut arena = builder.finish();
    arena.root_id = root_id;
    arena.is_fileset = true;
    Ok(arena)
}

/// Parse JSONL (newline-delimited JSON) into a tree arena.
/// Each non-empty line is parsed as independent JSON. The result is an array
/// whose children are the parsed lines, with 1-based line numbers stored as
/// array indices. The root node is marked with `is_jsonl_root = true`.
pub fn parse_jsonl_one(
    bytes: &[u8],
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| anyhow::anyhow!("JSONL input is not valid UTF-8: {e}"))?;
    let builder =
        JsonTreeBuilder::new(cfg.array_max_items, cfg.array_sampler.into());
    let root_id = builder.push_default();
    let mut child_ids: Vec<usize> = Vec::new();
    let mut line_numbers: Vec<usize> = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        // simd-json requires a mutable slice for in-place parsing
        let mut line_bytes = line.as_bytes().to_vec();
        let mut de = simd_json::Deserializer::from_slice(&mut line_bytes)
            .map_err(|e| {
                anyhow::anyhow!("JSONL line {}: {}", line_idx + 1, e)
            })?;
        let seed = builder.seed();
        let child_id: usize = seed.deserialize(&mut de).map_err(|e| {
            anyhow::anyhow!("JSONL line {}: {}", line_idx + 1, e)
        })?;
        child_ids.push(child_id);
        line_numbers.push(line_idx + 1);
    }

    let kept = child_ids.len();
    builder.finish_array(root_id, kept, kept, child_ids, line_numbers);

    let mut arena = builder.finish();
    arena.root_id = root_id;

    if let Some(node) = arena.nodes.get_mut(root_id) {
        node.array_len = Some(kept);
        node.is_jsonl_root = true;
    }

    Ok(arena)
}

/// Parse JSONL from a byte slice (for fileset use).
pub(crate) fn build_jsonl_tree_arena_from_slice(
    bytes: &[u8],
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    parse_jsonl_one(bytes, cfg)
}

/// Convenience functions for the JSON ingest path.
pub fn parse_json_one(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
) -> Result<TreeArena> {
    build_json_tree_arena_from_bytes(bytes, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fileset_marker_set_for_multi_inputs() {
        let inputs = vec![
            ("a.json".to_string(), b"{}".to_vec()),
            ("b.json".to_string(), b"[]".to_vec()),
        ];
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena = build_json_tree_arena_from_many(inputs, &cfg).unwrap();
        assert!(arena.is_fileset, "expected fileset marker true");
    }

    #[test]
    fn fileset_marker_false_for_single_input() {
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena =
            build_json_tree_arena_from_bytes(b"{}".to_vec(), &cfg).unwrap();
        assert!(!arena.is_fileset, "expected fileset marker false");
    }
}

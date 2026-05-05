mod builder;
mod samplers;

use anyhow::Result;
use builder::JsonTreeBuilder;
use serde::de::DeserializeSeed;

use crate::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena as TreeArena;

#[allow(dead_code, reason = "used only in non-test fn parse_jsonl_into_arena")]
type ChunkResult = (TreeArena, Vec<(usize, usize)>);

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

/// Collect (byte_start, 1-based line number) for every non-empty line.
pub(crate) fn jsonl_line_offsets(text: &str) -> Vec<(usize, usize)> {
    let mut offsets = Vec::new();
    let mut pos = 0usize;
    for (line_idx, raw_line) in text.split('\n').enumerate() {
        let start = pos;
        // +1 for the '\n' delimiter (absent after the last segment)
        pos += raw_line.len() + 1;
        if !raw_line.trim().is_empty() {
            offsets.push((start, line_idx + 1));
        }
    }
    offsets
}

/// Parse JSONL (newline-delimited JSON) into a tree arena.
/// Each non-empty line is parsed as independent JSON. The result is an array
/// whose children are the parsed lines, with 1-based line numbers stored as
/// array indices. The root node is marked with `is_jsonl_root = true`.
///
/// Lines are sampled using the same strategy as JSON arrays (controlled by
/// `PriorityConfig::array_max_items` and `array_sampler`), so only a subset
/// of lines is actually parsed for large inputs. When a `must_include`
/// predicate is provided, matching lines are always kept regardless of the
/// sampling cap.
pub fn parse_jsonl_one(
    bytes: &[u8],
    cfg: &PriorityConfig,
    must_include: impl Fn(usize) -> bool + Sync,
) -> Result<TreeArena> {
    use crate::ingest::sampling::{
        ArraySamplerKind, choose_indices, merge_required,
    };
    use crate::order::NodeKind;
    use crate::utils::tree_arena::JsonTreeNode;
    use rayon::prelude::*;

    let text = std::str::from_utf8(bytes)
        .map_err(|e| anyhow::anyhow!("JSONL input is not valid UTF-8: {e}"))?;

    let line_offsets = jsonl_line_offsets(text);
    let total = line_offsets.len();
    let sampler_kind: ArraySamplerKind = cfg.array_sampler.into();
    let sampled = choose_indices(sampler_kind, total, cfg.array_max_items);
    let kept_indices = merge_required(sampled, total, &must_include);

    let array_cap = cfg.array_max_items;

    // Parse kept lines in parallel, chunked to reduce per-task overhead.
    // Each chunk shares a single JsonTreeBuilder, producing one arena per
    // chunk instead of one per line.
    let num_chunks = rayon::current_num_threads().max(1);
    let chunk_size = (kept_indices.len() + num_chunks - 1) / num_chunks.max(1);

    let per_chunk: Vec<ChunkResult> = kept_indices
        .par_chunks(chunk_size.max(1))
        .map(|chunk| {
            let builder = JsonTreeBuilder::new(array_cap, sampler_kind);
            let mut roots: Vec<(usize, usize)> =
                Vec::with_capacity(chunk.len());
            let mut buf: Vec<u8> = Vec::new();
            for &idx in chunk {
                let (byte_start, line_num) = line_offsets[idx];
                let raw = &text[byte_start..];
                let raw = raw.split('\n').next().unwrap_or("").trim_end();
                let raw_bytes = raw.as_bytes();
                buf.clear();
                buf.extend_from_slice(raw_bytes);
                let mut de = simd_json::Deserializer::from_slice(&mut buf)
                    .map_err(|e| {
                        anyhow::anyhow!("JSONL line {line_num}: {e}")
                    })?;
                let seed = builder.seed();
                let root: usize = seed.deserialize(&mut de).map_err(|e| {
                    anyhow::anyhow!("JSONL line {line_num}: {e}")
                })?;
                roots.push((root, line_num));
            }
            Ok((builder.finish(), roots))
        })
        .collect::<Result<Vec<_>>>()?;

    // Sequential merge: combine chunk arenas into one, extracting each
    // line's root node via its offset-adjusted ID.
    let mut arena = TreeArena::default();
    let root_id = arena.nodes.len();
    arena.nodes.push(JsonTreeNode::default());

    let kept = kept_indices.len();
    let mut child_ids: Vec<usize> = Vec::with_capacity(kept);
    let mut line_numbers: Vec<usize> = Vec::with_capacity(kept);

    for (chunk_arena, roots) in per_chunk {
        let base = arena.nodes.len();
        let chunk_root = arena.append(chunk_arena);
        // chunk_root is the offset-adjusted root of the chunk arena,
        // but we need each individual line's root. The offset delta is
        // base - 0 (chunk arenas start at node 0).
        let _unused = chunk_root;
        for (orig_root, line_num) in roots {
            child_ids.push(base + orig_root);
            line_numbers.push(line_num);
        }
    }

    // Detect contiguous indices to skip storing arr_indices.
    let contiguous = line_numbers.len() == kept
        && line_numbers.iter().enumerate().all(|(i, &ln)| ln == i);

    let children_start = arena.children.len();
    arena.children.extend(&child_ids);

    let (arr_start, arr_len) = if kept == 0 || contiguous {
        (0, 0)
    } else {
        let start = arena.arr_indices.len();
        arena.arr_indices.extend(&line_numbers);
        (start, line_numbers.len())
    };

    let root = &mut arena.nodes[root_id];
    root.kind = NodeKind::Array;
    root.children_start = children_start;
    root.children_len = kept;
    root.array_len = Some(total);
    root.arr_indices_start = arr_start;
    root.arr_indices_len = arr_len;
    root.is_jsonl_root = true;

    arena.root_id = root_id;
    Ok(arena)
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

mod array_sample;
mod builder;
use serde::de::DeserializeSeed;

use crate::utils::tree_arena::JsonTreeArena;
use crate::{ArraySamplerStrategy, PriorityConfig};
use anyhow::Result;
use builder::JsonTreeBuilder;

#[cfg(test)]
pub fn build_json_tree_arena(
    input: &str,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    build_json_tree_arena_from_bytes(input.as_bytes().to_vec(), config)
}

pub fn build_json_tree_arena_from_bytes(
    mut bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let mut de = simd_json::Deserializer::from_slice(&mut bytes)?;
    let builder = JsonTreeBuilder::new(
        config.array_max_items,
        map_sampler(config.array_sampler),
    );
    let root_id: usize = {
        let seed = builder.seed();
        seed.deserialize(&mut de)?
    };
    let mut arena = builder.finish();
    arena.root_id = root_id;
    Ok(arena)
}

pub fn build_json_tree_arena_from_many(
    mut inputs: Vec<(String, Vec<u8>)>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let builder = JsonTreeBuilder::new(
        config.array_max_items,
        map_sampler(config.array_sampler),
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

fn map_sampler(kind: ArraySamplerStrategy) -> array_sample::ArraySamplerKind {
    match kind {
        ArraySamplerStrategy::Default => {
            array_sample::ArraySamplerKind::Default
        }
    }
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

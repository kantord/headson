mod builder;
use serde::de::DeserializeSeed;

use crate::order::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena;
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
    let builder = JsonTreeBuilder::new(config.array_max_items);
    let root_id: usize = {
        let seed = builder.seed();
        seed.deserialize(&mut de)?
    };
    let mut arena = builder.finish();
    arena.root_id = root_id;
    Ok(arena)
}

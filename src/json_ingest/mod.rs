mod builder;
mod samplers;
use serde::de::DeserializeSeed;

use crate::PriorityConfig;
use crate::utils::tree_arena::JsonTreeArena;
use anyhow::Result;
use builder::JsonTreeBuilder;
use regex::Regex;

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
        config.array_sampler.into(),
    );
    let root_id: usize = {
        let seed = builder.seed();
        seed.deserialize(&mut de)?
    };
    let mut arena = builder.finish();
    arena.root_id = root_id;
    if !config.grep_weak_patterns.is_empty() {
        compute_grep_matches(&mut arena, &config.grep_weak_patterns);
    }
    Ok(arena)
}

pub fn build_json_tree_arena_from_many(
    mut inputs: Vec<(String, Vec<u8>)>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
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
    if !config.grep_weak_patterns.is_empty() {
        compute_grep_matches(&mut arena, &config.grep_weak_patterns);
    }
    Ok(arena)
}

fn compute_grep_matches(arena: &mut JsonTreeArena, pats: &[Regex]) {
    let n = arena.nodes.len();
    if n == 0 {
        arena.grep_subtree_match.clear();
        return;
    }
    let direct = build_direct_match(arena, pats);
    let post = build_postorder(arena);
    let mut sub = vec![false; n];
    for id in post {
        let node = &arena.nodes[id];
        let mut any = direct[id];
        for i in 0..node.children_len {
            let cid = arena.children[node.children_start + i];
            if sub[cid] {
                any = true;
                break;
            }
        }
        sub[id] = any;
    }
    arena.grep_subtree_match = sub;
}

fn build_direct_match(arena: &JsonTreeArena, pats: &[Regex]) -> Vec<bool> {
    let n = arena.nodes.len();
    let mut out = vec![false; n];
    for (id, slot) in out.iter_mut().enumerate().take(n) {
        *slot = node_matches(arena, id, pats);
    }
    out
}

fn string_matches(s: &str, pats: &[Regex]) -> bool {
    pats.iter().any(|re| re.is_match(s))
}

fn keys_match(
    arena: &JsonTreeArena,
    node: &crate::utils::tree_arena::JsonTreeNode,
    pats: &[Regex],
) -> bool {
    if node.obj_keys_len == 0 {
        return false;
    }
    let start = node.obj_keys_start;
    let end = start + node.obj_keys_len;
    for k in &arena.obj_keys[start..end] {
        if string_matches(k, pats) {
            return true;
        }
    }
    false
}

fn node_matches(arena: &JsonTreeArena, id: usize, pats: &[Regex]) -> bool {
    let node = &arena.nodes[id];
    if let Some(s) = node.string_value.as_ref() {
        if string_matches(s, pats) {
            return true;
        }
    }
    keys_match(arena, node, pats)
}

fn build_postorder(arena: &JsonTreeArena) -> Vec<usize> {
    let n = arena.nodes.len();
    let mut out: Vec<usize> = Vec::with_capacity(n);
    let mut stack: Vec<(usize, bool)> = Vec::with_capacity(n.min(1024));
    stack.push((arena.root_id, false));
    while let Some((id, visited)) = stack.pop() {
        if visited {
            out.push(id);
            continue;
        }
        stack.push((id, true));
        let node = &arena.nodes[id];
        for i in 0..node.children_len {
            let cid = arena.children[node.children_start + i];
            stack.push((cid, false));
        }
    }
    out
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

use anyhow::Result;
use std::borrow::Cow;

use crate::PriorityConfig;
use crate::order::NodeKind;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

use crate::ingest::Ingest;
use crate::ingest::sampling::{ArraySamplerKind, choose_indices};

fn normalize_newlines(s: &str) -> Cow<'_, str> {
    // Normalize CRLF and CR to LF in a single allocation when needed.
    if s.as_bytes().contains(&b'\r') {
        let s = s.replace("\r\n", "\n");
        Cow::Owned(s.replace('\r', "\n"))
    } else {
        Cow::Borrowed(s)
    }
}

struct TextArenaBuilder {
    arena: JsonTreeArena,
    array_cap: usize,
    sampler: ArraySamplerKind,
}

impl TextArenaBuilder {
    fn new(array_cap: usize, sampler: ArraySamplerKind) -> Self {
        Self {
            arena: JsonTreeArena::default(),
            array_cap,
            sampler,
        }
    }

    fn finish(self) -> JsonTreeArena {
        self.arena
    }

    fn push_default(&mut self) -> usize {
        let id = self.arena.nodes.len();
        self.arena.nodes.push(JsonTreeNode::default());
        id
    }

    fn push_string(&mut self, s: String) -> usize {
        let id = self.push_default();
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::String;
        n.string_value = Some(s);
        id
    }

    fn push_string_atomic(&mut self, s: String) -> usize {
        let id = self.push_default();
        let n = &mut self.arena.nodes[id];
        // Model atomic strings as atomic token leaves; display kind later maps to String.
        n.kind = NodeKind::Number;
        n.atomic_token = Some(s);
        id
    }

    fn push_array_with_children(&mut self, children: Vec<usize>) -> usize {
        let id = self.push_default();
        let children_start = self.arena.children.len();
        let children_len = children.len();
        self.arena.children.extend(children);
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = children_len;
        n.array_len = Some(children_len);
        // No sampling at this level; contiguous
        n.arr_indices_start = 0;
        n.arr_indices_len = 0;
        id
    }

    fn push_root_array_sampled(
        &mut self,
        all_children: &[usize],
        total: usize,
    ) -> usize {
        let id = self.push_default();
        let idxs = choose_indices(self.sampler, total, self.array_cap);
        let kept = idxs.len().min(self.array_cap);
        let children_start = self.arena.children.len();
        for &orig_index in idxs.iter().take(kept) {
            if let Some(&cid) = all_children.get(orig_index) {
                self.arena.children.push(cid);
            }
        }
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = kept;
        n.array_len = Some(total);
        // Store arr_indices when not contiguous head prefix
        let contiguous =
            idxs.iter().take(kept).enumerate().all(|(i, &idx)| i == idx);
        if kept == 0 || contiguous {
            n.arr_indices_start = 0;
            n.arr_indices_len = 0;
        } else {
            let start = self.arena.arr_indices.len();
            self.arena.arr_indices.extend(idxs.into_iter().take(kept));
            let len = self.arena.arr_indices.len().saturating_sub(start);
            n.arr_indices_start = start;
            n.arr_indices_len = len.min(kept);
        }
        id
    }

    fn push_array_of_lines(
        &mut self,
        lines: &[String],
        total: usize,
        atomic: bool,
    ) -> usize {
        let id = self.push_default();
        let idxs = choose_indices(self.sampler, total, self.array_cap);
        let kept = idxs.len().min(self.array_cap);
        let mut pushed = 0usize;
        for (i, &orig_index) in idxs.iter().take(kept).enumerate() {
            if let Some(line) = lines.get(orig_index) {
                let child = if atomic {
                    self.push_string_atomic(line.clone())
                } else {
                    self.push_string(line.clone())
                };
                self.arena.children.push(child);
                pushed = i + 1;
            }
        }
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = self.arena.children.len().saturating_sub(pushed);
        n.children_len = pushed;
        n.array_len = Some(total);
        // Store arr_indices when not contiguous head prefix
        let contiguous =
            idxs.iter().take(kept).enumerate().all(|(i, &idx)| i == idx);
        if pushed == 0 || contiguous {
            n.arr_indices_start = 0;
            n.arr_indices_len = 0;
        } else {
            let start = self.arena.arr_indices.len();
            self.arena.arr_indices.extend(idxs.into_iter().take(kept));
            let len = self.arena.arr_indices.len().saturating_sub(start);
            n.arr_indices_start = start;
            n.arr_indices_len = len.min(pushed);
        }
        id
    }

    fn push_object_root(
        &mut self,
        keys: Vec<String>,
        children: Vec<usize>,
    ) -> usize {
        let id = self.push_default();
        let count = keys.len().min(children.len());
        let children_start = self.arena.children.len();
        let obj_keys_start = self.arena.obj_keys.len();
        self.arena.children.extend(children);
        self.arena.obj_keys.extend(keys);
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Object;
        n.children_start = children_start;
        n.children_len = count;
        n.obj_keys_start = obj_keys_start;
        n.obj_keys_len = count;
        n.object_len = Some(count);
        id
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Signature matches other ingest helpers and trait expectations"
)]
#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "Builder + indent/nesting logic is clearest co-located"
)]
pub fn build_text_tree_arena_from_bytes_with_mode(
    bytes: Vec<u8>,
    config: &PriorityConfig,
    atomic_strings: bool,
) -> Result<JsonTreeArena> {
    let lossy = String::from_utf8_lossy(&bytes);
    let norm = normalize_newlines(&lossy);
    // split_terminator keeps no trailing empty item for trailing newline
    let raw_lines: Vec<&str> = norm.split_terminator('\n').collect();

    // Detect indent unit: prefer tabs if present, otherwise minimal positive
    // number of leading spaces among indented lines; default to 2 spaces.
    let uses_tab = raw_lines.iter().any(|l| l.starts_with('\t'));
    let space_unit = if uses_tab {
        0usize
    } else {
        let mut min_pos: Option<usize> = None;
        for l in &raw_lines {
            let count = l.chars().take_while(|c| *c == ' ').count();
            if count > 0 {
                min_pos = Some(min_pos.map_or(count, |m| m.min(count)));
            }
        }
        min_pos.unwrap_or(2)
    };

    // Safe nested builder using indices (no raw pointers).
    #[derive(Default)]
    struct TNode {
        text: String,
        children: Vec<usize>,
    }

    let mut tnodes: Vec<TNode> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    // Stack of node indices representing the current path; length equals current depth.
    let mut stack: Vec<usize> = Vec::new();

    for &l in &raw_lines {
        // Compute structural depth from leading whitespace, but preserve
        // the original line text (including its exact indentation).
        let (mut d, text) = if uses_tab {
            let tabs = l.chars().take_while(|c| *c == '\t').count();
            (tabs, l.to_string())
        } else {
            let spaces = l.chars().take_while(|c| *c == ' ').count();
            let unit = space_unit.max(1);
            let depth = spaces / unit;
            (depth, l.to_string())
        };
        // Clamp over-indentation to at most +1 level from current, or to top if stack empty.
        let cur_depth = stack.len();
        if cur_depth == 0 && d > 0 {
            d = 0;
        } else if d > cur_depth + 1 {
            d = cur_depth + 1;
        }
        // Pop to the desired parent depth.
        while stack.len() > d {
            stack.pop();
        }
        // Create node
        let id = tnodes.len();
        tnodes.push(TNode {
            text,
            children: Vec::new(),
        });
        if d == 0 {
            roots.push(id);
        } else {
            let parent_id_opt = stack
                .get(d.saturating_sub(1))
                .copied()
                .or_else(|| stack.last().copied());
            if let Some(parent_id) = parent_id_opt {
                tnodes[parent_id].children.push(id);
            } else {
                // Fallback: treat as a new root to avoid panics (should be rare due to clamping)
                roots.push(id);
            }
        }
        // Push this node as the new current for depth d
        stack.push(id);
    }

    // Transcribe TNode tree into the JSON arena.
    fn push_tnode(
        id: usize,
        tnodes: &Vec<TNode>,
        b: &mut TextArenaBuilder,
        atomic: bool,
    ) -> usize {
        let n = &tnodes[id];
        let mut kids: Vec<usize> = Vec::new();
        if atomic {
            kids.push(b.push_string_atomic(n.text.clone()));
        } else {
            kids.push(b.push_string(n.text.clone()));
        }
        for &ch in &n.children {
            kids.push(push_tnode(ch, tnodes, b, atomic));
        }
        b.push_array_with_children(kids)
    }

    let mut b = TextArenaBuilder::new(
        config.array_max_items,
        config.array_sampler.into(),
    );
    let total = roots.len();
    let mut all_children: Vec<usize> = Vec::with_capacity(total);
    for &rid in &roots {
        all_children.push(push_tnode(rid, &tnodes, &mut b, atomic_strings));
    }
    let root_id = b.push_root_array_sampled(&all_children, total);
    let mut a = b.finish();
    a.root_id = root_id;
    Ok(a)
}

pub fn build_text_tree_arena_from_bytes(
    bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    build_text_tree_arena_from_bytes_with_mode(bytes, config, false)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "Signature matches other ingest helpers and trait expectations"
)]
pub fn build_text_tree_arena_from_many(
    mut inputs: Vec<(String, Vec<u8>)>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let mut b = TextArenaBuilder::new(
        config.array_max_items,
        config.array_sampler.into(),
    );
    let mut keys: Vec<String> = Vec::with_capacity(inputs.len());
    let mut children_ids: Vec<usize> = Vec::with_capacity(inputs.len());

    for (key, bytes) in inputs.drain(..) {
        let lossy = String::from_utf8_lossy(&bytes);
        let norm = normalize_newlines(&lossy);
        let lines_vec: Vec<String> = norm
            .split_terminator('\n')
            .map(std::string::ToString::to_string)
            .collect();
        let total = lines_vec.len();
        let atomic = crate::utils::extensions::is_code_like_name(&key);
        let child_id = b.push_array_of_lines(&lines_vec, total, atomic);
        keys.push(key);
        children_ids.push(child_id);
    }
    let root_id = b.push_object_root(keys, children_ids);
    let mut a = b.finish();
    a.root_id = root_id;
    a.is_fileset = true;
    Ok(a)
}

pub struct TextIngest;

impl Ingest for TextIngest {
    fn parse_one(
        bytes: Vec<u8>,
        cfg: &PriorityConfig,
    ) -> Result<JsonTreeArena> {
        build_text_tree_arena_from_bytes(bytes, cfg)
    }

    fn parse_many(
        inputs: Vec<(String, Vec<u8>)>,
        cfg: &PriorityConfig,
    ) -> Result<JsonTreeArena> {
        build_text_tree_arena_from_many(inputs, cfg)
    }
}

/// Convenience functions for the Text ingest path.
pub fn parse_text_one(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    TextIngest::parse_one(bytes, cfg)
}

pub fn parse_text_many(
    inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    TextIngest::parse_many(inputs, cfg)
}

#[cfg(test)]
mod tests {
    use crate::{
        PriorityConfig, RenderConfig, headson_text,
        serialization::types::{OutputTemplate, Style},
    };

    fn cfg_text() -> (RenderConfig, PriorityConfig) {
        let cfg = RenderConfig {
            template: OutputTemplate::Text,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::serialization::types::ColorMode::Off,
            color_enabled: false,
            style: Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
        };
        let prio = PriorityConfig::new(100, 100);
        (cfg, prio)
    }

    #[test]
    fn text_roundtrip_basic() {
        let (cfg, prio) = cfg_text();
        let input = b"a\nb\nc".to_vec();
        let out = headson_text(input, &cfg, &prio, 100).unwrap();
        assert_eq!(out, "a\nb\nc\n");
    }

    #[test]
    fn text_omission_marker_default() {
        let (mut cfg, prio) = cfg_text();
        let input = (0..10)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        // Budget small so only some lines fit
        cfg.style = Style::Default;
        let out = headson_text(input.into_bytes(), &cfg, &prio, 20).unwrap();
        assert!(out.contains("…\n"));
    }

    #[test]
    fn tail_sampler_keeps_last_n_indices_text() {
        // Build 10 lines; with array_max_items=5 and tail sampler we should keep last 5
        let lines = (0..10)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut cfg = PriorityConfig::new(usize::MAX, 5);
        cfg.array_sampler = crate::ArraySamplerStrategy::Tail;
        let arena =
            super::build_text_tree_arena_from_bytes(lines.into_bytes(), &cfg)
                .expect("arena");
        let root = &arena.nodes[arena.root_id];
        assert_eq!(root.children_len, 5, "kept 5");
        let mut orig_indices = Vec::new();
        for i in 0..root.children_len {
            let oi = if root.arr_indices_len > 0 {
                arena.arr_indices[root.arr_indices_start + i]
            } else {
                i
            };
            orig_indices.push(oi);
        }
        assert_eq!(orig_indices, vec![5, 6, 7, 8, 9]);
    }
}

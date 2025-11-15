use anyhow::Result;
use std::borrow::Cow;
use std::sync::Arc;

use crate::order::NodeKind;
use crate::utils::{
    text::truncate_at_n_graphemes,
    tree_arena::{JsonTreeArena, JsonTreeNode},
};
use crate::{ArrayBias, PriorityConfig};

use crate::ingest::sampling::{ArraySamplerKind, choose_indices};
use crate::ingest::{Ingest, fileset::build_fileset_root};

fn normalize_newlines(s: &str) -> Cow<'_, str> {
    // Normalize CRLF and CR to LF in a single allocation when needed.
    if s.as_bytes().contains(&b'\r') {
        let s = s.replace("\r\n", "\n");
        Cow::Owned(s.replace('\r', "\n"))
    } else {
        Cow::Borrowed(s)
    }
}

const ARRAY_NO_SAMPLING_THRESHOLD: usize = 20_000;
const CODE_LINE_HARD_CAP: usize = 150;

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

    fn push_string(&mut self, s: String, prefer_parent_line: bool) -> usize {
        let id = self.push_default();
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::String;
        n.string_value = Some(s);
        n.prefers_parent_line = prefer_parent_line;
        id
    }

    fn push_string_atomic(
        &mut self,
        s: String,
        prefer_parent_line: bool,
    ) -> usize {
        let id = self.push_default();
        let n = &mut self.arena.nodes[id];
        // Model atomic strings as atomic token leaves; display kind later maps to String.
        n.kind = NodeKind::Number;
        n.atomic_token = Some(s);
        n.prefers_parent_line = prefer_parent_line;
        id
    }

    fn push_array_with_children(
        &mut self,
        children: Vec<usize>,
        child_orig_indices: Option<Vec<usize>>,
        bias_override: Option<ArrayBias>,
        force_first_line: bool,
    ) -> usize {
        let id = self.push_default();
        let children_start = self.arena.children.len();
        let children_len = children.len();
        self.arena.children.extend(children);
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = children_len;
        n.array_len = Some(children_len);
        n.array_bias_override = bias_override;
        n.force_first_line = force_first_line;
        if let Some(orig) = child_orig_indices {
            let start = self.arena.arr_indices.len();
            self.arena.arr_indices.extend(orig);
            let len = self.arena.arr_indices.len().saturating_sub(start);
            n.arr_indices_start = start;
            n.arr_indices_len = len.min(children_len);
        } else {
            n.arr_indices_start = 0;
            n.arr_indices_len = 0;
        }
        id
    }

    fn push_root_array_sampled(
        &mut self,
        all_children: &[usize],
        total: usize,
        bias_override: Option<ArrayBias>,
    ) -> usize {
        let id = self.push_default();
        if total <= ARRAY_NO_SAMPLING_THRESHOLD {
            let children_start = self.arena.children.len();
            self.arena.children.extend_from_slice(all_children);
            let n = &mut self.arena.nodes[id];
            n.kind = NodeKind::Array;
            n.children_start = children_start;
            n.children_len = total;
            n.array_len = Some(total);
            n.array_bias_override = bias_override;
            n.force_first_line = false;
            n.arr_indices_start = 0;
            n.arr_indices_len = 0;
            return id;
        }
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
        n.array_bias_override = bias_override;
        n.force_first_line = false;
        // Always store original indices for child arrays to enable global line numbering
        let start = self.arena.arr_indices.len();
        self.arena.arr_indices.extend(idxs.into_iter().take(kept));
        let len = self.arena.arr_indices.len().saturating_sub(start);
        n.arr_indices_start = start;
        n.arr_indices_len = len.min(kept);
        id
    }

    fn push_array_of_lines(
        &mut self,
        lines: &[String],
        total: usize,
    ) -> usize {
        let id = self.push_default();
        if total <= ARRAY_NO_SAMPLING_THRESHOLD {
            self.push_full_line_array(id, lines, total);
            return id;
        }
        self.push_sampled_line_array(id, lines, total);
        id
    }

    fn push_full_line_array(
        &mut self,
        id: usize,
        lines: &[String],
        total: usize,
    ) {
        let children_start = self.arena.children.len();
        for line in lines {
            let child = self.push_string(line.clone(), false);
            self.arena.children.push(child);
        }
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = total;
        n.array_len = Some(total);
        n.arr_indices_start = 0;
        n.arr_indices_len = 0;
    }

    fn push_sampled_line_array(
        &mut self,
        id: usize,
        lines: &[String],
        total: usize,
    ) {
        let idxs = choose_indices(self.sampler, total, self.array_cap);
        let kept = idxs.len().min(self.array_cap);
        let children_start = self.arena.children.len();
        let mut pushed = 0usize;
        for (i, &orig_index) in idxs.iter().take(kept).enumerate() {
            if let Some(line) = lines.get(orig_index) {
                let child = self.push_string(line.clone(), false);
                self.arena.children.push(child);
                pushed = i + 1;
            }
        }
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = pushed;
        n.array_len = Some(total);
        let contiguous =
            idxs.iter().take(kept).enumerate().all(|(i, &idx)| i == idx);
        if pushed == 0 || contiguous {
            n.arr_indices_start = 0;
            n.arr_indices_len = 0;
        } else {
            let start = self.arena.arr_indices.len();
            self.arena
                .arr_indices
                .extend(idxs.iter().take(kept).copied());
            let len = self.arena.arr_indices.len().saturating_sub(start);
            n.arr_indices_start = start;
            n.arr_indices_len = len.min(pushed);
        }
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
fn build_text_tree_arena_plain(
    bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let lossy = String::from_utf8_lossy(&bytes);
    let norm = normalize_newlines(&lossy);
    let lines_vec: Vec<String> = norm
        .split_terminator('\n')
        .map(std::string::ToString::to_string)
        .collect();
    let total = lines_vec.len();
    let mut b = TextArenaBuilder::new(
        config.array_max_items,
        config.array_sampler.into(),
    );
    let root_id = b.push_array_of_lines(&lines_vec, total);
    let mut a = b.finish();
    a.root_id = root_id;
    Ok(a)
}

// Safe nested builder using indices (no raw pointers).
#[derive(Default)]
struct TNode {
    text: String,
    children: Vec<usize>,
}

fn detect_indent_unit(raw_lines: &[&str]) -> (bool, usize) {
    let uses_tab = raw_lines.iter().any(|l| l.starts_with('\t'));
    if uses_tab {
        (true, 0)
    } else {
        let mut min_pos: Option<usize> = None;
        for l in raw_lines {
            let count = l.chars().take_while(|c| *c == ' ').count();
            if count > 0 {
                min_pos = Some(min_pos.map_or(count, |m| m.min(count)));
            }
        }
        (false, min_pos.unwrap_or(2))
    }
}

fn build_code_nodes(
    raw_lines: &[&str],
    uses_tab: bool,
    space_unit: usize,
) -> (Vec<TNode>, Vec<usize>) {
    let mut tnodes: Vec<TNode> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for &l in raw_lines {
        let (raw_depth, text_raw, is_blank) =
            parse_code_line(l, uses_tab, space_unit);
        let mut target_depth = raw_depth;
        if is_blank {
            target_depth = stack.len();
        }
        let depth = clamp_depth(target_depth, stack.len());
        pop_to_depth(&mut stack, depth);
        let id = tnodes.len();
        tnodes.push(TNode {
            text: text_raw,
            children: Vec::new(),
        });
        attach_code_node(
            depth,
            is_blank,
            &mut stack,
            &mut roots,
            &mut tnodes,
            id,
        );
    }

    (tnodes, roots)
}

fn parse_code_line(
    line: &str,
    uses_tab: bool,
    space_unit: usize,
) -> (usize, String, bool) {
    if uses_tab {
        let tabs = line.chars().take_while(|c| *c == '\t').count();
        let text = line.to_string();
        let is_blank = text.trim().is_empty();
        (tabs, text, is_blank)
    } else {
        let spaces = line.chars().take_while(|c| *c == ' ').count();
        let unit = space_unit.max(1);
        let depth = spaces / unit;
        let text = line.to_string();
        let is_blank = text.trim().is_empty();
        (depth, text, is_blank)
    }
}

fn clamp_depth(depth: usize, current_depth: usize) -> usize {
    if current_depth == 0 && depth > 0 {
        0
    } else if depth > current_depth + 1 {
        current_depth + 1
    } else {
        depth
    }
}

fn pop_to_depth(stack: &mut Vec<usize>, target_depth: usize) {
    while stack.len() > target_depth {
        stack.pop();
    }
}

fn attach_code_node(
    depth: usize,
    is_blank: bool,
    stack: &mut Vec<usize>,
    roots: &mut Vec<usize>,
    tnodes: &mut [TNode],
    id: usize,
) {
    if depth == 0 {
        if !is_blank {
            roots.push(id);
        }
    } else if let Some(parent_id) = stack
        .get(depth.saturating_sub(1))
        .copied()
        .or_else(|| stack.last().copied())
    {
        tnodes[parent_id].children.push(id);
    } else {
        roots.push(id);
    }
    if !is_blank {
        stack.push(id);
    }
}

fn push_code_tnode(
    id: usize,
    tnodes: &[TNode],
    builder: &mut TextArenaBuilder,
    depth: usize,
) -> usize {
    let n = &tnodes[id];
    let mut kids: Vec<usize> = Vec::with_capacity(1 + n.children.len());
    let mut origs: Vec<usize> = Vec::with_capacity(1 + n.children.len());
    let prefer_line = !n.children.is_empty();
    kids.push(builder.push_string_atomic(n.text.clone(), prefer_line));
    origs.push(id);
    for &child in &n.children {
        let arr = push_code_tnode(child, tnodes, builder, depth + 1);
        kids.push(arr);
        origs.push(child);
    }
    let bias_override = if depth == 0 {
        Some(ArrayBias::HeadMidTail)
    } else {
        Some(ArrayBias::HeadTail)
    };
    let force_first = depth == 0 && !n.text.trim().is_empty();
    builder.push_array_with_children(
        kids,
        Some(origs),
        bias_override,
        force_first,
    )
}

fn transcribe_code_tree(
    tnodes: &[TNode],
    roots: &[usize],
    config: &PriorityConfig,
) -> JsonTreeArena {
    let mut builder = TextArenaBuilder::new(
        config.array_max_items,
        config.array_sampler.into(),
    );
    let mut all_children: Vec<usize> = Vec::with_capacity(roots.len());
    for &rid in roots {
        all_children.push(push_code_tnode(rid, tnodes, &mut builder, 0));
    }
    let root_id = builder.push_root_array_sampled(
        &all_children,
        all_children.len(),
        Some(ArrayBias::HeadMidTail),
    );
    let mut arena = builder.finish();
    arena.root_id = root_id;
    arena
}

pub fn build_text_tree_arena_from_bytes_with_mode(
    bytes: Vec<u8>,
    config: &PriorityConfig,
    atomic_strings: bool,
) -> Result<JsonTreeArena> {
    if !atomic_strings {
        return build_text_tree_arena_plain(bytes, config);
    }
    Ok(build_code_tree_arena(&bytes, config))
}

fn build_code_tree_arena(
    bytes: &[u8],
    config: &PriorityConfig,
) -> JsonTreeArena {
    let lossy = String::from_utf8_lossy(bytes);
    let norm = normalize_newlines(&lossy);
    let owned_lines: Vec<String> = norm
        .split_terminator('\n')
        .map(|line| truncate_at_n_graphemes(line, CODE_LINE_HARD_CAP))
        .collect();
    let raw_lines: Vec<&str> =
        owned_lines.iter().map(String::as_str).collect();
    let (uses_tab, space_unit) = detect_indent_unit(&raw_lines);
    let (tnodes, roots) = build_code_nodes(&raw_lines, uses_tab, space_unit);
    let mut arena = transcribe_code_tree(&tnodes, &roots, config);
    let root = arena.root_id;
    arena.code_lines.insert(root, Arc::new(owned_lines));
    arena
}

pub fn build_text_tree_arena_from_bytes(
    bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    build_text_tree_arena_plain(bytes, config)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "Signature matches other ingest helpers and trait expectations"
)]
pub fn build_text_tree_arena_from_many(
    inputs: Vec<(String, Vec<u8>)>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let mut arenas: Vec<(String, JsonTreeArena)> =
        Vec::with_capacity(inputs.len());
    for (key, bytes) in inputs {
        let atomic = crate::utils::extensions::is_code_like_name(&key);
        let arena =
            build_text_tree_arena_from_bytes_with_mode(bytes, config, atomic)?;
        arenas.push((key, arena));
    }
    Ok(build_fileset_root(arenas))
}

pub struct TextIngest;

impl Ingest for TextIngest {
    fn parse_one(
        bytes: Vec<u8>,
        cfg: &PriorityConfig,
    ) -> Result<JsonTreeArena> {
        parse_text_one(bytes, cfg)
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
    parse_text_one_with_mode(bytes, cfg, false)
}

/// Version of `parse_text_one` that allows explicitly toggling atomic code mode.
pub fn parse_text_one_with_mode(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
    atomic_strings: bool,
) -> Result<JsonTreeArena> {
    build_text_tree_arena_from_bytes_with_mode(bytes, cfg, atomic_strings)
}

pub fn parse_text_many(
    inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    TextIngest::parse_many(inputs, cfg)
}

#[cfg(test)]
mod tests {
    use super::ARRAY_NO_SAMPLING_THRESHOLD;
    use crate::{
        PriorityConfig, RenderConfig, headson_text,
        serialization::types::{OutputTemplate, Style},
    };
    use unicode_segmentation::UnicodeSegmentation;

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
            primary_source_name: None,
            show_fileset_headers: true,
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
        // Build more lines than the sampling threshold so tail sampler is exercised.
        let total = ARRAY_NO_SAMPLING_THRESHOLD + 10;
        let lines = (0..total)
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
        assert_eq!(
            orig_indices,
            ((total - 5)..total).collect::<Vec<_>>(),
            "tail sampler should keep last 5 indices"
        );
    }

    #[test]
    fn code_mode_truncates_long_lines() {
        let (_, prio) = cfg_text();
        let long_line = format!("fn main() {{ {} }}", "a".repeat(200));
        let arena = super::build_text_tree_arena_from_bytes_with_mode(
            format!("{long_line}\n").into_bytes(),
            &prio,
            true,
        )
        .expect("arena");
        let code_lines = arena
            .code_lines
            .get(&arena.root_id)
            .expect("code lines present");
        assert_eq!(code_lines.len(), 1);
        let line = &code_lines[0];
        assert!(
            line.ends_with('…'),
            "long code line should be truncated with ellipsis"
        );
        let graphemes =
            UnicodeSegmentation::graphemes(line.as_str(), true).count();
        assert_eq!(
            graphemes,
            super::CODE_LINE_HARD_CAP + 1,
            "line should cap at {} graphemes plus ellipsis",
            super::CODE_LINE_HARD_CAP
        );
    }

    #[test]
    fn plain_text_ingest_keeps_full_lines() {
        let (_, prio) = cfg_text();
        let long_line = format!("text {}", "b".repeat(200));
        let arena = super::build_text_tree_arena_from_bytes(
            format!("{long_line}\n").into_bytes(),
            &prio,
        )
        .expect("arena");
        let root = &arena.nodes[arena.root_id];
        let first_child = arena.children[root.children_start];
        let node = &arena.nodes[first_child];
        assert_eq!(
            node.string_value.as_deref(),
            Some(long_line.as_str()),
            "plain text ingest should not truncate lines"
        );
    }
}

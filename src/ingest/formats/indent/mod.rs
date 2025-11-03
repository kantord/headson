use anyhow::Result;

use crate::PriorityConfig;
use crate::order::NodeKind;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

use crate::ingest::Ingest;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum IndentStyle {
    Tabs,
    Spaces(usize), // unit width
}

fn normalize_newlines(s: &str) -> std::borrow::Cow<'_, str> {
    if s.as_bytes().contains(&b'\r') {
        let s = s.replace("\r\n", "\n");
        std::borrow::Cow::Owned(s.replace('\r', "\n"))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

fn detect_indent_unit(lines: &[&str]) -> IndentStyle {
    // If any line starts with a tab, prefer Tabs.
    if lines.iter().any(|l| l.starts_with('\t')) {
        return IndentStyle::Tabs;
    }
    // Collect positive indent deltas of spaces between consecutive non-empty lines.
    let mut last_indent = None::<usize>;
    let mut deltas: Vec<usize> = Vec::new();
    for line in lines.iter().copied() {
        let trimmed = line.trim_start_matches(|c: char| c == ' ' || c == '\t');
        // Count spaces only for detection; tabs treated as spaces width 4 here.
        let mut count = 0usize;
        for ch in line.chars() {
            match ch {
                ' ' => count += 1,
                '\t' => count += 4,
                _ => break,
            }
        }
        if trimmed.is_empty() {
            continue;
        }
        if let Some(prev) = last_indent {
            if count > prev {
                deltas.push(count - prev);
            }
        }
        last_indent = Some(count);
    }
    // Choose the most common small delta; fallback to gcd; default 4
    if deltas.is_empty() {
        return IndentStyle::Spaces(4);
    }
    // Mode over common widths
    let mut counts = [(2usize, 0usize), (4, 0), (8, 0)];
    for d in &deltas {
        for (w, c) in counts.iter_mut() {
            if d % *w == 0 {
                *c += 1;
            }
        }
    }
    counts.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
    if counts[0].1 > 0 {
        return IndentStyle::Spaces(counts[0].0);
    }
    // gcd fallback
    let mut g = deltas[0];
    for &d in &deltas[1..] {
        g = num_integer::gcd(g, d);
    }
    if g == 0 {
        IndentStyle::Spaces(4)
    } else {
        IndentStyle::Spaces(g.min(8).max(1))
    }
}

// No code parsing: we do not track delimiters/strings/comments.

struct Builder {
    arena: JsonTreeArena,
}

impl Builder {
    fn new() -> Self {
        Self {
            arena: JsonTreeArena::default(),
        }
    }

    fn push_default(&mut self) -> usize {
        let id = self.arena.nodes.len();
        self.arena.nodes.push(JsonTreeNode::default());
        id
    }

    fn push_array_empty(&mut self) -> usize {
        let id = self.push_default();
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = self.arena.children.len();
        n.children_len = 0;
        n.array_len = Some(0);
        id
    }

    fn append_child(&mut self, arr_id: usize, child_id: usize) {
        self.arena.children.push(child_id);
        let n = &mut self.arena.nodes[arr_id];
        n.children_len += 1;
        if let Some(len) = n.array_len.as_mut() {
            *len += 1;
        }
    }

    fn push_string(&mut self, s: String) -> usize {
        let id = self.push_default();
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::String;
        n.string_value = Some(s);
        id
    }

    fn push_line_entry(&mut self, text: String) -> (usize, usize) {
        // children array first (so object can reference it)
        let children_arr = self.push_array_empty();
        let line_str = self.push_string(text);
        let obj_id = self.push_default();
        let keys_start = self.arena.obj_keys.len();
        self.arena.obj_keys.push("line".to_string());
        self.arena.obj_keys.push("children".to_string());
        let children_start = self.arena.children.len();
        self.arena.children.push(line_str);
        self.arena.children.push(children_arr);
        let n = &mut self.arena.nodes[obj_id];
        n.kind = NodeKind::Object;
        n.obj_keys_start = keys_start;
        n.obj_keys_len = 2;
        n.children_start = children_start;
        n.children_len = 2;
        n.object_len = Some(2);
        (obj_id, children_arr)
    }
}

fn build_indent_tree_arena_from_bytes(
    bytes: Vec<u8>,
    _cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let lossy = String::from_utf8_lossy(&bytes);
    let norm = normalize_newlines(&lossy);
    let all_lines: Vec<&str> = norm.split_terminator('\n').collect();
    // Detect indent style/unit on the whole file
    let style = detect_indent_unit(&all_lines);
    let mut b = Builder::new();
    // Root is an array of entries
    let root_arr = b.push_array_empty();
    let mut stack: Vec<usize> = vec![root_arr]; // stack holds array node ids
    let mut last_children: Option<usize> = None;
    let mut current_level = 0usize; // relative to root
    for line in all_lines.into_iter() {
        // Treat all lines equally; no comment/string/delimiter parsing.
        // Count leading indent "columns"
        let mut cols = 0usize;
        match style {
            IndentStyle::Tabs => {
                for ch in line.chars() {
                    if ch == '\t' {
                        cols += 1;
                    } else if ch == ' ' {
                        cols += 0;
                    } else {
                        break;
                    }
                }
            }
            IndentStyle::Spaces(unit) => {
                for ch in line.chars() {
                    if ch == ' ' {
                        cols += 1;
                    } else if ch == '\t' {
                        cols += unit;
                    } else {
                        break;
                    }
                }
                cols /= unit.max(1);
            }
        }
        // Structural nesting strictly follows indentation.
        let target_level = cols;
        // Adjust stack depth
        if target_level > current_level {
            // push last_children arrays as needed
            for _ in 0..(target_level - current_level) {
                if let Some(child_arr) = last_children {
                    stack.push(child_arr);
                }
            }
        } else if target_level < current_level {
            for _ in 0..(current_level - target_level) {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
        }
        current_level = target_level;
        // Create node for this line under current array; strip leading indent so we
        // can reconstruct indentation from nesting during serialization.
        let content = line.trim_start_matches(|c| c == ' ' || c == '\t');
        let (obj_id, children_arr) = b.push_line_entry(content.to_string());
        let parent_arr = *stack.last().expect("root stack not empty");
        b.append_child(parent_arr, obj_id);
        last_children = Some(children_arr);
    }
    let mut arena = b.arena;
    arena.root_id = root_arr;
    Ok(arena)
}

fn build_indent_tree_arena_from_many(
    mut inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    // Build each file's root array, then wrap in an object keyed by filename
    let mut arena = JsonTreeArena::default();
    let mut keys: Vec<String> = Vec::with_capacity(inputs.len());
    let mut child_ids: Vec<usize> = Vec::with_capacity(inputs.len());
    for (name, bytes) in inputs.drain(..) {
        let sub = build_indent_tree_arena_from_bytes(bytes, cfg)?;
        // Splice sub arena into main arena by offsetting indices.
        let base_nodes = arena.nodes.len();
        let base_children = arena.children.len();
        let base_keys = arena.obj_keys.len();
        let base_arr_indices = arena.arr_indices.len();
        // Move nodes and adjust their internal offsets
        for mut node in sub.nodes.into_iter() {
            if node.children_len > 0 {
                node.children_start += base_children;
            }
            if node.obj_keys_len > 0 {
                node.obj_keys_start += base_keys;
            }
            if node.arr_indices_len > 0 {
                node.arr_indices_start += base_arr_indices;
            }
            arena.nodes.push(node);
        }
        arena
            .children
            .extend(sub.children.into_iter().map(|id| id + base_nodes));
        arena.obj_keys.extend(sub.obj_keys);
        arena.arr_indices.extend(sub.arr_indices);
        keys.push(name);
        child_ids.push(sub.root_id + base_nodes);
    }
    // Make wrapper object
    let root_id = arena.nodes.len();
    arena.nodes.push(JsonTreeNode::default());
    let children_start = arena.children.len();
    arena.children.extend(child_ids);
    let obj_keys_start = arena.obj_keys.len();
    arena.obj_keys.extend(keys);
    let n = &mut arena.nodes[root_id];
    n.kind = NodeKind::Object;
    n.children_start = children_start;
    n.children_len = arena.children.len() - children_start;
    n.obj_keys_start = obj_keys_start;
    n.obj_keys_len = n.children_len;
    n.object_len = Some(n.children_len);
    arena.root_id = root_id;
    arena.is_fileset = true;
    Ok(arena)
}

pub struct IndentIngest;

impl Ingest for IndentIngest {
    fn parse_one(
        bytes: Vec<u8>,
        cfg: &PriorityConfig,
    ) -> Result<JsonTreeArena> {
        build_indent_tree_arena_from_bytes(bytes, cfg)
    }
    fn parse_many(
        inputs: Vec<(String, Vec<u8>)>,
        cfg: &PriorityConfig,
    ) -> Result<JsonTreeArena> {
        build_indent_tree_arena_from_many(inputs, cfg)
    }
}

pub fn parse_indent_one(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    IndentIngest::parse_one(bytes, cfg)
}

pub fn parse_indent_many(
    inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    IndentIngest::parse_many(inputs, cfg)
}

#[cfg(test)]
mod tests {
    use super::build_indent_tree_arena_from_bytes;
    use crate::serialization::types::{ColorMode, OutputTemplate, Style};
    use crate::{PriorityConfig, RenderConfig};

    fn cfg_json() -> (RenderConfig, PriorityConfig) {
        let cfg = RenderConfig {
            template: OutputTemplate::Json,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: ColorMode::Off,
            color_enabled: false,
            style: Style::Strict,
            string_free_prefix_graphemes: None,
        };
        let prio = PriorityConfig::new(256, 256);
        (cfg, prio)
    }

    #[test]
    fn indent_basic_nesting_spaces() {
        let (_cfg, prio) = cfg_json();
        let input = b"a\n  b\n    c\n  d\n e\n".to_vec();
        let arena = build_indent_tree_arena_from_bytes(input, &prio).unwrap();
        // Root is array with entries a, e (since 'd' is sibling under a)
        assert_eq!(
            arena.nodes[arena.root_id].kind,
            crate::order::NodeKind::Array
        );
        assert!(arena.nodes[arena.root_id].children_len >= 2);
    }
}

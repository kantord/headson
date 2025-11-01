use anyhow::Result;
use std::borrow::Cow;

use crate::PriorityConfig;
use crate::order::NodeKind;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

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
}

impl TextArenaBuilder {
    fn new(array_cap: usize) -> Self {
        Self {
            arena: JsonTreeArena::default(),
            array_cap,
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

    fn push_array_of_lines(
        &mut self,
        lines: impl IntoIterator<Item = String>,
        total: usize,
    ) -> usize {
        let id = self.push_default();
        let kept = total.min(self.array_cap);
        let mut pushed = 0usize;
        for (i, line) in lines.into_iter().enumerate() {
            if i >= kept {
                break;
            }
            let child = self.push_string(line);
            self.arena.children.push(child);
            pushed += 1;
        }
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        // children for this array were appended after previous nodes; compute start = len(children) - pushed
        n.children_start = self.arena.children.len().saturating_sub(pushed);
        n.children_len = pushed;
        n.array_len = Some(total);
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
pub fn build_text_tree_arena_from_bytes(
    bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let lossy = String::from_utf8_lossy(&bytes);
    let norm = normalize_newlines(&lossy);
    // split_terminator keeps no trailing empty item for trailing newline
    let lines_vec: Vec<String> = norm
        .split_terminator('\n')
        .map(std::string::ToString::to_string)
        .collect();
    let total = lines_vec.len();
    let mut b = TextArenaBuilder::new(config.array_max_items);
    let root_id = b.push_array_of_lines(lines_vec, total);
    let mut a = b.finish();
    a.root_id = root_id;
    Ok(a)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "Signature matches other ingest helpers and trait expectations"
)]
pub fn build_text_tree_arena_from_many(
    mut inputs: Vec<(String, Vec<u8>)>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let mut b = TextArenaBuilder::new(config.array_max_items);
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
        let child_id = b.push_array_of_lines(lines_vec, total);
        keys.push(key);
        children_ids.push(child_id);
    }
    let root_id = b.push_object_root(keys, children_ids);
    let mut a = b.finish();
    a.root_id = root_id;
    a.is_fileset = true;
    Ok(a)
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
}

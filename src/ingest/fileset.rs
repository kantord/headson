use crate::order::NodeKind;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

use super::IngestOutput;
use super::formats::{
    json::build_json_tree_arena_from_slice,
    text::{
        build_text_tree_arena_from_bytes,
        build_text_tree_arena_from_bytes_with_mode,
    },
    yaml::build_yaml_tree_arena_from_bytes,
};
use crate::PriorityConfig;

/// Input descriptor for a single file in a multi-format fileset ingest.
#[derive(Debug)]
pub struct FilesetInput {
    pub name: String,
    pub bytes: Vec<u8>,
    pub kind: FilesetInputKind,
}

#[derive(Debug)]
pub(crate) struct FilesetEntry {
    pub name: String,
    pub arena: JsonTreeArena,
    pub suppressed: bool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FilesetInputKind {
    Json,
    Yaml,
    Text { atomic_lines: bool },
}

/// Parse a fileset and return any parse warnings.
pub fn parse_fileset_multi(
    inputs: Vec<FilesetInput>,
    cfg: &PriorityConfig,
) -> IngestOutput {
    let mut entries: Vec<FilesetEntry> = Vec::with_capacity(inputs.len());
    let mut warnings: Vec<String> = Vec::new();
    for FilesetInput {
        name,
        mut bytes,
        kind,
    } in inputs
    {
        let (arena, suppressed) = match kind {
            FilesetInputKind::Json => parse_or_empty(
                &name,
                &mut bytes,
                cfg,
                &mut warnings,
                "JSON",
                |bytes, cfg| build_json_tree_arena_from_slice(bytes, cfg),
            ),
            FilesetInputKind::Yaml => parse_or_empty(
                &name,
                &bytes,
                cfg,
                &mut warnings,
                "YAML",
                |bytes, cfg| build_yaml_tree_arena_from_bytes(bytes, cfg),
            ),
            FilesetInputKind::Text { atomic_lines } => {
                (parse_text_bytes(&bytes, cfg, atomic_lines), false)
            }
        };
        entries.push(FilesetEntry {
            name,
            arena,
            suppressed,
        });
    }
    IngestOutput {
        arena: build_fileset_root(entries),
        warnings,
    }
}

fn parse_or_empty<B, F>(
    name: &str,
    bytes: B,
    cfg: &PriorityConfig,
    warnings: &mut Vec<String>,
    label: &str,
    parse: F,
) -> (JsonTreeArena, bool)
where
    F: FnOnce(B, &PriorityConfig) -> anyhow::Result<JsonTreeArena>,
{
    match parse(bytes, cfg) {
        Ok(arena) => (arena, false),
        Err(err) => {
            warnings.push(format!("Failed to parse {name} as {label}: {err}"));
            (empty_object_arena(), true)
        }
    }
}

fn parse_text_bytes(
    bytes: &[u8],
    cfg: &PriorityConfig,
    atomic_lines: bool,
) -> JsonTreeArena {
    if atomic_lines {
        build_text_tree_arena_from_bytes_with_mode(bytes, cfg, true)
    } else {
        build_text_tree_arena_from_bytes(bytes, cfg)
    }
}

fn empty_object_arena() -> JsonTreeArena {
    let mut arena = JsonTreeArena::default();
    arena.nodes.push(JsonTreeNode {
        kind: NodeKind::Object,
        object_len: Some(0),
        ..JsonTreeNode::default()
    });
    arena.root_id = 0;
    arena
}

pub(crate) fn build_fileset_root(
    mut entries: Vec<FilesetEntry>,
) -> JsonTreeArena {
    let mut arena = JsonTreeArena {
        root_id: 0,
        is_fileset: true,
        ..JsonTreeArena::default()
    };
    arena.nodes.push(JsonTreeNode {
        kind: NodeKind::Object,
        ..JsonTreeNode::default()
    });

    let mut root_children: Vec<usize> = Vec::with_capacity(entries.len());
    let mut root_keys: Vec<String> = Vec::with_capacity(entries.len());

    for FilesetEntry {
        name,
        arena: child,
        suppressed,
    } in entries.drain(..)
    {
        let child_root = append_subtree(&mut arena, child);
        if let Some(node) = arena.nodes.get_mut(child_root) {
            node.fileset_suppressed = suppressed;
        }
        root_children.push(child_root);
        root_keys.push(name);
    }

    let children_start = arena.children.len();
    arena.children.extend(root_children.iter().copied());
    let obj_keys_start = arena.obj_keys.len();
    arena.obj_keys.extend(root_keys);

    {
        let root = &mut arena.nodes[arena.root_id];
        root.children_start = children_start;
        root.children_len = root_children.len();
        root.obj_keys_start = obj_keys_start;
        root.obj_keys_len = root.children_len;
        root.object_len = Some(root.children_len);
    }
    arena
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Tree merge touches multiple parallel arrays and offsets; easier to follow inline"
)]
fn append_subtree(dest: &mut JsonTreeArena, src: JsonTreeArena) -> usize {
    let node_offset = dest.nodes.len();
    let child_offset = dest.children.len();
    let obj_key_offset = dest.obj_keys.len();
    let arr_idx_offset = dest.arr_indices.len();
    let root_id = src.root_id;
    let JsonTreeArena {
        nodes,
        children,
        obj_keys,
        arr_indices,
        code_lines,
        ..
    } = src;

    dest.nodes.extend(nodes);
    for node in dest.nodes.iter_mut().skip(node_offset) {
        if node.children_len > 0 {
            node.children_start += child_offset;
        }
        if node.obj_keys_len > 0 {
            node.obj_keys_start += obj_key_offset;
        }
        if node.arr_indices_len > 0 {
            node.arr_indices_start += arr_idx_offset;
        }
    }

    dest.children
        .extend(children.into_iter().map(|child| child + node_offset));
    dest.obj_keys.extend(obj_keys);
    dest.arr_indices.extend(arr_indices);
    for (arena_idx, lines) in code_lines {
        dest.code_lines.insert(arena_idx + node_offset, lines);
    }

    node_offset + root_id
}

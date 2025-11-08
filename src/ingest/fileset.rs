use anyhow::Result;

use crate::order::NodeKind;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

use super::formats::{
    json::build_json_tree_arena_from_bytes,
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FilesetInputKind {
    Json,
    Yaml,
    Text { atomic_lines: bool },
}

pub fn parse_fileset_multi(
    inputs: Vec<FilesetInput>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let mut arenas: Vec<(String, JsonTreeArena)> =
        Vec::with_capacity(inputs.len());
    for FilesetInput { name, bytes, kind } in inputs {
        let arena = match kind {
            FilesetInputKind::Json => {
                build_json_tree_arena_from_bytes(bytes, cfg)?
            }
            FilesetInputKind::Yaml => {
                build_yaml_tree_arena_from_bytes(bytes, cfg)?
            }
            FilesetInputKind::Text { atomic_lines } => {
                if atomic_lines {
                    build_text_tree_arena_from_bytes_with_mode(
                        bytes, cfg, true,
                    )?
                } else {
                    build_text_tree_arena_from_bytes(bytes, cfg)?
                }
            }
        };
        arenas.push((name, arena));
    }
    Ok(build_fileset_root(arenas))
}

pub(crate) fn build_fileset_root(
    mut items: Vec<(String, JsonTreeArena)>,
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

    let mut root_children: Vec<usize> = Vec::with_capacity(items.len());
    let mut root_keys: Vec<String> = Vec::with_capacity(items.len());

    for (key, child) in items.drain(..) {
        let child_root = append_subtree(&mut arena, child);
        root_children.push(child_root);
        root_keys.push(key);
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

use anyhow::{Result, anyhow};

use crate::PriorityConfig;
use crate::order::NodeKind;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

use super::Ingest;
use yaml_rust2::Yaml;

pub fn build_yaml_tree_arena_from_bytes(
    bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let s = String::from_utf8(bytes)
        .map_err(|_| anyhow!("input is not valid UTF-8 text"))?;
    let docs = yaml_rust2::YamlLoader::load_from_str(&s)?;
    let mut b = YamlArenaBuilder::new(config.array_max_items);
    let root_id = if docs.len() <= 1 {
        match docs.first() {
            Some(doc) => b.build(doc),
            None => b.build(&Yaml::Array(vec![])),
        }
    } else {
        // Multi-doc YAML in a single input -> wrap into an array root.
        let mut children: Vec<usize> = Vec::with_capacity(docs.len());
        for d in &docs {
            children.push(b.build(d));
        }
        b.push_array(children, docs.len())
    };
    let mut arena = b.finish();
    arena.root_id = root_id;
    Ok(arena)
}

pub fn build_yaml_tree_arena_from_many(
    mut inputs: Vec<(String, Vec<u8>)>,
    config: &PriorityConfig,
) -> Result<JsonTreeArena> {
    let mut b = YamlArenaBuilder::new(config.array_max_items);
    let mut keys: Vec<String> = Vec::with_capacity(inputs.len());
    let mut children: Vec<usize> = Vec::with_capacity(inputs.len());
    for (key, bytes) in inputs.drain(..) {
        let s = String::from_utf8(bytes)
            .map_err(|_| anyhow!("input is not valid UTF-8 text"))?;
        let docs = yaml_rust2::YamlLoader::load_from_str(&s)?;
        let child_id = if docs.len() <= 1 {
            match docs.first() {
                Some(doc) => b.build(doc),
                None => b.build(&Yaml::Array(vec![])),
            }
        } else {
            let mut arr_children: Vec<usize> = Vec::with_capacity(docs.len());
            for d in &docs {
                arr_children.push(b.build(d));
            }
            b.push_array(arr_children, docs.len())
        };
        keys.push(key);
        children.push(child_id);
    }
    let root_id = b.push_object_root(keys, children);
    let mut arena = b.finish();
    arena.root_id = root_id;
    arena.is_fileset = true;
    Ok(arena)
}

struct YamlArenaBuilder {
    arena: JsonTreeArena,
    array_cap: usize,
}

impl YamlArenaBuilder {
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

    fn push_object_root(
        &mut self,
        keys: Vec<String>,
        children: Vec<usize>,
    ) -> usize {
        let id = self.push_default();
        let count = keys.len().min(children.len());
        self.finish_object(id, count, children, keys);
        id
    }

    fn push_array(&mut self, children: Vec<usize>, total_len: usize) -> usize {
        let id = self.push_default();
        let kept = children.len().min(self.array_cap);
        let kept_children =
            children.into_iter().take(kept).collect::<Vec<_>>();
        self.finish_array(id, kept, total_len, kept_children);
        id
    }

    fn finish_array(
        &mut self,
        id: usize,
        kept: usize,
        total: usize,
        local_children: Vec<usize>,
    ) {
        let children_start = self.arena.children.len();
        self.arena.children.extend(local_children);

        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = kept;
        n.array_len = Some(total);
        // We always keep a contiguous prefix; avoid storing arr_indices
        n.arr_indices_start = 0;
        n.arr_indices_len = 0;
    }

    fn finish_object(
        &mut self,
        id: usize,
        count: usize,
        local_children: Vec<usize>,
        local_keys: Vec<String>,
    ) {
        let children_start = self.arena.children.len();
        let obj_keys_start = self.arena.obj_keys.len();
        self.arena.children.extend(local_children);
        self.arena.obj_keys.extend(local_keys);
        let n = &mut self.arena.nodes[id];
        n.kind = NodeKind::Object;
        n.children_start = children_start;
        n.children_len = count;
        n.obj_keys_start = obj_keys_start;
        n.obj_keys_len = count;
        n.object_len = Some(count);
    }

    fn build(&mut self, y: &Yaml) -> usize {
        match y {
            Yaml::Array(v) => {
                let total = v.len();
                let kept = total.min(self.array_cap);
                let mut child_ids = Vec::with_capacity(kept);
                for item in v.iter().take(kept) {
                    child_ids.push(self.build(item));
                }
                self.push_array(child_ids, total)
            }
            Yaml::Hash(hm) => {
                let mut keys: Vec<String> = Vec::with_capacity(hm.len());
                let mut children: Vec<usize> = Vec::with_capacity(hm.len());
                for (k, v) in hm.iter() {
                    let key = stringify_yaml_key(k);
                    let cid = self.build(v);
                    keys.push(key);
                    children.push(cid);
                }
                self.push_object_root(keys, children)
            }
            Yaml::String(s) => {
                let id = self.push_default();
                let n = &mut self.arena.nodes[id];
                n.kind = NodeKind::String;
                n.string_value = Some(s.clone());
                id
            }
            Yaml::Integer(i) => {
                let id = self.push_default();
                let n = &mut self.arena.nodes[id];
                n.kind = NodeKind::Number;
                n.atomic_token = Some(i.to_string());
                id
            }
            Yaml::Real(s) => {
                let id = self.push_default();
                let n = &mut self.arena.nodes[id];
                n.kind = NodeKind::Number;
                n.atomic_token = Some(s.clone());
                id
            }
            Yaml::Boolean(b) => {
                let id = self.push_default();
                let n = &mut self.arena.nodes[id];
                n.kind = NodeKind::Bool;
                n.atomic_token =
                    Some(if *b { "true" } else { "false" }.to_string());
                id
            }
            Yaml::Null | Yaml::BadValue => {
                let id = self.push_default();
                let n = &mut self.arena.nodes[id];
                n.kind = NodeKind::Null;
                n.atomic_token = Some("null".to_string());
                id
            }
            // Represent aliases as a fixed string to avoid unstable parser IDs
            // and keep output deterministic.
            Yaml::Alias(_n) => {
                let id = self.push_default();
                let node = &mut self.arena.nodes[id];
                node.kind = NodeKind::String;
                node.string_value = Some("*alias".to_string());
                id
            }
        }
    }
}

fn stringify_yaml_key(k: &Yaml) -> String {
    match k {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
        Yaml::Null | Yaml::BadValue => "null".to_string(),
        Yaml::Array(_) | Yaml::Hash(_) | Yaml::Alias(_) => {
            "<complex>".to_string()
        }
    }
}

pub struct YamlIngest;

impl Ingest for YamlIngest {
    fn parse_one(
        bytes: Vec<u8>,
        cfg: &PriorityConfig,
    ) -> Result<JsonTreeArena> {
        build_yaml_tree_arena_from_bytes(bytes, cfg)
    }

    fn parse_many(
        inputs: Vec<(String, Vec<u8>)>,
        cfg: &PriorityConfig,
    ) -> Result<JsonTreeArena> {
        build_yaml_tree_arena_from_many(inputs, cfg)
    }
}

/// Convenience functions for the YAML ingest path.
pub fn parse_yaml_one(
    bytes: Vec<u8>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    YamlIngest::parse_one(bytes, cfg)
}

pub fn parse_yaml_many(
    inputs: Vec<(String, Vec<u8>)>,
    cfg: &PriorityConfig,
) -> Result<JsonTreeArena> {
    YamlIngest::parse_many(inputs, cfg)
}

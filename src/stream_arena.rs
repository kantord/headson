use crate::queue::NodeKind;
use crate::queue::PQConfig;
use anyhow::Result;
use serde::Deserializer;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

#[derive(Debug, Default, Clone)]
pub struct StreamArena {
    pub nodes: Vec<SaNode>,
    pub children: Vec<usize>,
    pub obj_keys: Vec<String>,
    pub root_id: usize,
}

#[derive(Debug, Clone)]
pub struct SaNode {
    pub kind: NodeKind,
    pub number_value: Option<serde_json::Number>,
    pub bool_value: Option<bool>,
    pub string_value: Option<String>,
    pub children_start: usize,
    pub children_len: usize,
    pub obj_keys_start: usize,
    pub obj_keys_len: usize,
    pub array_len: Option<usize>,
    pub object_len: Option<usize>,
}

impl Default for SaNode {
    fn default() -> Self {
        Self {
            kind: NodeKind::Null,
            number_value: None,
            bool_value: None,
            string_value: None,
            children_start: 0,
            children_len: 0,
            obj_keys_start: 0,
            obj_keys_len: 0,
            array_len: None,
            object_len: None,
        }
    }
}

use std::cell::RefCell;

struct ArenaCell {
    arena: RefCell<StreamArena>,
    array_cap: usize,
}

impl ArenaCell {
    fn push_default(&self) -> usize {
        let mut a = self.arena.borrow_mut();
        let id = a.nodes.len();
        a.nodes.push(SaNode::default());
        id
    }
}

struct NodeSeed<'a> {
    cell: &'a ArenaCell,
}

impl<'de> DeserializeSeed<'de> for NodeSeed<'_> {
    type Value = usize; // node id

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NodeVisitor<'b> {
            c: &'b ArenaCell,
        }
        impl<'de> Visitor<'de> for NodeVisitor<'_> {
            type Value = usize;

            fn expecting(
                &self,
                f: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                write!(f, "any JSON value")
            }

            fn visit_bool<E>(
                self,
                v: bool,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = self.c.push_default();
                {
                    let mut a = self.c.arena.borrow_mut();
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::Bool;
                    n.bool_value = Some(v);
                }
                Ok(id)
            }

            fn visit_i64<E>(
                self,
                v: i64,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = self.c.push_default();
                {
                    let mut a = self.c.arena.borrow_mut();
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::Number;
                    n.number_value = Some(serde_json::Number::from(v));
                }
                Ok(id)
            }

            fn visit_u64<E>(
                self,
                v: u64,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = self.c.push_default();
                {
                    let mut a = self.c.arena.borrow_mut();
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::Number;
                    n.number_value = Some(serde_json::Number::from(v));
                }
                Ok(id)
            }

            fn visit_f64<E>(
                self,
                v: f64,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = self.c.push_default();
                {
                    let mut a = self.c.arena.borrow_mut();
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::Number;
                    let num = serde_json::Number::from_f64(v)
                        .ok_or_else(|| E::custom("invalid f64"))?;
                    n.number_value = Some(num);
                }
                Ok(id)
            }

            fn visit_str<E>(
                self,
                v: &str,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = self.c.push_default();
                {
                    let mut a = self.c.arena.borrow_mut();
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::String;
                    n.string_value = Some(v.to_string());
                }
                Ok(id)
            }

            fn visit_string<E>(
                self,
                v: String,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = self.c.push_default();
                {
                    let mut a = self.c.arena.borrow_mut();
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::String;
                    n.string_value = Some(v);
                }
                Ok(id)
            }

            fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let id = self.c.push_default();
                {
                    let mut a = self.c.arena.borrow_mut();
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::Null;
                }
                Ok(id)
            }

            fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_unit()
            }

            fn visit_seq<A>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let id = self.c.push_default();
                let mut local_children: Vec<usize> = Vec::new();
                let mut kept = 0usize;
                let mut total = 0usize;
                // Materialize up to array_cap children
                while kept < self.c.array_cap {
                    let next = {
                        let seed = NodeSeed { cell: self.c };
                        seq.next_element_seed(seed)?
                    };
                    match next {
                        Some(cid) => {
                            local_children.push(cid);
                            kept += 1;
                            total += 1;
                        }
                        None => break,
                    }
                }
                // Consume rest but ignore, counting only
                while (seq.next_element::<IgnoredAny>()?).is_some() {
                    total += 1;
                }
                let children_start = {
                    let a = self.c.arena.borrow();
                    a.children.len()
                };
                {
                    let mut a = self.c.arena.borrow_mut();
                    a.children.extend(local_children);
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::Array;
                    n.children_start = children_start;
                    n.children_len = kept;
                    n.array_len = Some(total);
                }
                Ok(id)
            }

            fn visit_map<A>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let id = self.c.push_default();
                let mut local_children: Vec<usize> = Vec::new();
                let mut local_keys: Vec<String> = Vec::new();
                let mut count = 0usize;
                while let Some(key) = map.next_key::<String>()? {
                    let cid: usize = {
                        let seed = NodeSeed { cell: self.c };
                        map.next_value_seed(seed)?
                    };
                    local_children.push(cid);
                    local_keys.push(key);
                    count += 1;
                }
                let (children_start, obj_keys_start) = {
                    let a = self.c.arena.borrow();
                    (a.children.len(), a.obj_keys.len())
                };
                {
                    let mut a = self.c.arena.borrow_mut();
                    a.children.extend(local_children);
                    a.obj_keys.extend(local_keys);
                    let n = &mut a.nodes[id];
                    n.kind = NodeKind::Object;
                    n.children_start = children_start;
                    n.children_len = count;
                    n.obj_keys_start = obj_keys_start;
                    n.obj_keys_len = count;
                    n.object_len = Some(count);
                }
                Ok(id)
            }
        }
        deserializer.deserialize_any(NodeVisitor { c: self.cell })
    }
}

// Build a compact arena in a single pass using a serde Visitor.
// Arrays are capped at `cfg.array_max_items` during parse; we still record the
// total length to report omissions accurately later.
#[cfg(test)]
pub fn build_stream_arena(input: &str, cfg: &PQConfig) -> Result<StreamArena> {
    // Use simd-json serde deserializer, parsing from a mutable buffer
    let mut bytes = input.as_bytes().to_vec();
    let mut de = simd_json::Deserializer::from_slice(&mut bytes)?;
    let cell = ArenaCell {
        arena: RefCell::new(StreamArena::default()),
        array_cap: cfg.array_max_items,
    };
    let root_id: usize = {
        let seed = NodeSeed { cell: &cell };
        seed.deserialize(&mut de)?
    };
    {
        let mut a = cell.arena.borrow_mut();
        a.root_id = root_id;
    }
    Ok(cell.arena.into_inner())
}

// Variant that avoids copying: accepts owned bytes and parses in-place.
pub fn build_stream_arena_from_bytes(
    mut bytes: Vec<u8>,
    cfg: &PQConfig,
) -> Result<StreamArena> {
    let mut de = simd_json::Deserializer::from_slice(&mut bytes)?;
    let cell = ArenaCell {
        arena: RefCell::new(StreamArena::default()),
        array_cap: cfg.array_max_items,
    };
    let root_id: usize = {
        let seed = NodeSeed { cell: &cell };
        seed.deserialize(&mut de)?
    };
    let mut arena = cell.arena.into_inner();
    arena.root_id = root_id;
    // We do not borrow into `bytes`, so it can be dropped safely here.
    Ok(arena)
}

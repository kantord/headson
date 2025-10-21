use crate::order::NodeKind;
use crate::order::PriorityConfig;
use crate::utils::arena::{ArenaNode, StreamArena};
use anyhow::Result;
use serde::Deserializer;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

// Arena types live under utils::arena

use std::cell::RefCell;

struct ArenaCell {
    arena: RefCell<StreamArena>,
    array_cap: usize,
}

impl ArenaCell {
    fn push_default(&self) -> usize {
        let mut a = self.arena.borrow_mut();
        let id = a.nodes.len();
        a.nodes.push(ArenaNode::default());
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
        deserializer.deserialize_any(NodeVisitor { cell: self.cell })
    }
}

struct NodeVisitor<'b> {
    cell: &'b ArenaCell,
}

impl<'de> Visitor<'de> for NodeVisitor<'_> {
    type Value = usize;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "any JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let id = self.cell.push_default();
        {
            let mut a = self.cell.arena.borrow_mut();
            let n = &mut a.nodes[id];
            n.kind = NodeKind::Bool;
            n.bool_value = Some(v);
        }
        Ok(id)
    }

    fn visit_i64<E>(self, v: i64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let id = self.cell.push_default();
        {
            let mut a = self.cell.arena.borrow_mut();
            let n = &mut a.nodes[id];
            n.kind = NodeKind::Number;
            n.number_value = Some(serde_json::Number::from(v));
        }
        Ok(id)
    }

    fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let id = self.cell.push_default();
        {
            let mut a = self.cell.arena.borrow_mut();
            let n = &mut a.nodes[id];
            n.kind = NodeKind::Number;
            n.number_value = Some(serde_json::Number::from(v));
        }
        Ok(id)
    }

    fn visit_f64<E>(self, v: f64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let id = self.cell.push_default();
        {
            let mut a = self.cell.arena.borrow_mut();
            let n = &mut a.nodes[id];
            n.kind = NodeKind::Number;
            let num = serde_json::Number::from_f64(v)
                .ok_or_else(|| E::custom("invalid f64"))?;
            n.number_value = Some(num);
        }
        Ok(id)
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let id = self.cell.push_default();
        {
            let mut a = self.cell.arena.borrow_mut();
            let n = &mut a.nodes[id];
            n.kind = NodeKind::String;
            n.string_value = Some(v.to_string());
        }
        Ok(id)
    }

    fn visit_string<E>(self, v: String) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let id = self.cell.push_default();
        {
            let mut a = self.cell.arena.borrow_mut();
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
        let id = self.cell.push_default();
        {
            let mut a = self.cell.arena.borrow_mut();
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
        let id = self.cell.push_default();
        let mut local_children: Vec<usize> = Vec::new();
        let mut kept = 0usize;
        let mut total = 0usize;
        while kept < self.cell.array_cap {
            let next = {
                let seed = NodeSeed { cell: self.cell };
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
        while (seq.next_element::<IgnoredAny>()?).is_some() {
            total += 1;
        }
        let children_start = {
            let a = self.cell.arena.borrow();
            a.children.len()
        };
        {
            let mut a = self.cell.arena.borrow_mut();
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
        let id = self.cell.push_default();
        let mut local_children: Vec<usize> = Vec::new();
        let mut local_keys: Vec<String> = Vec::new();
        let mut count = 0usize;
        while let Some(key) = map.next_key::<String>()? {
            let cid: usize = {
                let seed = NodeSeed { cell: self.cell };
                map.next_value_seed(seed)?
            };
            local_children.push(cid);
            local_keys.push(key);
            count += 1;
        }
        let (children_start, obj_keys_start) = {
            let a = self.cell.arena.borrow();
            (a.children.len(), a.obj_keys.len())
        };
        {
            let mut a = self.cell.arena.borrow_mut();
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

// Build a compact arena in a single pass using a serde Visitor.
// Arrays are capped at `config.array_max_items` during parse; we still record the
// total length to report omissions accurately later.
#[cfg(test)]
pub fn build_stream_arena(
    input: &str,
    config: &PriorityConfig,
) -> Result<StreamArena> {
    // Delegate to the in-place bytes variant to avoid duplicate logic
    build_stream_arena_from_bytes(input.as_bytes().to_vec(), config)
}

// Variant that avoids copying: accepts owned bytes and parses in-place.
pub fn build_stream_arena_from_bytes(
    mut bytes: Vec<u8>,
    config: &PriorityConfig,
) -> Result<StreamArena> {
    let mut de = simd_json::Deserializer::from_slice(&mut bytes)?;
    let cell = ArenaCell {
        arena: RefCell::new(StreamArena::default()),
        array_cap: config.array_max_items,
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

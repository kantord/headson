use serde::Deserializer;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::cell::RefCell;

use crate::order::NodeKind;
use crate::utils::tree_arena::{JsonTreeArena, JsonTreeNode};

#[derive(Default)]
pub(crate) struct JsonTreeBuilder {
    arena: RefCell<JsonTreeArena>,
    pub(crate) array_cap: usize,
}

impl JsonTreeBuilder {
    pub(crate) fn new(array_cap: usize) -> Self {
        Self {
            arena: RefCell::new(JsonTreeArena::default()),
            array_cap,
        }
    }

    pub(crate) fn seed(&self) -> NodeSeed<'_> {
        NodeSeed { b: self }
    }

    pub(crate) fn finish(self) -> JsonTreeArena {
        self.arena.into_inner()
    }

    // Create an object node from provided keys and child ids and return its id.
    pub(crate) fn push_object_root(
        &self,
        keys: Vec<String>,
        children: Vec<usize>,
    ) -> usize {
        let id = self.push_default();
        let count = keys.len().min(children.len());
        self.finish_object(id, count, children, keys);
        id
    }

    fn push_default(&self) -> usize {
        let mut a = self.arena.borrow_mut();
        let id = a.nodes.len();
        a.nodes.push(JsonTreeNode::default());
        id
    }

    fn push_with(&self, set: impl FnOnce(&mut JsonTreeNode)) -> usize {
        let id = self.push_default();
        let mut a = self.arena.borrow_mut();
        let n = &mut a.nodes[id];
        set(n);
        id
    }

    fn push_number<N>(&self, v: N) -> usize
    where
        serde_json::Number: From<N>,
    {
        self.push_with(|n| {
            n.kind = NodeKind::Number;
            n.number_value = Some(serde_json::Number::from(v));
        })
    }

    fn push_bool(&self, v: bool) -> usize {
        self.push_with(|n| {
            n.kind = NodeKind::Bool;
            n.bool_value = Some(v);
        })
    }
    fn push_string_owned(&self, s: String) -> usize {
        self.push_with(|n| {
            n.kind = NodeKind::String;
            n.string_value = Some(s);
        })
    }
    fn push_null(&self) -> usize {
        self.push_with(|n| {
            n.kind = NodeKind::Null;
        })
    }

    fn finish_array(
        &self,
        id: usize,
        kept: usize,
        total: usize,
        local_children: Vec<usize>,
        local_indices: Vec<usize>,
    ) {
        let mut a = self.arena.borrow_mut();
        let children_start = a.children.len();
        a.children.extend(local_children);
        let arr_indices_start = if local_indices.is_empty() {
            0
        } else {
            let start = a.arr_indices.len();
            a.arr_indices.extend(local_indices);
            start
        };
        // Compute pushed count before taking a mutable borrow to the node
        let pushed = a.arr_indices.len().saturating_sub(arr_indices_start);
        let n = &mut a.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = kept;
        n.array_len = Some(total);
        n.arr_indices_start = arr_indices_start;
        // When no indices were pushed, mark len=0 to indicate contiguous 0..kept
        n.arr_indices_len = pushed.min(kept);
    }

    fn finish_object(
        &self,
        id: usize,
        count: usize,
        local_children: Vec<usize>,
        local_keys: Vec<String>,
    ) {
        let mut a = self.arena.borrow_mut();
        let children_start = a.children.len();
        let obj_keys_start = a.obj_keys.len();
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
}

pub(crate) struct NodeSeed<'a> {
    pub(crate) b: &'a JsonTreeBuilder,
}

impl<'de> DeserializeSeed<'de> for NodeSeed<'_> {
    type Value = usize;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NodeVisitor { b: self.b })
    }
}

struct NodeVisitor<'b> {
    b: &'b JsonTreeBuilder,
}

impl<'de> Visitor<'de> for NodeVisitor<'_> {
    type Value = usize;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "any JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.b.push_bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.b.push_number(v))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.b.push_number(v))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let num = serde_json::Number::from_f64(v)
            .ok_or_else(|| E::custom("invalid f64"))?;
        let id = self.b.push_with(|n| {
            n.kind = NodeKind::Number;
            n.number_value = Some(num);
        });
        Ok(id)
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.b.push_string_owned(v.to_owned()))
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.b.push_string_owned(v))
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.b.push_null())
    }
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_unit()
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Single-pass head+tail selection with ring buffer adds a few branches"
    )]
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let id = self.b.push_default();
        let mut local_children: Vec<usize> = Vec::new();
        let low = seq.size_hint().unwrap_or(0);
        local_children.reserve(low.min(self.b.array_cap));
        let mut kept = 0usize;
        let mut total = 0usize;
        while kept < self.b.array_cap {
            let next = {
                let seed = self.b.seed();
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
        // Indices are contiguous 0..kept; omit storing them to avoid overhead.
        let local_indices: Vec<usize> = Vec::new();
        self.b
            .finish_array(id, kept, total, local_children, local_indices);
        Ok(id)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let id = self.b.push_default();
        let mut local_children: Vec<usize> = Vec::new();
        let mut local_keys: Vec<String> = Vec::new();
        let low = map.size_hint().unwrap_or(0);
        local_children.reserve(low);
        local_keys.reserve(low);
        let mut count = 0usize;
        while let Some(key) = map.next_key::<String>()? {
            let cid: usize = {
                let seed = self.b.seed();
                map.next_value_seed(seed)?
            };
            local_children.push(cid);
            local_keys.push(key);
            count += 1;
        }
        self.b.finish_object(id, count, local_children, local_keys);
        Ok(id)
    }
}

impl JsonTreeBuilder {}

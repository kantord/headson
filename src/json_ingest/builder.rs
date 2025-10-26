use serde::Deserializer;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value as JsonValue;
use std::cell::RefCell;
use std::collections::VecDeque;

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
        let arr_indices_start = a.arr_indices.len();
        a.children.extend(local_children);
        a.arr_indices.extend(local_indices);
        let n = &mut a.nodes[id];
        n.kind = NodeKind::Array;
        n.children_start = children_start;
        n.children_len = kept;
        n.array_len = Some(total);
        n.arr_indices_start = arr_indices_start;
        n.arr_indices_len = kept;
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
        let cap = self.b.array_cap;
        let h = cap / 2 + (cap % 2); // ceil(cap/2) without overflow
        let t = cap / 2; // floor(cap/2)
        let mut local_children: Vec<usize> = Vec::new();
        let mut local_indices: Vec<usize> = Vec::new();
        let mut tail_vals: VecDeque<(usize, JsonValue)> = VecDeque::new();
        let mut total = 0usize;
        // Greedily keep head elements via builder seed; buffer potential tail as serde_json::Value.
        loop {
            if total < h {
                let seed = self.b.seed();
                match seq.next_element_seed(seed)? {
                    Some(cid) => {
                        local_children.push(cid);
                        local_indices.push(total);
                        total += 1;
                    }
                    None => break,
                }
            } else if t > 0 {
                match seq.next_element::<JsonValue>()? {
                    Some(v) => {
                        if tail_vals.len() == t {
                            tail_vals.pop_front();
                        }
                        tail_vals.push_back((total, v));
                        total += 1;
                    }
                    None => break,
                }
            } else {
                match seq.next_element::<IgnoredAny>()? {
                    Some(_) => total += 1,
                    None => break,
                }
            }
        }
        // Append tail nodes converted from buffered values.
        for (idx, v) in tail_vals.drain(..) {
            let cid = self.b.push_from_value(&v);
            local_children.push(cid);
            local_indices.push(idx);
        }
        let kept = local_children.len();
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

impl JsonTreeBuilder {
    pub(crate) fn push_from_value(&self, v: &JsonValue) -> usize {
        match v {
            JsonValue::Null => self.push_null(),
            JsonValue::Bool(b) => self.push_bool(*b),
            JsonValue::Number(n) => {
                // Clone serde_json::Number directly
                self.push_with(|node| {
                    node.kind = NodeKind::Number;
                    node.number_value = Some(n.clone());
                })
            }
            JsonValue::String(s) => self.push_string_owned(s.clone()),
            JsonValue::Array(arr) => {
                let id = self.push_default();
                let total = arr.len();
                let kept = total.min(self.array_cap);
                let mut children: Vec<usize> = Vec::with_capacity(kept);
                let mut indices: Vec<usize> = Vec::with_capacity(kept);
                for (i, child) in arr.iter().enumerate().take(kept) {
                    let cid = self.push_from_value(child);
                    children.push(cid);
                    indices.push(i);
                }
                self.finish_array(id, kept, total, children, indices);
                id
            }
            JsonValue::Object(map) => {
                let id = self.push_default();
                let count = map.len();
                let mut children: Vec<usize> = Vec::with_capacity(count);
                let mut keys: Vec<String> = Vec::with_capacity(count);
                for (k, child) in map.iter() {
                    let cid = self.push_from_value(child);
                    children.push(cid);
                    keys.push(k.clone());
                }
                self.finish_object(id, count, children, keys);
                id
            }
        }
    }
}

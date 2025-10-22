use serde_json;

#[derive(Clone, Debug)]
pub struct PriorityConfig {
    pub max_string_graphemes: usize,
    pub array_max_items: usize,
}

impl PriorityConfig {
    pub fn new(max_string_graphemes: usize, array_max_items: usize) -> Self {
        Self {
            max_string_graphemes,
            array_max_items,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NodeKind {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RankedNode {
    pub node_id: NodeId,
    pub kind: NodeKind,
    pub key_in_object: Option<String>,
    pub number_value: Option<serde_json::Number>,
    pub bool_value: Option<bool>,
    pub string_value: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct NodeMetrics {
    pub array_len: Option<usize>,
    pub object_len: Option<usize>,
    pub string_len: Option<usize>,
    pub string_truncated: bool,
}

#[derive(Clone, Debug)]
pub struct PriorityOrder {
    pub metrics: Vec<NodeMetrics>,
    pub nodes: Vec<RankedNode>,
    // All ids in this structure are PQ ids (0..total_nodes).
    // They correspond to `NodeId.0` in `RankedNode` for convenience when indexing.
    pub parent: Vec<Option<NodeId>>, // parent[id] = parent id (PQ id)
    pub children: Vec<Vec<NodeId>>,  // children[id] = children ids (PQ ids)
    pub order: Vec<NodeId>, // ids sorted by ascending priority (PQ ids)
    pub total_nodes: usize,
}

pub const ROOT_PQ_ID: usize = 0;

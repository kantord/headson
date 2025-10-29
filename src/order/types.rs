#[derive(Clone, Debug)]
pub struct PriorityConfig {
    pub max_string_graphemes: usize,
    pub array_max_items: usize,
    pub prefer_tail_arrays: bool,
    // Array selection bias for partial renders.
    pub array_bias: ArrayBias,
    // Array pre-sampling strategy.
    pub array_sampler: ArraySamplerStrategy,
}

impl PriorityConfig {
    pub fn new(max_string_graphemes: usize, array_max_items: usize) -> Self {
        Self {
            max_string_graphemes,
            array_max_items,
            prefer_tail_arrays: false,
            array_bias: ArrayBias::HeadMidTail,
            array_sampler: ArraySamplerStrategy::Default,
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

// Classification of leaf nodes by truncatability semantics.
// Atomic: values that cannot be truncated (null, bool, number).
// String: values that can be truncated to a prefix during rendering.

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ObjectType {
    Object,
    Fileset,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArrayBias {
    Head,
    HeadMidTail,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArraySamplerStrategy {
    Default,
    Head,
    Tail,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RankedNode {
    Array {
        node_id: NodeId,
        key_in_object: Option<String>,
    },
    Object {
        node_id: NodeId,
        key_in_object: Option<String>,
    },
    // LongLeaf: truncatable string leaf with full value available.
    LongLeaf {
        node_id: NodeId,
        key_in_object: Option<String>,
        value: String,
    },
    // LongLeafPart: synthetic node for string-grapheme prioritization; holds no value.
    LongLeafPart {
        node_id: NodeId,
        key_in_object: Option<String>,
    },
    // AtomicLeaf: non-truncatable scalar, printed verbatim.
    AtomicLeaf {
        node_id: NodeId,
        key_in_object: Option<String>,
        token: String,
    },
}

impl RankedNode {
    pub fn node_id(&self) -> NodeId {
        match self {
            RankedNode::Array { node_id, .. }
            | RankedNode::Object { node_id, .. }
            | RankedNode::LongLeaf { node_id, .. }
            | RankedNode::LongLeafPart { node_id, .. }
            | RankedNode::AtomicLeaf { node_id, .. } => *node_id,
        }
    }
    pub fn key_in_object(&self) -> Option<&str> {
        match self {
            RankedNode::Array { key_in_object, .. }
            | RankedNode::Object { key_in_object, .. }
            | RankedNode::LongLeaf { key_in_object, .. }
            | RankedNode::LongLeafPart { key_in_object, .. }
            | RankedNode::AtomicLeaf { key_in_object, .. } => {
                key_in_object.as_deref()
            }
        }
    }
    pub fn display_kind(&self) -> NodeKind {
        match self {
            RankedNode::Array { .. } => NodeKind::Array,
            RankedNode::Object { .. } => NodeKind::Object,
            RankedNode::LongLeaf { .. }
            | RankedNode::LongLeafPart { .. }
            | RankedNode::AtomicLeaf { .. } => NodeKind::String,
        }
    }
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
    // For each PQ id, the original index within the parent array, when the
    // parent is an array. None for non-array parents and synthetic nodes.
    pub index_in_parent_array: Vec<Option<usize>>,
    pub by_priority: Vec<NodeId>, // ids sorted by ascending priority (PQ ids)
    pub total_nodes: usize,
    pub object_type: Vec<ObjectType>,
}

pub const ROOT_PQ_ID: usize = 0;

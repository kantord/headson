use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::scoring::DEFAULT_SAFETY_CAP;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Breadcrumb {
    pub file: String,
    pub path: String,
    pub count: u64,
    pub last_step: u64,
}

/// Novelty penalty for a previously-seen node: `ln(1 + count) * alpha^steps_ago`.
///
/// `steps_ago` is clamped before the `i32` conversion: a raw `as i32` cast of
/// a large `u64` could wrap negative, turning the decay into a huge
/// amplification for `alpha < 1`.
pub fn novelty_penalty(count: u64, steps_ago: u64, alpha: f64) -> f64 {
    let steps = i32::try_from(steps_ago).unwrap_or(i32::MAX);
    (1.0 + count as f64).ln() * alpha.powi(steps)
}

#[derive(Clone, Debug)]
pub struct ExploreContext {
    pub breadcrumbs: Vec<Breadcrumb>,
    pub current_step: u64,
    pub alpha: f64,
    /// Resolved absolute path of a single (non-fileset) input file, used as
    /// the breadcrumb `file` component. `None` for stdin and fileset inputs
    /// (fileset slots resolve their own per-file paths from input names).
    pub file: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PriorityConfig {
    pub max_string_graphemes: usize,
    pub array_max_items: usize,
    pub prefer_tail_arrays: bool,
    // Array selection bias for partial renders.
    pub array_bias: ArrayBias,
    // Array pre-sampling strategy.
    pub array_sampler: ArraySamplerStrategy,
    // True when a lines-only budget is active (line cap with no byte cap).
    // Indicates that rendering may favor structural breadth over deep string
    // expansion under line-capped previews.
    pub line_budget_only: bool,
    /// Hard ceiling on priority queue nodes to prevent degenerate inputs
    /// from exhausting memory/time. Default is 2,000,000.
    pub safety_cap: usize,
    /// Optional explore context for deprioritizing previously-seen nodes.
    pub explore: Option<ExploreContext>,
}

impl PriorityConfig {
    pub fn new(max_string_graphemes: usize, array_max_items: usize) -> Self {
        Self {
            max_string_graphemes,
            array_max_items,
            prefer_tail_arrays: false,
            array_bias: ArrayBias::HeadMidTail,
            array_sampler: ArraySamplerStrategy::Default,
            line_budget_only: false,
            safety_cap: DEFAULT_SAFETY_CAP,
            explore: None,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Priority tuning bundles several independent knobs"
    )]
    pub fn for_budget(
        max_string_graphemes: usize,
        per_file_budget: usize,
        prefer_tail_arrays: bool,
        array_sampler: ArraySamplerStrategy,
        line_budget_only: bool,
    ) -> Self {
        let array_max_items = if line_budget_only {
            usize::MAX
        } else {
            (per_file_budget / 2).max(1)
        };
        Self {
            max_string_graphemes,
            array_max_items,
            prefer_tail_arrays,
            array_bias: ArrayBias::HeadMidTail,
            array_sampler,
            line_budget_only,
            safety_cap: DEFAULT_SAFETY_CAP,
            explore: None,
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
    JsonlRoot,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArrayBias {
    Head,
    HeadMidTail,
    HeadTail,
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
    // SplittableLeaf: truncatable string leaf with full value available.
    SplittableLeaf {
        node_id: NodeId,
        key_in_object: Option<String>,
        value: String,
    },
    // LeafPart: synthetic node for string-grapheme prioritization; holds no value.
    LeafPart {
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
            | RankedNode::SplittableLeaf { node_id, .. }
            | RankedNode::LeafPart { node_id, .. }
            | RankedNode::AtomicLeaf { node_id, .. } => *node_id,
        }
    }
    pub fn key_in_object(&self) -> Option<&str> {
        match self {
            RankedNode::Array { key_in_object, .. }
            | RankedNode::Object { key_in_object, .. }
            | RankedNode::SplittableLeaf { key_in_object, .. }
            | RankedNode::LeafPart { key_in_object, .. }
            | RankedNode::AtomicLeaf { key_in_object, .. } => {
                key_in_object.as_deref()
            }
        }
    }
    pub fn display_kind(&self) -> NodeKind {
        match self {
            RankedNode::Array { .. } => NodeKind::Array,
            RankedNode::Object { .. } => NodeKind::Object,
            RankedNode::SplittableLeaf { .. }
            | RankedNode::LeafPart { .. }
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
    pub scores: Vec<u128>,
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
    pub code_lines: HashMap<usize, Arc<Vec<String>>>,
    // For filesets, preserve ingest order and suppression state for render slots.
    pub fileset_render_slots: Option<Vec<FilesetRenderSlot>>,
    /// True if the priority queue expansion hit the safety cap.
    pub safety_cap_hit: bool,
    /// FNV-1a Merkle hashes indexed by PQ node id, populated when explore
    /// penalty matching computed them, so shown-leaf collection can reuse
    /// the table instead of running a second full-tree hash pass.
    pub merkle_hashes: Option<Vec<u64>>,
}

#[derive(Copy, Clone, Debug)]
pub struct FilesetRenderSlot {
    pub id: NodeId,
    pub suppressed: bool,
}

impl PriorityOrder {
    pub fn fileset_render_slots(&self) -> Option<&[FilesetRenderSlot]> {
        self.fileset_render_slots.as_deref()
    }
}

pub const ROOT_PQ_ID: usize = 0;

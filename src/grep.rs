use anyhow::Result;
use regex::{Regex, RegexBuilder};

use crate::order::{
    NodeId, ObjectType, PriorityOrder, ROOT_PQ_ID, RankedNode,
};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum GrepShow {
    #[default]
    Matching,
    All,
}

/// Grep configuration threaded through the pipeline.
#[derive(Default)]
pub struct GrepConfig {
    pub regex: Option<Regex>,
    pub weak: bool,
    pub show: GrepShow,
}

pub fn build_grep_config(
    grep: Option<&str>,
    weak_grep: Option<&str>,
    grep_show: GrepShow,
) -> Result<GrepConfig> {
    match (grep, weak_grep) {
        (Some(_), Some(_)) => {
            anyhow::bail!("--grep and --weak-grep cannot be used together")
        }
        (Some(pat), None) => Ok(GrepConfig {
            regex: Some(RegexBuilder::new(pat).unicode(true).build()?),
            weak: false,
            show: grep_show,
        }),
        (None, Some(pat)) => Ok(GrepConfig {
            regex: Some(RegexBuilder::new(pat).unicode(true).build()?),
            weak: true,
            show: GrepShow::Matching,
        }),
        (None, None) => Ok(GrepConfig {
            regex: None,
            weak: false,
            show: GrepShow::Matching,
        }),
    }
}

pub(crate) struct GrepState {
    pub must_keep: Vec<bool>,
    pub must_keep_count: usize,
}

impl GrepState {
    pub fn is_enabled(&self) -> bool {
        self.must_keep_count > 0
    }
}

fn matches_ranked(
    order: &PriorityOrder,
    idx: usize,
    node: &RankedNode,
    re: &Regex,
) -> bool {
    let value_match = match node {
        RankedNode::SplittableLeaf { value, .. } => re.is_match(value),
        RankedNode::AtomicLeaf { token, .. } => re.is_match(token),
        _ => false,
    };
    if value_match {
        return true;
    }
    let key_match = node.key_in_object().is_some_and(|k| re.is_match(k));
    if !key_match {
        return false;
    }
    let is_fileset_child = order
        .object_type
        .get(ROOT_PQ_ID)
        .is_some_and(|t| *t == ObjectType::Fileset)
        && order
            .parent
            .get(idx)
            .and_then(|p| *p)
            .is_some_and(|p| p.0 == ROOT_PQ_ID);
    !is_fileset_child
}

fn mark_matches_and_ancestors(
    order: &PriorityOrder,
    re: &Regex,
    must_keep: &mut [bool],
) {
    for (idx, node) in order.nodes.iter().enumerate() {
        if !matches_ranked(order, idx, node, re) {
            continue;
        }
        let mut cursor = Some(NodeId(idx));
        while let Some(node_id) = cursor {
            let raw = node_id.0;
            if must_keep[raw] {
                break;
            }
            must_keep[raw] = true;
            cursor = order.parent.get(raw).and_then(|p| *p);
        }
    }
}

/// Find all nodes that match the regex (or whose keys match) and mark their
/// ancestor chain for guaranteed inclusion.
pub(crate) fn compute_grep_state(
    order: &PriorityOrder,
    grep: &GrepConfig,
) -> Option<GrepState> {
    let re = grep.regex.as_ref()?;
    let mut must_keep = vec![false; order.total_nodes];
    mark_matches_and_ancestors(order, re, &mut must_keep);
    let must_keep_count = must_keep.iter().filter(|b| **b).count();
    (must_keep_count > 0).then_some(GrepState {
        must_keep,
        must_keep_count,
    })
}

/// Reorder priority so must-keep nodes are visited first, preserving the
/// existing relative order within each bucket.
pub(crate) fn reorder_priority_with_must_keep(
    order: &mut PriorityOrder,
    must_keep: &[bool],
) {
    let mut seen = vec![false; order.total_nodes];
    let mut reordered: Vec<NodeId> = Vec::with_capacity(order.total_nodes);
    for &id in order.by_priority.iter() {
        let idx = id.0;
        if must_keep.get(idx).copied().unwrap_or(false) && !seen[idx] {
            reordered.push(id);
            seen[idx] = true;
        }
    }

    for &id in order.by_priority.iter() {
        let idx = id.0;
        if !seen[idx] {
            reordered.push(id);
            seen[idx] = true;
        }
    }
    order.by_priority = reordered;
}

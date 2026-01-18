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
    pub strong_regex: Option<Regex>,
    pub weak_regex: Option<Regex>,
    pub show: GrepShow,
}

impl GrepConfig {
    pub fn has_strong(&self) -> bool {
        self.strong_regex.is_some()
    }

    pub fn has_weak(&self) -> bool {
        self.weak_regex.is_some()
    }

    /// Returns the regex to use for matching (strong takes precedence for must-keep).
    pub fn matching_regex(&self) -> Option<&Regex> {
        self.strong_regex.as_ref().or(self.weak_regex.as_ref())
    }
}

fn build_regex(pat: &str) -> Result<Regex> {
    Ok(RegexBuilder::new(pat).unicode(true).build()?)
}

pub fn build_grep_config(
    grep: Option<&str>,
    weak_grep: Option<&str>,
    grep_show: GrepShow,
    _case_insensitive: bool, // unused: case insensitivity embedded in patterns via (?i:...)
) -> Result<GrepConfig> {
    let strong_regex = grep.map(build_regex).transpose()?;
    let weak_regex = weak_grep.map(build_regex).transpose()?;
    Ok(GrepConfig {
        strong_regex,
        weak_regex,
        show: grep_show,
    })
}

pub(crate) struct GrepState {
    /// All matches (strong + weak) - used for priority reordering
    pub priority_boost: Vec<bool>,
    /// Strong matches only - used for must_keep enforcement
    pub must_keep: Vec<bool>,
    pub priority_boost_count: usize,
    pub must_keep_count: usize,
}

impl GrepState {
    pub fn is_enabled(&self) -> bool {
        self.priority_boost_count > 0
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
/// ancestor chain for priority boosting and/or guaranteed inclusion.
pub(crate) fn compute_grep_state(
    order: &PriorityOrder,
    grep: &GrepConfig,
) -> Option<GrepState> {
    let has_any = grep.strong_regex.is_some() || grep.weak_regex.is_some();
    if !has_any {
        return None;
    }

    let mut priority_boost = vec![false; order.total_nodes];
    let mut must_keep = vec![false; order.total_nodes];

    // Mark strong matches in both priority_boost and must_keep
    if let Some(re) = &grep.strong_regex {
        mark_matches_and_ancestors(order, re, &mut priority_boost);
        mark_matches_and_ancestors(order, re, &mut must_keep);
    }

    // Mark weak matches only in priority_boost (not must_keep)
    if let Some(re) = &grep.weak_regex {
        mark_matches_and_ancestors(order, re, &mut priority_boost);
    }

    let priority_boost_count = priority_boost.iter().filter(|b| **b).count();
    let must_keep_count = must_keep.iter().filter(|b| **b).count();

    (priority_boost_count > 0).then_some(GrepState {
        priority_boost,
        must_keep,
        priority_boost_count,
        must_keep_count,
    })
}

/// Reorder priority so boosted nodes are visited first, preserving the
/// existing relative order within each bucket.
pub(crate) fn reorder_priority_with_boost(
    order: &mut PriorityOrder,
    priority_boost: &[bool],
) {
    let mut seen = vec![false; order.total_nodes];
    let mut reordered: Vec<NodeId> = Vec::with_capacity(order.total_nodes);
    for &id in order.by_priority.iter() {
        let idx = id.0;
        if priority_boost.get(idx).copied().unwrap_or(false) && !seen[idx] {
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

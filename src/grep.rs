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
    /// Combined regex for highlighting (strong | weak)
    pub highlight_regex: Option<Regex>,
    pub show: GrepShow,
}

impl GrepConfig {
    pub fn has_strong(&self) -> bool {
        self.strong_regex.is_some()
    }
}

fn build_regex(pat: &str, case_insensitive: bool) -> Result<Regex> {
    Ok(RegexBuilder::new(pat)
        .unicode(true)
        .case_insensitive(case_insensitive)
        .build()?)
}

/// Combine multiple patterns into a single regex string.
/// Case-sensitive patterns are wrapped in `(?:...)`, case-insensitive in `(?i:...)`.
/// This prevents inline flags from leaking between patterns when joined with `|`.
/// Returns `None` if no patterns are provided.
pub fn combine_patterns(
    case_sensitive: &[impl AsRef<str>],
    case_insensitive: &[impl AsRef<str>],
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    for pat in case_sensitive {
        parts.push(format!("(?:{})", pat.as_ref()));
    }
    for pat in case_insensitive {
        parts.push(format!("(?i:{})", pat.as_ref()));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("|"))
    }
}

/// Build a GrepConfig from pattern slices.
/// Combines case-sensitive and case-insensitive patterns with OR semantics.
pub fn build_grep_config_from_patterns(
    strong: &[impl AsRef<str>],
    strong_icase: &[impl AsRef<str>],
    weak: &[impl AsRef<str>],
    weak_icase: &[impl AsRef<str>],
    grep_show: GrepShow,
) -> Result<GrepConfig> {
    let strong_combined = combine_patterns(strong, strong_icase);
    let weak_combined = combine_patterns(weak, weak_icase);
    build_grep_config(
        strong_combined.as_deref(),
        weak_combined.as_deref(),
        grep_show,
        false, // case-insensitivity already embedded via (?i:...)
    )
}

/// Build a GrepConfig from optional pattern strings.
/// For simple cases with single patterns. Use `build_grep_config_from_patterns`
/// for multiple patterns with mixed case-sensitivity.
pub fn build_grep_config(
    grep: Option<&str>,
    weak_grep: Option<&str>,
    grep_show: GrepShow,
    case_insensitive: bool,
) -> Result<GrepConfig> {
    let strong_regex =
        grep.map(|p| build_regex(p, case_insensitive)).transpose()?;
    let weak_regex = weak_grep
        .map(|p| build_regex(p, case_insensitive))
        .transpose()?;

    // Build highlight regex, reusing compiled regexes where possible
    let highlight_regex = match (&strong_regex, &weak_regex) {
        (Some(s), Some(w)) => {
            // Must build combined pattern when both exist
            let combined = format!("({})|({w})", s.as_str());
            Some(build_regex(&combined, case_insensitive)?)
        }
        (Some(s), None) => Some(s.clone()),
        (None, Some(w)) => Some(w.clone()),
        (None, None) => None,
    };

    Ok(GrepConfig {
        strong_regex,
        weak_regex,
        highlight_regex,
        show: grep_show,
    })
}

/// Grep matching state computed from a priority order.
/// - `all_matches`: nodes matching strong OR weak patterns (used for priority reordering)
/// - `strong_matches`: nodes matching only strong patterns (used for guaranteed inclusion)
pub(crate) struct GrepState {
    pub all_matches: Vec<bool>,
    pub strong_matches: Vec<bool>,
    pub strong_match_count: usize,
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
    flags: &mut [bool],
) {
    for (idx, node) in order.nodes.iter().enumerate() {
        if !matches_ranked(order, idx, node, re) {
            continue;
        }
        let mut cursor = Some(NodeId(idx));
        while let Some(node_id) = cursor {
            let raw = node_id.0;
            if flags[raw] {
                break;
            }
            flags[raw] = true;
            cursor = order.parent.get(raw).and_then(|p| *p);
        }
    }
}

/// Compute grep state by scanning the tree.
/// Optimized: strong matches are computed once, then weak matches are OR'd in.
pub(crate) fn compute_grep_state(
    order: &PriorityOrder,
    grep: &GrepConfig,
) -> Option<GrepState> {
    let has_any = grep.strong_regex.is_some() || grep.weak_regex.is_some();
    if !has_any {
        return None;
    }

    let mut strong_matches = vec![false; order.total_nodes];

    // Compute strong matches once
    if let Some(re) = &grep.strong_regex {
        mark_matches_and_ancestors(order, re, &mut strong_matches);
    }

    // all_matches = strong_matches OR weak_matches
    let mut all_matches = strong_matches.clone();
    if let Some(re) = &grep.weak_regex {
        mark_matches_and_ancestors(order, re, &mut all_matches);
    }

    let all_match_count = all_matches.iter().filter(|b| **b).count();
    let strong_match_count = strong_matches.iter().filter(|b| **b).count();

    // Return Some only if there are any matches
    (all_match_count > 0).then_some(GrepState {
        all_matches,
        strong_matches,
        strong_match_count,
    })
}

/// Reorder priority so matched nodes are visited first, preserving the
/// existing relative order within each bucket.
pub(crate) fn reorder_priority_with_matches(
    order: &mut PriorityOrder,
    all_matches: &[bool],
) {
    let mut seen = vec![false; order.total_nodes];
    let mut reordered: Vec<NodeId> = Vec::with_capacity(order.total_nodes);
    for &id in order.by_priority.iter() {
        let idx = id.0;
        if all_matches.get(idx).copied().unwrap_or(false) && !seen[idx] {
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

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

/// Pattern configuration for grep, designed to make invalid states unrepresentable.
/// The highlight regex is always derivable from the pattern state.
#[derive(Default)]
pub enum GrepPatterns {
    /// No grep patterns configured
    #[default]
    None,
    /// Only strong (filtering) pattern
    StrongOnly(Regex),
    /// Only weak (highlighting) pattern
    WeakOnly(Regex),
    /// Both strong and weak patterns, with precomputed combined highlight regex
    Both {
        strong: Regex,
        weak: Regex,
        highlight: Regex,
    },
}

impl GrepPatterns {
    /// Returns the strong (filtering) regex if configured.
    pub fn strong(&self) -> Option<&Regex> {
        match self {
            Self::StrongOnly(r) | Self::Both { strong: r, .. } => Some(r),
            _ => None,
        }
    }

    /// Returns the weak (highlight-only) regex if configured.
    pub fn weak(&self) -> Option<&Regex> {
        match self {
            Self::WeakOnly(r) | Self::Both { weak: r, .. } => Some(r),
            _ => None,
        }
    }

    /// Returns the highlight regex (strong | weak combined).
    /// No cloning needed - returns a reference.
    pub fn highlight(&self) -> Option<&Regex> {
        match self {
            Self::None => None,
            Self::StrongOnly(r) | Self::WeakOnly(r) => Some(r),
            Self::Both { highlight, .. } => Some(highlight),
        }
    }

    /// Returns true if a strong (filtering) pattern is configured.
    pub fn has_strong(&self) -> bool {
        matches!(self, Self::StrongOnly(_) | Self::Both { .. })
    }

    /// Returns true if any pattern (strong or weak) is configured.
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Grep configuration threaded through the pipeline.
#[derive(Default)]
pub struct GrepConfig {
    pub patterns: GrepPatterns,
    pub show: GrepShow,
}

impl GrepConfig {
    pub fn has_strong(&self) -> bool {
        self.patterns.has_strong()
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
    let patterns = match (grep, weak_grep) {
        (Some(s), Some(w)) => {
            let strong = build_regex(s, case_insensitive)?;
            let weak = build_regex(w, case_insensitive)?;
            // Combine patterns without redundant parens - | has lowest precedence
            let combined = format!("{}|{}", strong.as_str(), weak.as_str());
            let highlight = build_regex(&combined, case_insensitive)?;
            GrepPatterns::Both {
                strong,
                weak,
                highlight,
            }
        }
        (Some(s), None) => {
            GrepPatterns::StrongOnly(build_regex(s, case_insensitive)?)
        }
        (None, Some(w)) => {
            GrepPatterns::WeakOnly(build_regex(w, case_insensitive)?)
        }
        (None, None) => GrepPatterns::None,
    };

    Ok(GrepConfig {
        patterns,
        show: grep_show,
    })
}

/// Grep matching state computed from a priority order.
/// - `matched_nodes`: nodes matching any grep pattern (strong OR weak), used for priority boosting
/// - `guaranteed_nodes`: nodes matching strong patterns only, must be included in output
/// - `guaranteed_count`: count of nodes in `guaranteed_nodes` (used for filtering decisions)
pub(crate) struct GrepState {
    pub matched_nodes: Vec<bool>,
    pub guaranteed_nodes: Vec<bool>,
    pub guaranteed_count: usize,
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
/// Returns `None` if no grep patterns are configured.
/// Returns `Some(GrepState)` if grep is active, even with zero matches.
/// This makes the semantics clear: `None` = grep disabled, `Some` = grep enabled.
pub(crate) fn compute_grep_state(
    order: &PriorityOrder,
    grep: &GrepConfig,
) -> Option<GrepState> {
    if !grep.patterns.is_active() {
        return None;
    }

    let mut guaranteed_nodes = vec![false; order.total_nodes];

    // Compute guaranteed (strong) matches once
    if let Some(re) = grep.patterns.strong() {
        mark_matches_and_ancestors(order, re, &mut guaranteed_nodes);
    }

    // matched_nodes = guaranteed_nodes OR weak matches
    let mut matched_nodes = guaranteed_nodes.clone();
    if let Some(re) = grep.patterns.weak() {
        mark_matches_and_ancestors(order, re, &mut matched_nodes);
    }

    let guaranteed_count = guaranteed_nodes.iter().filter(|b| **b).count();

    Some(GrepState {
        matched_nodes,
        guaranteed_nodes,
        guaranteed_count,
    })
}

/// Reorder priority so grep-matched nodes are visited first, preserving the
/// existing relative order within each bucket.
pub(crate) fn reorder_priority_for_grep(
    order: &mut PriorityOrder,
    matched_nodes: &[bool],
) {
    let mut seen = vec![false; order.total_nodes];
    let mut reordered: Vec<NodeId> = Vec::with_capacity(order.total_nodes);
    for &id in order.by_priority.iter() {
        let idx = id.0;
        if matched_nodes.get(idx).copied().unwrap_or(false) && !seen[idx] {
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

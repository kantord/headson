use crate::grep::{
    GrepShow, GrepState, compute_grep_state, reorder_priority_with_must_keep,
};
use crate::order::{NodeId, ObjectType};
use crate::utils::measure::OutputStats;
use crate::{GrepConfig, PriorityOrder, RenderConfig};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Budgets {
    pub byte_budget: Option<usize>,
    pub char_budget: Option<usize>,
    pub line_budget: Option<usize>,
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Top-level orchestrator; splitting would obscure the budget/search flow"
)]
pub fn find_largest_render_under_budgets(
    order_build: &mut PriorityOrder,
    config: &RenderConfig,
    grep: &GrepConfig,
    budgets: Budgets,
) -> String {
    let total = order_build.total_nodes;
    if total == 0 {
        return String::new();
    }
    let measure_cfg = measure_config(order_build, config);
    let mut grep_state = compute_grep_state(order_build, grep);
    if grep.show == GrepShow::Matching
        && grep.regex.is_some()
        && grep_state.is_none()
        && order_build
            .object_type
            .get(crate::order::ROOT_PQ_ID)
            .is_some_and(|t| *t == ObjectType::Fileset)
    {
        return String::new();
    }
    filter_fileset_without_matches(order_build, &mut grep_state, grep);
    reorder_if_strong_grep(order_build, &grep_state, grep);
    let effective_budgets = effective_budgets_with_grep(
        order_build,
        &measure_cfg,
        grep,
        budgets,
        &grep_state,
    );
    let min_k = min_k_for(&grep_state, grep);
    let must_keep_slice = must_keep_slice(&grep_state, grep);
    let (k, mut inclusion_flags, render_set_id) = select_best_k(
        order_build,
        &measure_cfg,
        effective_budgets,
        min_k,
        must_keep_slice,
    );

    crate::serialization::prepare_render_set_top_k_and_ancestors(
        order_build,
        k,
        &mut inclusion_flags,
        render_set_id,
    );
    if let Some(state) = &grep_state {
        if !grep.weak && state.is_enabled() {
            include_must_keep(
                order_build,
                &mut inclusion_flags,
                render_set_id,
                &state.must_keep,
            );
        }
    }

    if config.debug {
        crate::debug::emit_render_debug(
            order_build,
            &inclusion_flags,
            render_set_id,
            config,
            budgets,
            k,
        );
    }

    crate::serialization::render_from_render_set(
        order_build,
        &inclusion_flags,
        render_set_id,
        &crate::RenderConfig {
            grep_highlight: config
                .grep_highlight
                .clone()
                .or_else(|| grep.regex.clone()),
            ..config.clone()
        },
    )
}

fn is_strong_grep(grep: &GrepConfig, state: &Option<GrepState>) -> bool {
    state.as_ref().is_some_and(GrepState::is_enabled) && !grep.weak
}

fn reorder_if_strong_grep(
    order_build: &mut PriorityOrder,
    state: &Option<GrepState>,
    grep: &GrepConfig,
) {
    if is_strong_grep(grep, state) {
        if let Some(s) = state {
            reorder_priority_with_must_keep(order_build, &s.must_keep);
        }
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Fileset filtering logic is easier to follow inline"
)]
fn filter_fileset_without_matches(
    order_build: &mut PriorityOrder,
    state: &mut Option<GrepState>,
    grep: &GrepConfig,
) {
    let Some(s) = state.as_mut() else {
        return;
    };
    if !s.is_enabled() {
        return;
    }
    if matches!(grep.show, crate::grep::GrepShow::All) {
        return;
    }
    if order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_none_or(|t| *t != ObjectType::Fileset)
    {
        return;
    }
    let Some(fileset_children) =
        order_build.fileset_children.clone().or_else(|| {
            order_build.children.get(crate::order::ROOT_PQ_ID).cloned()
        })
    else {
        return;
    };
    if fileset_children.is_empty() {
        return;
    }

    let Some(slot_map) = compute_fileset_slot_map(order_build) else {
        return;
    };

    let mut keep_slots = vec![false; fileset_children.len()];
    for (idx, keep) in s.must_keep.iter().enumerate() {
        if !*keep {
            continue;
        }
        if let Some(slot) = slot_map.get(idx).copied().flatten() {
            if let Some(flag) = keep_slots.get_mut(slot) {
                *flag = true;
            }
        }
    }

    if !keep_slots.iter().any(|k| *k) {
        // Fallback: consider fileset children directly in case matches were only
        // recorded on the file root.
        for (slot, child) in fileset_children.iter().enumerate() {
            if s.must_keep.get(child.0).copied().unwrap_or(false) {
                if let Some(flag) = keep_slots.get_mut(slot) {
                    *flag = true;
                }
            }
        }
    }

    order_build.by_priority.retain(|node| {
        match slot_map.get(node.0).copied().flatten() {
            Some(slot) => keep_slots.get(slot).copied().unwrap_or(false),
            None => true,
        }
    });

    let mut filtered_children: Vec<NodeId> = Vec::new();
    for (slot, child) in fileset_children.iter().enumerate() {
        if keep_slots.get(slot).copied().unwrap_or(false) {
            filtered_children.push(*child);
        }
    }
    order_build.fileset_children = Some(filtered_children.clone());
    if let Some(metrics) =
        order_build.metrics.get_mut(crate::order::ROOT_PQ_ID)
    {
        metrics.object_len = Some(filtered_children.len());
    }

    for (idx, keep) in s.must_keep.iter_mut().enumerate() {
        if let Some(slot) = slot_map.get(idx).copied().flatten() {
            if !keep_slots.get(slot).copied().unwrap_or(false) {
                *keep = false;
            }
        }
    }
    s.must_keep_count = s.must_keep.iter().filter(|b| **b).count();
}

#[allow(
    clippy::cognitive_complexity,
    reason = "single DFS that is clearer in one routine than split helpers"
)]
fn compute_fileset_slot_map(
    order_build: &PriorityOrder,
) -> Option<Vec<Option<usize>>> {
    if order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_none_or(|t| *t != ObjectType::Fileset)
    {
        return None;
    }
    let children = order_build.fileset_children.as_deref().or_else(|| {
        order_build
            .children
            .get(crate::order::ROOT_PQ_ID)
            .map(|v| &**v)
    })?;
    if children.is_empty() {
        return None;
    }

    let mut slots: Vec<Option<usize>> = vec![None; order_build.total_nodes];
    for (slot, child) in children.iter().enumerate() {
        let mut stack = vec![child.0];
        while let Some(node_idx) = stack.pop() {
            if slots.get(node_idx).is_some_and(Option::is_some) {
                continue;
            }
            if let Some(slot_ref) = slots.get_mut(node_idx) {
                *slot_ref = Some(slot);
            }
            if let Some(kids) = order_build.children.get(node_idx) {
                stack.extend(kids.iter().map(|k| k.0));
            }
        }
    }
    Some(slots)
}

fn effective_budgets_with_grep(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    grep: &GrepConfig,
    budgets: Budgets,
    state: &Option<GrepState>,
) -> Budgets {
    if !is_strong_grep(grep, state) {
        return budgets;
    }
    let Some(s) = state else {
        return budgets;
    };
    let cost = measure_must_keep(
        order_build,
        measure_cfg,
        &s.must_keep,
        budgets.char_budget.is_some(),
    );
    add_budgets(budgets, cost)
}

fn min_k_for(state: &Option<GrepState>, grep: &GrepConfig) -> usize {
    if is_strong_grep(grep, state) {
        state
            .as_ref()
            .map(|s| s.must_keep_count.max(1))
            .unwrap_or(1)
    } else {
        1
    }
}

fn must_keep_slice<'a>(
    state: &'a Option<GrepState>,
    grep: &GrepConfig,
) -> Option<&'a [bool]> {
    state
        .as_ref()
        .filter(|_| !grep.weak)
        .and_then(|s| s.is_enabled().then_some(s.must_keep.as_slice()))
}

fn select_best_k(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    budgets: Budgets,
    min_k: usize,
    must_keep: Option<&[bool]>,
) -> (usize, Vec<u32>, u32) {
    let total = order_build.total_nodes;
    let lo = min_k.max(1);
    let available = order_build.by_priority.len().max(1);
    let hi = match budgets.byte_budget {
        Some(c) => total.min(c.max(1)),
        None => total,
    }
    .min(available);

    let mut inclusion_flags: Vec<u32> = vec![0; total];

    let mut render_set_id: u32 = 1;
    let mut best_k: Option<usize> = None;
    let measure_chars = budgets.char_budget.is_some();
    let _ = crate::pruner::search::binary_search_max(lo, hi, |mid| {
        let current_render_id = render_set_id;
        crate::serialization::prepare_render_set_top_k_and_ancestors(
            order_build,
            mid,
            &mut inclusion_flags,
            current_render_id,
        );
        if let Some(flags) = must_keep {
            include_must_keep(
                order_build,
                &mut inclusion_flags,
                current_render_id,
                flags,
            );
        }
        let s = crate::serialization::render_from_render_set(
            order_build,
            &inclusion_flags,
            current_render_id,
            measure_cfg,
        );
        let stats =
            crate::utils::measure::count_output_stats(&s, measure_chars);
        let fits_bytes = budgets.byte_budget.is_none_or(|c| stats.bytes <= c);
        let fits_chars = budgets.char_budget.is_none_or(|c| stats.chars <= c);
        let fits_lines =
            budgets.line_budget.is_none_or(|cap| stats.lines <= cap);
        render_set_id = render_set_id.wrapping_add(1).max(1);
        if fits_bytes && fits_chars && fits_lines {
            best_k = Some(mid);
            true
        } else {
            false
        }
    });
    let k = best_k.unwrap_or(lo);
    (k, inclusion_flags, render_set_id)
}

pub(crate) fn constrained_dimensions(
    budgets: Budgets,
    stats: &crate::utils::measure::OutputStats,
) -> Vec<&'static str> {
    let checks = [
        (budgets.byte_budget.map(|b| stats.bytes >= b), "bytes"),
        (budgets.char_budget.map(|c| stats.chars >= c), "chars"),
        (budgets.line_budget.map(|l| stats.lines >= l), "lines"),
    ];
    checks
        .iter()
        .filter_map(|(cond, name)| cond.unwrap_or(false).then_some(*name))
        .collect()
}

fn measure_config(
    order_build: &PriorityOrder,
    config: &RenderConfig,
) -> RenderConfig {
    let root_is_fileset = order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_some_and(|t| *t == crate::order::ObjectType::Fileset);
    let mut measure_cfg = config.clone();
    measure_cfg.color_enabled = false;
    if config.show_fileset_headers
        && root_is_fileset
        && !config.count_fileset_headers_in_budgets
    {
        // Budgets are for content; measure without fileset headers so
        // section titles/summary lines remain “free” during selection.
        measure_cfg.show_fileset_headers = false;
    }
    measure_cfg
}

fn measure_must_keep(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    must_keep: &[bool],
    measure_chars: bool,
) -> OutputStats {
    let mut inclusion_flags: Vec<u32> = vec![0; order_build.total_nodes];
    let render_set_id: u32 = 1;
    include_must_keep(
        order_build,
        &mut inclusion_flags,
        render_set_id,
        must_keep,
    );
    let rendered = crate::serialization::render_from_render_set(
        order_build,
        &inclusion_flags,
        render_set_id,
        measure_cfg,
    );
    crate::utils::measure::count_output_stats(&rendered, measure_chars)
}

fn add_budgets(budgets: Budgets, extra: OutputStats) -> Budgets {
    Budgets {
        byte_budget: budgets
            .byte_budget
            .map(|b| b.saturating_add(extra.bytes)),
        char_budget: budgets
            .char_budget
            .map(|c| c.saturating_add(extra.chars)),
        line_budget: budgets
            .line_budget
            .map(|l| l.saturating_add(extra.lines)),
    }
}

fn include_string_descendants(
    order: &PriorityOrder,
    id: usize,
    flags: &mut [u32],
    render_id: u32,
) {
    if let Some(children) = order.children.get(id) {
        for child in children {
            let idx = child.0;
            if flags[idx] != render_id {
                flags[idx] = render_id;
                include_string_descendants(order, idx, flags, render_id);
            }
        }
    }
}

fn include_must_keep(
    order_build: &PriorityOrder,
    inclusion_flags: &mut [u32],
    render_set_id: u32,
    must_keep: &[bool],
) {
    for (idx, keep) in must_keep.iter().enumerate() {
        if !*keep {
            continue;
        }
        crate::utils::graph::mark_node_and_ancestors(
            order_build,
            crate::NodeId(idx),
            inclusion_flags,
            render_set_id,
        );
        if matches!(
            order_build.nodes.get(idx),
            Some(crate::RankedNode::SplittableLeaf { .. })
        ) {
            include_string_descendants(
                order_build,
                idx,
                inclusion_flags,
                render_set_id,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    // No internal tests here; behavior is covered by integration tests.
}

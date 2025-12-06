use crate::grep::{
    GrepShow, GrepState, compute_grep_state, reorder_priority_with_must_keep,
};
use crate::order::{NodeId, ObjectType, ROOT_PQ_ID};
use crate::utils::measure::{OutputStats, count_output_stats};
use crate::{GrepConfig, PriorityOrder, RenderConfig};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Budgets {
    pub byte_budget: Option<usize>,
    pub char_budget: Option<usize>,
    pub line_budget: Option<usize>,
    pub per_slot_byte_budget: Option<usize>,
    pub per_slot_char_budget: Option<usize>,
    pub per_slot_line_budget: Option<usize>,
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
    let search_budgets = adjust_tree_budgets(budgets, &measure_cfg);
    let mut grep_state = compute_grep_state(order_build, grep);
    if !grep.weak
        && grep.show == GrepShow::Matching
        && grep.regex.is_some()
        && grep_state.is_none()
        && order_build
            .object_type
            .get(crate::order::ROOT_PQ_ID)
            .is_some_and(|t| *t == ObjectType::Fileset)
    {
        return String::new();
    }
    filter_fileset_without_matches(
        order_build,
        &mut grep_state,
        grep,
        config.fileset_tree,
    );
    reorder_if_grep(order_build, &grep_state);
    let effective_budgets = effective_budgets_with_grep(
        order_build,
        &measure_cfg,
        grep,
        search_budgets,
        &grep_state,
    );
    let min_k = min_k_for(&grep_state, grep);
    let must_keep_slice = must_keep_slice(&grep_state, grep);
    let (k, mut inclusion_flags, render_set_id, sinkhole_order) =
        select_best_k(
            order_build,
            &measure_cfg,
            effective_budgets,
            min_k,
            must_keep_slice,
        );
    inclusion_flags.fill(0);

    if let Some(order) = sinkhole_order.as_ref() {
        mark_sinkhole_top_k_and_ancestors(
            order_build,
            order,
            k,
            &mut inclusion_flags,
            render_set_id,
            !config.fileset_tree,
        );
    } else {
        crate::serialization::prepare_render_set_top_k_and_ancestors(
            order_build,
            k,
            &mut inclusion_flags,
            render_set_id,
        );
    }
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

fn reorder_if_grep(
    order_build: &mut PriorityOrder,
    state: &Option<GrepState>,
) {
    if let Some(s) = state {
        reorder_priority_with_must_keep(order_build, &s.must_keep);
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
    keep_fileset_children_for_tree: bool,
) {
    if grep.weak {
        return;
    }
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

    if !keep_fileset_children_for_tree {
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

#[allow(
    clippy::cognitive_complexity,
    reason = "Small parent walk; splitting would obscure the simple loop."
)]
fn propagate_slots_from_parents(
    slots: &mut [Option<usize>],
    order_build: &PriorityOrder,
) {
    for idx in 0..slots.len() {
        if slots[idx].is_some() {
            continue;
        }
        let mut parent_idx =
            order_build.parent.get(idx).and_then(|p| p.map(|n| n.0));
        while let Some(pid) = parent_idx {
            if let Some(slot) = slots.get(pid).copied().flatten() {
                if let Some(entry) = slots.get_mut(idx) {
                    *entry = Some(slot);
                }
                break;
            }
            parent_idx =
                order_build.parent.get(pid).and_then(|p| p.map(|n| n.0));
        }
    }
}

fn fileset_slot_names(order_build: &PriorityOrder) -> Option<Vec<String>> {
    let children = order_build
        .fileset_children
        .as_deref()
        .or_else(|| order_build.children.get(ROOT_PQ_ID).map(|v| &**v))?;
    if children.is_empty() {
        return None;
    }
    let mut names = Vec::with_capacity(children.len());
    for child in children {
        let name = order_build
            .nodes
            .get(child.0)
            .and_then(|n| n.key_in_object())
            .unwrap_or_default()
            .to_string();
        names.push(name);
    }
    Some(names)
}

fn node_budget_cost(
    order_build: &PriorityOrder,
    node_idx: usize,
    measure_chars: bool,
    newline_len: usize,
) -> OutputStats {
    match order_build.nodes.get(node_idx) {
        Some(crate::RankedNode::AtomicLeaf { token, .. }) => {
            let mut stats = count_output_stats(token.as_str(), measure_chars);
            // Atomic tokens render with no trailing newline; add a minimal
            // quote/punctuation buffer to avoid undercounting.
            stats.bytes = stats.bytes.saturating_add(2);
            if measure_chars {
                stats.chars = stats.chars.saturating_add(2);
            }
            stats.lines = stats.lines.max(1);
            stats
        }
        Some(crate::RankedNode::SplittableLeaf { value, .. }) => {
            let mut stats = count_output_stats(value.as_str(), measure_chars);
            // Quotes + newline overhead for display templates; strict JSON
            // will be close to this bound as well.
            stats.bytes = stats.bytes.saturating_add(2 + newline_len);
            if measure_chars {
                stats.chars = stats.chars.saturating_add(2 + newline_len);
            }
            stats.lines = stats.lines.max(1);
            stats
        }
        _ => {
            let mut stats = OutputStats {
                bytes: newline_len,
                chars: 0,
                lines: 0,
            };
            if measure_chars {
                stats.chars = newline_len;
            }
            stats
        }
    }
}

fn fits_per_slot(
    current: &OutputStats,
    delta: &OutputStats,
    budgets: &Budgets,
) -> bool {
    let bytes_cap = budgets.per_slot_byte_budget;
    let chars_cap = budgets.per_slot_char_budget;
    let lines_cap = budgets.per_slot_line_budget;

    let would_bytes = current.bytes.saturating_add(delta.bytes);
    if bytes_cap.is_some_and(|cap| would_bytes > cap) {
        return false;
    }
    let would_chars = current.chars.saturating_add(delta.chars);
    if chars_cap.is_some_and(|cap| would_chars > cap) {
        return false;
    }
    let would_lines = current.lines.saturating_add(delta.lines);
    if lines_cap.is_some_and(|cap| would_lines > cap) {
        return false;
    }
    true
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Single-pass fileset walk; inlining keeps the budget flow readable."
)]
#[allow(
    clippy::too_many_lines,
    reason = "Single-pass walk reads clearer in one function."
)]
fn sinkhole_priority_order(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    budgets: &Budgets,
    must_keep: Option<&[bool]>,
) -> Option<Vec<NodeId>> {
    if budgets.per_slot_byte_budget.is_none()
        && budgets.per_slot_char_budget.is_none()
        && budgets.per_slot_line_budget.is_none()
    {
        return None;
    }
    let mut slot_map = compute_fileset_slot_map(order_build)?;
    propagate_slots_from_parents(&mut slot_map, order_build);
    let slot_count = slot_map.iter().flatten().max().map(|s| *s + 1)?;
    let mut usage: Vec<OutputStats> = vec![
        OutputStats {
            bytes: 0,
            chars: 0,
            lines: 0
        };
        slot_count
    ];
    let mut filtered: Vec<NodeId> =
        Vec::with_capacity(order_build.by_priority.len());
    let charge_headers = measure_cfg.show_fileset_headers
        && !measure_cfg.newline.is_empty()
        && !measure_cfg.fileset_tree;
    let header_names = if charge_headers {
        fileset_slot_names(order_build)
    } else {
        None
    };
    let mut header_charged: Vec<bool> = vec![false; slot_count];
    let measure_chars = budgets.char_budget.is_some()
        || budgets.per_slot_char_budget.is_some();
    let newline_len = measure_cfg.newline.len();

    for node_id in order_build.by_priority.iter() {
        let nid = node_id.0;
        if matches!(
            order_build.nodes.get(nid),
            Some(crate::RankedNode::LeafPart { .. })
        ) {
            continue;
        }
        let slot = slot_map.get(nid).and_then(|s| *s);
        let mut delta =
            node_budget_cost(order_build, nid, measure_chars, newline_len);
        let is_must_keep =
            must_keep.and_then(|m| m.get(nid)).copied().unwrap_or(false);
        let mut header_stats: Option<OutputStats> = None;
        if charge_headers {
            if let Some(slot_idx) = slot {
                if !header_charged.get(slot_idx).copied().unwrap_or(false) {
                    if let Some(name) =
                        header_names.as_ref().and_then(|n| n.get(slot_idx))
                    {
                        let mut stats = count_output_stats(
                            &format!("==> {name} <=="),
                            measure_chars,
                        );
                        stats.lines = stats.lines.max(1);
                        stats.bytes = stats.bytes.saturating_add(newline_len);
                        if measure_chars {
                            stats.chars =
                                stats.chars.saturating_add(newline_len);
                        }
                        header_stats = Some(stats);
                    }
                }
            }
        }

        if let Some(slot_idx) = slot {
            if is_must_keep {
                if let Some(h) = header_stats {
                    usage[slot_idx].bytes =
                        usage[slot_idx].bytes.saturating_add(h.bytes);
                    usage[slot_idx].chars =
                        usage[slot_idx].chars.saturating_add(h.chars);
                    usage[slot_idx].lines =
                        usage[slot_idx].lines.saturating_add(h.lines);
                    if let Some(hc) = header_charged.get_mut(slot_idx) {
                        *hc = true;
                    }
                }
                filtered.push(*node_id);
                continue;
            }
            if let Some(h) = header_stats.take() {
                delta.bytes = delta.bytes.saturating_add(h.bytes);
                delta.chars = delta.chars.saturating_add(h.chars);
                delta.lines = delta.lines.saturating_add(h.lines);
            }
            if !fits_per_slot(&usage[slot_idx], &delta, budgets) {
                continue;
            }
            usage[slot_idx].bytes =
                usage[slot_idx].bytes.saturating_add(delta.bytes);
            usage[slot_idx].chars =
                usage[slot_idx].chars.saturating_add(delta.chars);
            usage[slot_idx].lines =
                usage[slot_idx].lines.saturating_add(delta.lines);
            if header_stats.is_some() {
                if let Some(hc) = header_charged.get_mut(slot_idx) {
                    *hc = true;
                }
            }
        }
        filtered.push(*node_id);
    }

    Some(filtered)
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
        budgets.char_budget.is_some()
            || budgets.per_slot_char_budget.is_some(),
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

#[allow(
    clippy::cognitive_complexity,
    reason = "Budget search is clearer as a single routine."
)]
fn select_best_k(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    budgets: Budgets,
    min_k: usize,
    must_keep: Option<&[bool]>,
) -> (usize, Vec<u32>, u32, Option<Vec<NodeId>>) {
    let total = order_build.total_nodes;
    let base_lo = if must_keep.is_some() { 1 } else { min_k.max(1) };
    let sinkhole_order =
        sinkhole_priority_order(order_build, measure_cfg, &budgets, must_keep);
    let selection_order_ref = sinkhole_order
        .as_deref()
        .unwrap_or(&order_build.by_priority);
    let available = if let Some(order) = sinkhole_order.as_ref() {
        order
            .iter()
            .filter(|nid| counts_toward_k(order_build, nid.0))
            .count()
            .max(1)
    } else {
        selection_order_ref.len().max(1)
    };
    let capped_lo = base_lo.min(available);
    let hi = match budgets.byte_budget {
        Some(c) => total.min(c.max(1)),
        None => total,
    }
    .min(available);
    let effective_lo = capped_lo;
    let effective_hi = hi.max(effective_lo);

    let mut inclusion_flags: Vec<u32> = vec![0; total];

    let mut render_set_id: u32 = 1;
    let mut best_k: Option<usize> = None;
    let measure_chars = budgets.char_budget.is_some()
        || budgets.per_slot_char_budget.is_some();
    let use_sinkhole = sinkhole_order.is_some();
    let budgets_for_search = if let Some(flags) = must_keep {
        let mk =
            measure_must_keep(order_build, measure_cfg, flags, measure_chars);
        subtract_must_keep_from_budgets(budgets, mk)
    } else {
        budgets
    };
    let apply_must_keep = must_keep.is_some();
    let effective_min_k = if apply_must_keep { effective_lo } else { 1 };
    let _ = crate::pruner::search::binary_search_max(
        effective_lo.max(effective_min_k),
        effective_hi,
        |mid| {
            let current_render_id = render_set_id;
            if use_sinkhole {
                mark_sinkhole_top_k_and_ancestors(
                    order_build,
                    selection_order_ref,
                    mid,
                    &mut inclusion_flags,
                    current_render_id,
                    true,
                );
            } else {
                crate::serialization::prepare_render_set_top_k_and_ancestors(
                    order_build,
                    mid,
                    &mut inclusion_flags,
                    current_render_id,
                );
            }
            if let Some(flags) = must_keep {
                if apply_must_keep {
                    include_must_keep(
                        order_build,
                        &mut inclusion_flags,
                        current_render_id,
                        flags,
                    );
                }
            }
            let s = crate::serialization::render_from_render_set(
                order_build,
                &inclusion_flags,
                current_render_id,
                measure_cfg,
            );
            let stats =
                crate::utils::measure::count_output_stats(&s, measure_chars);
            let fits_bytes = budgets_for_search
                .byte_budget
                .is_none_or(|c| stats.bytes <= c);
            let fits_chars = budgets_for_search
                .char_budget
                .is_none_or(|c| stats.chars <= c);
            let fits_lines = budgets_for_search
                .line_budget
                .is_none_or(|cap| stats.lines <= cap);
            render_set_id = render_set_id.wrapping_add(1).max(1);
            if fits_bytes && fits_chars && fits_lines {
                best_k = Some(mid);
                true
            } else {
                false
            }
        },
    );
    let k = best_k.unwrap_or(effective_lo);
    (k, inclusion_flags, render_set_id, sinkhole_order)
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
    if config.fileset_tree {
        // In tree mode, show_fileset_headers controls whether scaffold lines
        // (pipes/gutters) render; respect the budget flag so scaffold can stay
        // “free” when headers are excluded from budgets.
        measure_cfg.show_fileset_headers =
            config.count_fileset_headers_in_budgets;
    } else if config.show_fileset_headers
        && root_is_fileset
        && !config.count_fileset_headers_in_budgets
    {
        // Budgets are for content; measure without fileset headers so
        // section titles/summary lines remain “free” during selection.
        measure_cfg.show_fileset_headers = false;
    }
    measure_cfg
}

fn adjust_tree_budgets(budgets: Budgets, cfg: &RenderConfig) -> Budgets {
    if !cfg.fileset_tree || cfg.count_fileset_headers_in_budgets {
        return budgets;
    }
    let slack = cfg.indent_unit.len().saturating_mul(4)
        + cfg.newline.len().saturating_mul(4)
        + 8;
    Budgets {
        byte_budget: budgets.byte_budget.map(|b| b.saturating_add(slack)),
        char_budget: budgets.char_budget,
        line_budget: budgets.line_budget,
        per_slot_byte_budget: budgets.per_slot_byte_budget,
        per_slot_char_budget: budgets.per_slot_char_budget,
        per_slot_line_budget: budgets.per_slot_line_budget,
    }
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

fn subtract_must_keep_from_budgets(
    budgets: Budgets,
    must_keep: OutputStats,
) -> Budgets {
    Budgets {
        byte_budget: budgets
            .byte_budget
            .map(|b| b.saturating_sub(must_keep.bytes)),
        char_budget: budgets
            .char_budget
            .map(|c| c.saturating_sub(must_keep.chars)),
        line_budget: budgets
            .line_budget
            .map(|l| l.saturating_sub(must_keep.lines)),
        per_slot_byte_budget: budgets.per_slot_byte_budget,
        per_slot_char_budget: budgets.per_slot_char_budget,
        per_slot_line_budget: budgets.per_slot_line_budget,
    }
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
        // Per-slot budgets stay fixed; must-keep items can exceed the cap but
        // should not expand the allowance for unrelated nodes in that slot.
        per_slot_byte_budget: budgets.per_slot_byte_budget,
        per_slot_char_budget: budgets.per_slot_char_budget,
        per_slot_line_budget: budgets.per_slot_line_budget,
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

fn build_priority_index_from_order(
    order: &[NodeId],
    total_nodes: usize,
) -> Vec<usize> {
    let mut priority_index = vec![usize::MAX; total_nodes];
    for (idx, nid) in order.iter().enumerate() {
        if let Some(slot) = priority_index.get_mut(nid.0) {
            *slot = idx;
        }
    }
    priority_index
}

fn enforce_force_first_child_custom(
    order_build: &PriorityOrder,
    inclusion_flags: &mut [u32],
    render_id: u32,
    priority_order: &[NodeId],
) {
    let priority_index = build_priority_index_from_order(
        priority_order,
        order_build.total_nodes,
    );

    for (idx, force) in order_build.force_first_child.iter().enumerate() {
        if !force_child_parent_included(
            inclusion_flags,
            render_id,
            *force,
            idx,
        ) {
            continue;
        }
        let Some(best_child) =
            best_priority_child(order_build, idx, &priority_index)
        else {
            continue;
        };
        if inclusion_flags[best_child.0] == render_id {
            continue;
        }
        crate::utils::graph::mark_node_and_ancestors(
            order_build,
            best_child,
            inclusion_flags,
            render_id,
        );
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Top-level render-set marking; splitting would add indirection."
)]
fn mark_sinkhole_top_k_and_ancestors(
    order_build: &PriorityOrder,
    sinkhole_order: &[NodeId],
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
    apply_force_first_child: bool,
) {
    if inclusion_flags.len() < order_build.total_nodes {
        inclusion_flags.resize(order_build.total_nodes, 0);
    }
    if top_k == 0 {
        return;
    }
    let priority_index = build_priority_index_from_order(
        &order_build.by_priority,
        order_build.total_nodes,
    );
    let mut counted = 0;
    for &id in sinkhole_order.iter() {
        if counts_toward_k(order_build, id.0) {
            crate::utils::graph::mark_node_and_ancestors(
                order_build,
                id,
                inclusion_flags,
                render_id,
            );
            let parent_is_fileset_root = order_build
                .parent
                .get(id.0)
                .and_then(|p| *p)
                .is_some_and(|p| p.0 == ROOT_PQ_ID)
                && order_build
                    .object_type
                    .get(ROOT_PQ_ID)
                    .is_some_and(|t| *t == ObjectType::Fileset);
            if parent_is_fileset_root {
                let has_child_included = order_build
                    .children
                    .get(id.0)
                    .map(|kids| {
                        kids.iter().any(|kid| {
                            inclusion_flags
                                .get(kid.0)
                                .copied()
                                .unwrap_or_default()
                                == render_id
                        })
                    })
                    .unwrap_or(false);
                if !has_child_included {
                    if let Some(best_child) =
                        best_priority_child(order_build, id.0, &priority_index)
                    {
                        crate::utils::graph::mark_node_and_ancestors(
                            order_build,
                            best_child,
                            inclusion_flags,
                            render_id,
                        );
                    }
                }
            }
            if matches!(
                order_build.nodes.get(id.0),
                Some(crate::RankedNode::SplittableLeaf { .. })
            ) {
                include_string_descendants(
                    order_build,
                    id.0,
                    inclusion_flags,
                    render_id,
                );
            }
            counted += 1;
            if counted >= top_k {
                break;
            }
        }
    }
    if apply_force_first_child {
        enforce_force_first_child_custom(
            order_build,
            inclusion_flags,
            render_id,
            &order_build.by_priority,
        );
    }
}

fn counts_toward_k(order_build: &PriorityOrder, node_idx: usize) -> bool {
    let is_fileset_child = order_build
        .parent
        .get(node_idx)
        .and_then(|p| *p)
        .is_some_and(|p| p.0 == ROOT_PQ_ID)
        && order_build
            .object_type
            .get(ROOT_PQ_ID)
            .is_some_and(|t| *t == ObjectType::Fileset);
    match order_build.nodes.get(node_idx) {
        Some(crate::RankedNode::SplittableLeaf { .. })
        | Some(crate::RankedNode::AtomicLeaf { .. }) => true,
        _ if is_fileset_child => true,
        _ => order_build
            .children
            .get(node_idx)
            .map(Vec::is_empty)
            .unwrap_or(true),
    }
}

fn force_child_parent_included(
    inclusion_flags: &[u32],
    render_id: u32,
    force: bool,
    idx: usize,
) -> bool {
    let included =
        inclusion_flags.get(idx).copied().unwrap_or_default() == render_id;
    force && included
}

fn best_priority_child(
    order_build: &PriorityOrder,
    parent_idx: usize,
    priority_index: &[usize],
) -> Option<NodeId> {
    let children = order_build.children.get(parent_idx)?;
    children
        .iter()
        .min_by_key(|cid| {
            priority_index.get(cid.0).copied().unwrap_or(usize::MAX)
        })
        .copied()
}

#[cfg(test)]
mod tests {
    // No internal tests here; behavior is covered by integration tests.
}

use crate::grep::{
    GrepShow, GrepState, compute_grep_state, reorder_priority_with_must_keep,
};
use crate::order::{NodeId, ObjectType, ROOT_PQ_ID};
use crate::utils::measure::{OutputStats, count_output_stats};
use crate::{GrepConfig, PriorityOrder, RenderConfig};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BudgetKind {
    Bytes,
    Chars,
    Lines,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Budget {
    pub kind: BudgetKind,
    pub cap: usize,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Budgets {
    pub global: Option<Budget>,
    pub per_slot: Option<Budget>,
}

impl Budget {
    fn add_stats(self, stats: &OutputStats) -> Self {
        let delta = match self.kind {
            BudgetKind::Bytes => stats.bytes,
            BudgetKind::Chars => stats.chars,
            BudgetKind::Lines => stats.lines,
        };
        Self {
            cap: self.cap.saturating_add(delta),
            ..self
        }
    }

    fn subtract_stats(self, stats: &OutputStats) -> Self {
        let delta = match self.kind {
            BudgetKind::Bytes => stats.bytes,
            BudgetKind::Chars => stats.chars,
            BudgetKind::Lines => stats.lines,
        };
        Self {
            cap: self.cap.saturating_sub(delta),
            ..self
        }
    }

    fn exceeds(&self, stats: &OutputStats) -> bool {
        match self.kind {
            BudgetKind::Bytes => stats.bytes > self.cap,
            BudgetKind::Chars => stats.chars > self.cap,
            BudgetKind::Lines => stats.lines > self.cap,
        }
    }
}

impl Budgets {
    pub fn measure_chars(&self) -> bool {
        matches!(
            self.global,
            Some(Budget {
                kind: BudgetKind::Chars,
                ..
            })
        ) || matches!(
            self.per_slot,
            Some(Budget {
                kind: BudgetKind::Chars,
                ..
            })
        )
    }

    pub fn measure_lines(&self) -> bool {
        matches!(
            self.global,
            Some(Budget {
                kind: BudgetKind::Lines,
                ..
            })
        ) || matches!(
            self.per_slot,
            Some(Budget {
                kind: BudgetKind::Lines,
                ..
            })
        )
    }

    pub fn per_slot_active(&self) -> bool {
        self.per_slot.is_some()
    }

    pub fn global_active(&self) -> bool {
        self.global.is_some()
    }

    pub fn per_slot_kind(&self) -> Option<BudgetKind> {
        self.per_slot.map(|b| b.kind)
    }

    pub fn global_kind(&self) -> Option<BudgetKind> {
        self.global.map(|b| b.kind)
    }

    pub fn per_slot_cap_for(&self, kind: BudgetKind) -> Option<usize> {
        match self.per_slot {
            Some(b) if b.kind == kind => Some(b.cap),
            _ => None,
        }
    }

    pub fn global_cap_for(&self, kind: BudgetKind) -> Option<usize> {
        match self.global {
            Some(b) if b.kind == kind => Some(b.cap),
            _ => None,
        }
    }

    pub fn per_slot_zero_cap(&self) -> bool {
        matches!(self.per_slot, Some(b) if b.cap == 0)
    }
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
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
    let root_is_fileset = order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_some_and(|t| *t == ObjectType::Fileset);
    let measure_cfg = measure_config(order_build, config);
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
        budgets,
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
    if effective_budgets.per_slot_zero_cap() {
        return String::new();
    }
    if k == 0
        && must_keep_slice.is_none()
        && !effective_budgets.per_slot_active()
        && !root_is_fileset
    {
        return String::new();
    }
    inclusion_flags.fill(0);
    let per_slot_caps_active = effective_budgets.per_slot_active();

    if let Some(order) = sinkhole_order.as_ref() {
        mark_sinkhole_top_k_and_ancestors(
            order_build,
            order,
            k,
            &mut inclusion_flags,
            render_set_id,
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
    if per_slot_caps_active && budgets.global.is_none() {
        ensure_fileset_headers_for_empty_slots(
            order_build,
            render_set_id,
            &mut inclusion_flags,
            &effective_budgets,
            &measure_cfg,
            config.count_fileset_headers_in_budgets,
        );
    }

    if per_slot_caps_active
        && matches!(
            effective_budgets.per_slot,
            Some(Budget {
                kind: BudgetKind::Lines,
                cap: 0
            })
        )
    {
        if let Some(slot_map) = compute_fileset_slot_map(order_build) {
            let has_included_slot =
                inclusion_flags.iter().enumerate().any(|(idx, flag)| {
                    *flag == render_set_id
                        && slot_map
                            .get(idx)
                            .and_then(|s| *s)
                            .is_some_and(|_| true)
                });
            if !has_included_slot {
                return String::new();
            }
        }
        if !root_is_fileset {
            return String::new();
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
pub(crate) fn compute_fileset_slot_map(
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

fn slot_measure_config(measure_cfg: &RenderConfig) -> RenderConfig {
    if measure_cfg.fileset_tree && measure_cfg.count_fileset_headers_in_budgets
    {
        measure_cfg.clone()
    } else if measure_cfg.fileset_tree {
        let mut cfg = measure_cfg.clone();
        cfg.fileset_tree = false;
        cfg.show_fileset_headers = false;
        cfg
    } else {
        measure_cfg.clone()
    }
}

fn compute_slot_stats_by_render(
    order_build: &PriorityOrder,
    base_flags: &[u32],
    render_id: u32,
    measure_cfg: &RenderConfig,
    slot_map: &[Option<usize>],
    measure_chars: bool,
) -> Vec<OutputStats> {
    let mut scratch_flags: Vec<u32> = vec![0; base_flags.len()];
    let slot_count =
        slot_map.iter().flatten().max().map(|s| *s + 1).unwrap_or(0);
    let mut out: Vec<OutputStats> = Vec::with_capacity(slot_count);
    let slot_cfg = slot_measure_config(measure_cfg);
    for slot_idx in 0..slot_count {
        for (idx, flag) in base_flags.iter().enumerate() {
            let node_slot = slot_map.get(idx).and_then(|s| *s);
            if *flag != render_id {
                scratch_flags[idx] = 0;
                continue;
            }
            if node_slot.is_some_and(|s| s != slot_idx) {
                scratch_flags[idx] = 0;
            } else {
                scratch_flags[idx] = render_id;
            }
        }
        let rendered = crate::serialization::render_from_render_set(
            order_build,
            &scratch_flags,
            render_id,
            &slot_cfg,
        );
        out.push(count_output_stats(&rendered, measure_chars));
    }
    out
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
    let Some(cap) = budgets.per_slot else {
        return true;
    };
    let would = match cap.kind {
        BudgetKind::Bytes => current.bytes.saturating_add(delta.bytes),
        BudgetKind::Chars => current.chars.saturating_add(delta.chars),
        BudgetKind::Lines => current.lines.saturating_add(delta.lines),
    };
    would <= cap.cap
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
    let _ = budgets.per_slot?;
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
    let measure_chars = budgets.measure_chars();
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
        budgets.measure_chars(),
    );
    // Expand the budgets to cover must-keep matches; the search phase will subtract
    // this cost so caps apply only to non-matching content. Per-slot caps remain
    // as-is; later selection still forces must-keep matches even if they exceed a
    // per-file cap, matching the “strong grep makes matches free” contract.
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
    clippy::too_many_lines,
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
    let zero_global_cap =
        matches!(budgets.global, Some(Budget { cap: 0, .. }));
    let allow_zero =
        must_keep.is_some() || budgets.per_slot.is_some() || zero_global_cap;
    let base_lo = if allow_zero { 0 } else { min_k.max(1) };
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
    let hi = match budgets.global {
        Some(Budget { cap: 0, .. }) => 0,
        Some(Budget {
            kind: BudgetKind::Bytes,
            cap,
        }) => total.min(cap.max(1)),
        _ => total,
    }
    .min(available);
    let effective_lo = capped_lo;
    let effective_hi = hi.max(effective_lo);

    let mut inclusion_flags: Vec<u32> = vec![0; total];

    let mut render_set_id: u32 = 1;
    let mut best_k: Option<usize> = None;
    let measure_chars = budgets.measure_chars();
    let use_sinkhole = sinkhole_order.is_some();
    let per_slot_caps_active = budgets.per_slot.is_some();
    let slot_map = if per_slot_caps_active {
        compute_fileset_slot_map(order_build)
    } else {
        None
    };
    let slot_count = slot_map
        .as_ref()
        .and_then(|map| map.iter().flatten().max().map(|s| *s + 1));
    let (must_keep_stats, must_keep_slot_stats) =
        if let Some(flags) = must_keep {
            let (mk, mk_slots) = measure_must_keep_with_slots(
                order_build,
                measure_cfg,
                flags,
                measure_chars,
                slot_map.as_deref(),
            );
            (Some(mk), mk_slots)
        } else {
            (None, None)
        };
    let search_budgets_excluding_must_keep =
        if let Some(mk) = must_keep_stats.as_ref() {
            // Matches were already added to the effective budget upstream so they are “free”;
            // subtract them here so the search only constrains non-matching content.
            subtract_must_keep_from_budgets(budgets, *mk)
        } else {
            budgets
        };
    let apply_must_keep = must_keep.is_some();
    if apply_must_keep {
        if let Some(b) = search_budgets_excluding_must_keep.global {
            if b.cap == 0 {
                return (0, inclusion_flags, render_set_id, sinkhole_order);
            }
        }
    }
    let effective_min_k = if apply_must_keep { effective_lo } else { 0 };
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
            let mut recorder = slot_count.map(|n| {
                crate::serialization::output::SlotStatsRecorder::new(
                    n,
                    measure_chars,
                )
            });
            let (s, mut slot_stats) =
                crate::serialization::render_from_render_set_with_slots(
                    order_build,
                    &inclusion_flags,
                    current_render_id,
                    measure_cfg,
                    slot_map.as_deref(),
                    recorder.take(),
                );
            let mut stats =
                crate::utils::measure::count_output_stats(&s, measure_chars);
            if let Some(mk) = must_keep_stats.as_ref() {
                stats.bytes = stats.bytes.saturating_sub(mk.bytes);
                stats.chars = stats.chars.saturating_sub(mk.chars);
                stats.lines = stats.lines.saturating_sub(mk.lines);
            }
            if slot_stats
                .as_ref()
                .map(|slot_vec| {
                    slot_vec.iter().all(|stat| {
                        stat.bytes == 0 && stat.chars == 0 && stat.lines == 0
                    })
                })
                .unwrap_or(true)
            {
                if let Some(map) = slot_map.as_ref() {
                    slot_stats = Some(compute_slot_stats_by_render(
                        order_build,
                        &inclusion_flags,
                        current_render_id,
                        measure_cfg,
                        map,
                        measure_chars,
                    ));
                }
            }
            if per_slot_caps_active
                && measure_cfg.count_fileset_headers_in_budgets
                && slot_stats.is_some()
                && slot_map.is_some()
            {
                // Recompute per-slot stats using a slot-scoped render so counted headers
                // are charged even when the main recorder missed them (fileset headers are
                // assembled outside the normal Out/recorder path).
                if let Some(map) = slot_map.as_ref() {
                    slot_stats = Some(compute_slot_stats_by_render(
                        order_build,
                        &inclusion_flags,
                        current_render_id,
                        measure_cfg,
                        map,
                        measure_chars,
                    ));
                }
            }
            let fits_global = search_budgets_excluding_must_keep
                .global
                .map(|b| !b.exceeds(&stats))
                .unwrap_or(true);
            let fits_per_slot = if per_slot_caps_active {
                if let Some(cap) = budgets.per_slot {
                    if let Some(slot_stats_vec) = slot_stats {
                        slot_stats_vec.iter().enumerate().all(|(idx, st)| {
                            let mk_slot = must_keep_slot_stats
                                .as_ref()
                                .and_then(|mk| mk.get(idx));
                            let charged = match cap.kind {
                                BudgetKind::Bytes => st.bytes.saturating_sub(
                                    mk_slot.map(|m| m.bytes).unwrap_or(0),
                                ),
                                BudgetKind::Chars => st.chars.saturating_sub(
                                    mk_slot.map(|m| m.chars).unwrap_or(0),
                                ),
                                BudgetKind::Lines => st.lines.saturating_sub(
                                    mk_slot.map(|m| m.lines).unwrap_or(0),
                                ),
                            };
                            charged <= cap.cap
                        })
                    } else {
                        !cap.exceeds(&stats)
                    }
                } else {
                    true
                }
            } else {
                true
            };
            render_set_id = render_set_id.wrapping_add(1).max(1);
            if fits_global && fits_per_slot {
                best_k = Some(mid);
                true
            } else {
                false
            }
        },
    );
    let k = best_k.unwrap_or(0);
    (k, inclusion_flags, render_set_id, sinkhole_order)
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Tiny budget summary checks are clearer inline than split helpers."
)]
pub(crate) fn constrained_dimensions(
    budgets: Budgets,
    stats: &crate::utils::measure::OutputStats,
    slot_stats: Option<&[crate::utils::measure::OutputStats]>,
) -> Vec<&'static str> {
    let mut dims: Vec<&'static str> = Vec::new();
    if let Some(b) = budgets.global {
        if b.exceeds(stats) {
            dims.push(kind_str(b.kind, false));
        }
    }
    if let Some(b) = budgets.per_slot {
        if let Some(slot_vec) = slot_stats {
            if slot_vec.iter().any(|st| b.exceeds(st)) {
                dims.push(kind_str(b.kind, true));
            }
        } else if b.exceeds(stats) {
            // Fallback when per-slot details are unavailable: use aggregate stats.
            dims.push(kind_str(b.kind, true));
        }
    }
    dims
}

fn kind_str(kind: BudgetKind, per_slot: bool) -> &'static str {
    match (kind, per_slot) {
        (BudgetKind::Bytes, false) => "bytes",
        (BudgetKind::Chars, false) => "chars",
        (BudgetKind::Lines, false) => "lines",
        (BudgetKind::Bytes, true) => "per-file bytes",
        (BudgetKind::Chars, true) => "per-file chars",
        (BudgetKind::Lines, true) => "per-file lines",
    }
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

fn measure_must_keep(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    must_keep: &[bool],
    measure_chars: bool,
) -> OutputStats {
    measure_must_keep_with_slots(
        order_build,
        measure_cfg,
        must_keep,
        measure_chars,
        None,
    )
    .0
}

fn measure_must_keep_with_slots(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    must_keep: &[bool],
    measure_chars: bool,
    slot_map: Option<&[Option<usize>]>,
) -> (OutputStats, Option<Vec<OutputStats>>) {
    let mut inclusion_flags: Vec<u32> = vec![0; order_build.total_nodes];
    let render_set_id: u32 = 1;
    include_must_keep(
        order_build,
        &mut inclusion_flags,
        render_set_id,
        must_keep,
    );
    let mut recorder = slot_map.map(|slots| {
        let max_slot = slots.iter().flatten().max().copied().unwrap_or(0);
        crate::serialization::output::SlotStatsRecorder::new(
            max_slot.saturating_add(1),
            measure_chars,
        )
    });
    let (rendered, mut slot_stats) =
        crate::serialization::render_from_render_set_with_slots(
            order_build,
            &inclusion_flags,
            render_set_id,
            measure_cfg,
            slot_map,
            recorder.take(),
        );
    if slot_stats
        .as_ref()
        .map(|stats| {
            stats
                .iter()
                .all(|s| s.bytes == 0 && s.chars == 0 && s.lines == 0)
        })
        .unwrap_or(true)
    {
        if let Some(map) = slot_map.as_ref() {
            slot_stats = Some(compute_slot_stats_by_render(
                order_build,
                &inclusion_flags,
                render_set_id,
                measure_cfg,
                map,
                measure_chars,
            ));
        }
    }
    (
        crate::utils::measure::count_output_stats(&rendered, measure_chars),
        slot_stats,
    )
}

fn subtract_must_keep_from_budgets(
    budgets: Budgets,
    must_keep: OutputStats,
) -> Budgets {
    Budgets {
        global: budgets.global.map(|b| b.subtract_stats(&must_keep)),
        per_slot: budgets.per_slot,
    }
}

fn add_budgets(budgets: Budgets, extra: OutputStats) -> Budgets {
    Budgets {
        global: budgets.global.map(|b| b.add_stats(&extra)),
        // Per-slot budgets stay fixed; must-keep items can exceed the cap but
        // should not expand the allowance for unrelated nodes in that slot.
        per_slot: budgets.per_slot,
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

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Top-level render-set marking; splitting would add indirection."
)]
fn mark_sinkhole_top_k_and_ancestors(
    order_build: &PriorityOrder,
    sinkhole_order: &[NodeId],
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
) {
    if inclusion_flags.len() < order_build.total_nodes {
        inclusion_flags.resize(order_build.total_nodes, 0);
    }
    if top_k == 0 {
        return;
    }
    let mut counted = 0;
    for &id in sinkhole_order.iter() {
        if counts_toward_k(order_build, id.0) {
            crate::utils::graph::mark_node_and_ancestors(
                order_build,
                id,
                inclusion_flags,
                render_id,
            );
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
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Single walk over render flags; splitting would obscure the slot/header handling."
)]
fn ensure_fileset_headers_for_empty_slots(
    order_build: &PriorityOrder,
    render_id: u32,
    inclusion_flags: &mut Vec<u32>,
    budgets: &Budgets,
    measure_cfg: &RenderConfig,
    count_headers_in_budgets: bool,
) {
    let Some(slot_map) = compute_fileset_slot_map(order_build) else {
        return;
    };
    let slot_count =
        slot_map.iter().flatten().max().map(|s| *s + 1).unwrap_or(0);
    if slot_count == 0 {
        return;
    }
    let children = order_build
        .fileset_children
        .as_deref()
        .or_else(|| order_build.children.get(ROOT_PQ_ID).map(|v| &**v));
    let Some(fileset_children) = children else {
        return;
    };
    if inclusion_flags.len() < order_build.total_nodes {
        inclusion_flags.resize(order_build.total_nodes, 0);
    }
    let measure_chars = budgets.measure_chars();
    let header_names = fileset_slot_names(order_build);
    let newline_len = measure_cfg.newline.len();
    for slot_idx in 0..slot_count {
        let has_slot_node =
            inclusion_flags.iter().enumerate().any(|(idx, flag)| {
                *flag == render_id
                    && slot_map
                        .get(idx)
                        .and_then(|s| *s)
                        .is_some_and(|s| s == slot_idx)
            });
        if has_slot_node {
            continue;
        }
        if matches!(budgets.per_slot, Some(Budget { cap: 0, .. })) {
            continue;
        }
        let header_stats = header_stats_for_slot(
            slot_idx,
            &header_names,
            measure_chars,
            newline_len,
            budgets,
        );
        if count_headers_in_budgets && header_stats.is_none() {
            continue;
        }
        if let Some(file_node) = fileset_children.get(slot_idx) {
            crate::utils::graph::mark_node_and_ancestors(
                order_build,
                *file_node,
                inclusion_flags,
                render_id,
            );
        }
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Header measurement includes conditional branches for caps/kinds; splitting would obscure the budget logic."
)]
fn header_stats_for_slot(
    slot_idx: usize,
    header_names: &Option<Vec<String>>,
    measure_chars: bool,
    newline_len: usize,
    budgets: &Budgets,
) -> Option<OutputStats> {
    let header_stats = if let Some(name) =
        header_names.as_ref().and_then(|n| n.get(slot_idx))
    {
        let mut stats =
            count_output_stats(&format!("==> {name} <=="), measure_chars);
        stats.lines = stats.lines.max(1);
        stats.bytes = stats.bytes.saturating_add(newline_len);
        if measure_chars {
            stats.chars = stats.chars.saturating_add(newline_len);
        }
        stats
    } else {
        OutputStats {
            bytes: newline_len,
            chars: if measure_chars { newline_len } else { 0 },
            lines: 1,
        }
    };
    if let Some(cap) = budgets.per_slot {
        let exceeds = match cap.kind {
            BudgetKind::Bytes => header_stats.bytes > cap.cap,
            BudgetKind::Chars => header_stats.chars > cap.cap,
            BudgetKind::Lines => header_stats.lines > cap.cap,
        };
        if exceeds {
            return None;
        }
    }
    Some(header_stats)
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

#[cfg(test)]
mod tests {
    // No internal tests here; behavior is covered by integration tests.
}

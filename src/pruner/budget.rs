use crate::grep::{
    GrepShow, GrepState, compute_grep_state, reorder_priority_with_must_keep,
};
use crate::order::{NodeId, ObjectType, ROOT_PQ_ID};
use crate::utils::measure::{OutputStats, count_output_stats};
use crate::{GrepConfig, PriorityOrder, RenderConfig};
use std::collections::VecDeque;

mod select;
use select::{SelectionContext, select_best_k};

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

#[derive(Debug)]
struct SelectionOutcome {
    k: Option<usize>,
    inclusion_flags: Vec<u32>,
    render_set_id: u32,
    selection_order: Option<Vec<NodeId>>,
}

impl Budget {
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

fn is_fileset_root(order_build: &PriorityOrder) -> bool {
    order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_some_and(|t| *t == ObjectType::Fileset)
}

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
    let root_is_fileset = is_fileset_root(order_build);
    let mut grep_state = compute_grep_state(order_build, grep);
    if strong_fileset_grep_without_matches(grep, &grep_state, root_is_fileset)
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
    let fileset_slots = FilesetSlots::new(order_build);
    let header_budgeting = header_budgeting_policy(order_build, config);
    let measure_cfg = measure_config(order_build, config, header_budgeting);
    let min_k = min_k_for(&grep_state, grep);
    let must_keep_slice = must_keep_slice(&grep_state, grep);
    let selection_ctx = SelectionContext {
        order_build,
        measure_cfg: &measure_cfg,
        budgets,
        min_k,
        must_keep: must_keep_slice,
        grep,
        state: &grep_state,
        fileset_slots: fileset_slots.as_ref(),
    };
    let finalize_ctx = FinalizeContext {
        budgets,
        fileset_slots: fileset_slots.as_ref(),
        measure_cfg: &measure_cfg,
        grep,
        grep_state: &grep_state,
        must_keep: must_keep_slice,
    };
    finalize_render_from_selection(
        order_build,
        config,
        header_budgeting,
        select_best_k(&selection_ctx),
        root_is_fileset,
        &finalize_ctx,
    )
    .unwrap_or_default()
}

struct FinalizeContext<'a> {
    budgets: Budgets,
    fileset_slots: Option<&'a FilesetSlots>,
    measure_cfg: &'a RenderConfig,
    grep: &'a GrepConfig,
    grep_state: &'a Option<GrepState>,
    must_keep: Option<&'a [bool]>,
}

fn finalize_render_from_selection(
    order_build: &mut PriorityOrder,
    config: &RenderConfig,
    header_budgeting: HeadersBudgeting,
    selection: SelectionOutcome,
    root_is_fileset: bool,
    finalize_ctx: &FinalizeContext<'_>,
) -> Option<String> {
    let SelectionOutcome {
        k: k_opt,
        mut inclusion_flags,
        render_set_id,
        selection_order,
    } = selection;
    let found_k = k_opt.is_some();
    let k = k_opt.unwrap_or(0);
    if should_short_circuit_after_selection(
        &finalize_ctx.budgets,
        finalize_ctx.must_keep,
        root_is_fileset,
        found_k,
        k,
    ) {
        return None;
    }
    inclusion_flags.fill(0);
    let per_slot_caps_active = finalize_ctx.budgets.per_slot_active();

    apply_selection(
        order_build,
        selection_order.as_deref(),
        k,
        &mut inclusion_flags,
        render_set_id,
    );
    include_strong_grep_must_keep(
        order_build,
        finalize_ctx.grep,
        finalize_ctx.grep_state,
        &mut inclusion_flags,
        render_set_id,
    );
    if per_slot_caps_active && !config.count_fileset_headers_in_budgets {
        ensure_fileset_headers_for_empty_slots(
            order_build,
            render_set_id,
            &mut inclusion_flags,
            &finalize_ctx.budgets,
            finalize_ctx.measure_cfg,
            finalize_ctx.fileset_slots,
            header_budgeting,
        );
    }

    if should_short_circuit_zero_line_slots(
        &finalize_ctx.budgets,
        finalize_ctx.fileset_slots,
        &inclusion_flags,
        render_set_id,
        root_is_fileset,
    ) {
        return None;
    }

    if config.debug {
        crate::debug::emit_render_debug(
            order_build,
            &inclusion_flags,
            render_set_id,
            config,
            finalize_ctx.budgets,
            k,
        );
    }

    Some(crate::serialization::render_from_render_set(
        order_build,
        &inclusion_flags,
        render_set_id,
        &crate::RenderConfig {
            grep_highlight: config
                .grep_highlight
                .clone()
                .or_else(|| finalize_ctx.grep.regex.clone()),
            ..config.clone()
        },
    ))
}

fn strong_fileset_grep_without_matches(
    grep: &GrepConfig,
    state: &Option<GrepState>,
    root_is_fileset: bool,
) -> bool {
    !grep.weak
        && matches!(grep.show, GrepShow::Matching)
        && grep.regex.is_some()
        && state.is_none()
        && root_is_fileset
}

fn is_strong_grep(grep: &GrepConfig, state: &Option<GrepState>) -> bool {
    state.as_ref().is_some_and(GrepState::is_enabled) && !grep.weak
}

fn apply_selection(
    order_build: &PriorityOrder,
    selection_order: Option<&[NodeId]>,
    k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_set_id: u32,
) {
    if let Some(order) = selection_order {
        mark_custom_top_k_and_ancestors(
            order_build,
            order,
            k,
            inclusion_flags,
            render_set_id,
        );
    } else {
        crate::serialization::prepare_render_set_top_k_and_ancestors(
            order_build,
            k,
            inclusion_flags,
            render_set_id,
        );
    }
}

fn include_strong_grep_must_keep(
    order_build: &PriorityOrder,
    grep: &GrepConfig,
    grep_state: &Option<GrepState>,
    inclusion_flags: &mut [u32],
    render_set_id: u32,
) {
    if !is_strong_grep(grep, grep_state) {
        return;
    }
    if let Some(state) = grep_state {
        include_must_keep(
            order_build,
            inclusion_flags,
            render_set_id,
            &state.must_keep,
        );
    }
}

fn should_short_circuit_after_selection(
    budgets: &Budgets,
    must_keep_slice: Option<&[bool]>,
    root_is_fileset: bool,
    found_k: bool,
    k: usize,
) -> bool {
    if budgets.per_slot_zero_cap() {
        return true;
    }
    if k == 0
        && must_keep_slice.is_none()
        && !budgets.per_slot_active()
        && !root_is_fileset
    {
        return true;
    }
    if !found_k && must_keep_slice.is_none() && !root_is_fileset {
        return true;
    }
    false
}

fn should_short_circuit_zero_line_slots(
    budgets: &Budgets,
    fileset_slots: Option<&FilesetSlots>,
    inclusion_flags: &[u32],
    render_set_id: u32,
    root_is_fileset: bool,
) -> bool {
    let Some(Budget {
        kind: BudgetKind::Lines,
        cap: 0,
    }) = budgets.per_slot
    else {
        return false;
    };
    if let Some(slots) = fileset_slots {
        let has_included_slot =
            inclusion_flags.iter().enumerate().any(|(idx, flag)| {
                *flag == render_set_id
                    && slots.map.get(idx).and_then(|s| *s).is_some()
            });
        if !has_included_slot {
            return true;
        }
    }
    !root_is_fileset
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

#[derive(Clone, Debug)]
pub(crate) struct FilesetSlots {
    pub map: Vec<Option<usize>>,
    pub count: usize,
    pub names: Option<Vec<String>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HeadersBudgeting {
    Free,
    Charged,
}

impl HeadersBudgeting {
    pub fn is_charged(self) -> bool {
        matches!(self, HeadersBudgeting::Charged)
    }
}

fn header_budgeting_policy(
    order_build: &PriorityOrder,
    config: &RenderConfig,
) -> HeadersBudgeting {
    if !is_fileset_root(order_build) || !config.show_fileset_headers {
        return HeadersBudgeting::Free;
    }
    if config.count_fileset_headers_in_budgets {
        HeadersBudgeting::Charged
    } else {
        HeadersBudgeting::Free
    }
}

impl FilesetSlots {
    pub(crate) fn new(order_build: &PriorityOrder) -> Option<Self> {
        let map = compute_fileset_slot_map(order_build)?;
        let count = map.iter().flatten().max().map(|s| *s + 1)?;
        let names = fileset_slot_names(order_build);
        Some(Self { map, count, names })
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

fn round_robin_slot_priority(
    order_build: &PriorityOrder,
    slots: &FilesetSlots,
) -> Option<Vec<NodeId>> {
    let slot_count = slots.count;
    if slot_count == 0 {
        return None;
    }
    let (mut buckets, unslotted) =
        bucket_nodes_by_slot(order_build, &slots.map, slot_count);
    let mut out = drain_round_robin(&mut buckets);
    out.extend(unslotted);
    Some(out)
}

fn bucket_nodes_by_slot(
    order_build: &PriorityOrder,
    slot_map: &[Option<usize>],
    slot_count: usize,
) -> (Vec<VecDeque<NodeId>>, Vec<NodeId>) {
    let mut buckets: Vec<VecDeque<NodeId>> = vec![VecDeque::new(); slot_count];
    let mut unslotted: Vec<NodeId> = Vec::new();
    for node in order_build.by_priority.iter().copied() {
        if let Some(slot) = slot_map.get(node.0).and_then(|s| *s) {
            if let Some(bucket) = buckets.get_mut(slot) {
                bucket.push_back(node);
                continue;
            }
        }
        unslotted.push(node);
    }
    (buckets, unslotted)
}

fn drain_round_robin(buckets: &mut [VecDeque<NodeId>]) -> Vec<NodeId> {
    let slot_count = buckets.len();
    let total: usize = buckets.iter().map(VecDeque::len).sum();
    let mut out: Vec<NodeId> = Vec::with_capacity(total);
    let mut remaining = total;
    let mut cursor = 0usize;
    while remaining > 0 {
        let slot = cursor % slot_count;
        if let Some(node) = buckets.get_mut(slot).and_then(VecDeque::pop_front)
        {
            out.push(node);
            remaining = remaining.saturating_sub(1);
        }
        cursor = cursor.saturating_add(1);
    }
    out
}

fn fits_per_slot_cap(
    cap: Option<Budget>,
    fallback_stats: &OutputStats,
    slot_stats: Option<&[OutputStats]>,
    must_keep_slot_stats: Option<&[OutputStats]>,
) -> bool {
    let Some(cap) = cap else { return true };
    let Some(slot_stats) = slot_stats else {
        return !cap.exceeds(fallback_stats);
    };
    slot_stats.iter().enumerate().all(|(idx, st)| {
        let mk_slot = must_keep_slot_stats.as_ref().and_then(|mk| mk.get(idx));
        let charged = match cap.kind {
            BudgetKind::Bytes => st
                .bytes
                .saturating_sub(mk_slot.map(|m| m.bytes).unwrap_or(0)),
            BudgetKind::Chars => st
                .chars
                .saturating_sub(mk_slot.map(|m| m.chars).unwrap_or(0)),
            BudgetKind::Lines => {
                let match_lines = mk_slot.map(|m| m.lines).unwrap_or(0);
                let mut lines = st.lines.saturating_sub(match_lines);
                if match_lines > cap.cap && lines > 0 && match_lines < st.lines
                {
                    // Treat the omission line as free when matches already exceed the cap
                    // so at least one non-matching line can fit.
                    lines = lines.saturating_sub(1);
                }
                lines
            }
        };
        charged <= cap.cap
    })
}

fn effective_budgets_with_grep(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    grep: &GrepConfig,
    state: &Option<GrepState>,
    fileset_slots: Option<&FilesetSlots>,
    measure_chars: bool,
) -> Option<(OutputStats, Option<Vec<OutputStats>>)> {
    if !is_strong_grep(grep, state) {
        return None;
    }
    let Some(s) = state else {
        return None;
    };
    Some(measure_must_keep_with_slots(
        order_build,
        measure_cfg,
        &s.must_keep,
        measure_chars,
        fileset_slots,
    ))
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
    header_budgeting: HeadersBudgeting,
) -> RenderConfig {
    let root_is_fileset = order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_some_and(|t| *t == crate::order::ObjectType::Fileset);
    let mut measure_cfg = config.clone();
    measure_cfg.color_enabled = false;
    measure_cfg.count_fileset_headers_in_budgets =
        header_budgeting.is_charged();
    if config.fileset_tree {
        // In tree mode, show_fileset_headers controls whether scaffold lines
        // (pipes/gutters) render; honor the budgeting policy so scaffold can
        // stay “free” when headers are excluded from budgets.
        measure_cfg.show_fileset_headers = header_budgeting.is_charged();
    } else if config.show_fileset_headers
        && root_is_fileset
        && header_budgeting == HeadersBudgeting::Free
    {
        // Budgets are for content; measure without fileset headers so
        // section titles/summary lines remain “free” during selection.
        measure_cfg.show_fileset_headers = false;
    }
    measure_cfg
}

fn measure_must_keep_with_slots(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    must_keep: &[bool],
    measure_chars: bool,
    fileset_slots: Option<&FilesetSlots>,
) -> (OutputStats, Option<Vec<OutputStats>>) {
    let mut measure_cfg = measure_cfg.clone();
    if matches!(
        measure_cfg.template,
        crate::OutputTemplate::Text | crate::OutputTemplate::Auto
    ) {
        // Strip omission markers when measuring must-keep slices so free matches
        // don’t undercount non-matching context.
        measure_cfg.style = crate::serialization::types::Style::Strict;
    }
    let mut inclusion_flags: Vec<u32> = vec![0; order_build.total_nodes];
    let render_set_id: u32 = 1;
    include_must_keep(
        order_build,
        &mut inclusion_flags,
        render_set_id,
        must_keep,
    );
    let mut recorder = fileset_slots.map(|slots| {
        crate::serialization::output::SlotStatsRecorder::new(
            slots.count,
            measure_chars,
        )
    });
    let (rendered, slot_stats) =
        crate::serialization::render_from_render_set_with_slots(
            order_build,
            &inclusion_flags,
            render_set_id,
            &measure_cfg,
            fileset_slots.map(|slots| slots.map.as_slice()),
            recorder.take(),
        );
    (
        crate::utils::measure::count_output_stats(&rendered, measure_chars),
        slot_stats,
    )
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

fn mark_custom_top_k_and_ancestors(
    order_build: &PriorityOrder,
    selection_order: &[NodeId],
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
) {
    if inclusion_flags.len() < order_build.total_nodes {
        inclusion_flags.resize(order_build.total_nodes, 0);
    }
    for node in selection_order.iter().take(top_k) {
        crate::utils::graph::mark_node_and_ancestors(
            order_build,
            *node,
            inclusion_flags,
            render_id,
        );
    }
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    reason = "Header insertion must juggle per-slot state, budgeting policy, and tree marking in one pass; splitting would hurt readability."
)]
fn ensure_fileset_headers_for_empty_slots(
    order_build: &PriorityOrder,
    render_id: u32,
    inclusion_flags: &mut Vec<u32>,
    budgets: &Budgets,
    measure_cfg: &RenderConfig,
    fileset_slots: Option<&FilesetSlots>,
    header_budgeting: HeadersBudgeting,
) {
    let Some(slots) = fileset_slots else {
        return;
    };
    if slots.count == 0 {
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
    let newline_len = measure_cfg.newline.len();
    let zero_per_slot =
        matches!(budgets.per_slot, Some(Budget { cap: 0, .. }));
    for slot_idx in 0..slots.count {
        let slot_has_nodes =
            inclusion_flags.iter().enumerate().any(|(idx, flag)| {
                *flag == render_id
                    && slots
                        .map
                        .get(idx)
                        .and_then(|s| *s)
                        .is_some_and(|s| s == slot_idx)
            });
        if slot_has_nodes || zero_per_slot {
            continue;
        }
        let header_stats = header_stats_for_slot(
            slot_idx,
            slots.names.as_ref(),
            measure_chars,
            newline_len,
            budgets,
        );
        if header_budgeting.is_charged() && header_stats.is_none() {
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
    reason = "Header measurement branches on name presence and budget kinds; keeping it in one routine makes the cap checks traceable."
)]
fn header_stats_for_slot(
    slot_idx: usize,
    header_names: Option<&Vec<String>>,
    measure_chars: bool,
    newline_len: usize,
    budgets: &Budgets,
) -> Option<OutputStats> {
    let stats = match header_names.and_then(|n| n.get(slot_idx)) {
        Some(name) => {
            let mut s =
                count_output_stats(&format!("==> {name} <=="), measure_chars);
            s.lines = s.lines.max(1);
            s.bytes = s.bytes.saturating_add(newline_len);
            if measure_chars {
                s.chars = s.chars.saturating_add(newline_len);
            }
            s
        }
        None => OutputStats {
            bytes: newline_len,
            chars: if measure_chars { newline_len } else { 0 },
            lines: 1,
        },
    };
    if let Some(cap) = budgets.per_slot {
        let value = match cap.kind {
            BudgetKind::Bytes => stats.bytes,
            BudgetKind::Chars => stats.chars,
            BudgetKind::Lines => stats.lines,
        };
        if value > cap.cap {
            return None;
        }
    }
    Some(stats)
}

#[cfg(test)]
mod tests {
    // No internal tests here; behavior is covered by integration tests.
}

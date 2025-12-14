use super::{
    Budget, BudgetKind, Budgets, FilesetSlots, GrepState, fits_per_slot_cap,
    mark_custom_top_k_and_ancestors, measure_must_keep_with_slots,
};
use crate::order::NodeId;
use crate::utils::measure::OutputStats;
use crate::{GrepConfig, PriorityOrder, RenderConfig};

pub(crate) struct SelectionContext<'a> {
    pub(crate) order_build: &'a PriorityOrder,
    pub(crate) measure_cfg: &'a RenderConfig,
    pub(crate) budgets: Budgets,
    pub(crate) min_k: usize,
    pub(crate) must_keep: Option<&'a [bool]>,
    pub(crate) grep: &'a GrepConfig,
    pub(crate) state: &'a Option<GrepState>,
    pub(crate) fileset_slots: Option<&'a FilesetSlots>,
}

struct SelectionPrep {
    selection_order: Option<Vec<NodeId>>,
    per_slot_caps_active: bool,
    slot_count: Option<usize>,
    effective_lo: usize,
    effective_hi: usize,
    measure_chars: bool,
}

struct MustKeepInfo {
    stats: Option<OutputStats>,
    slot_stats: Option<Vec<OutputStats>>,
    apply: bool,
}

struct SearchState {
    inclusion_flags: Vec<u32>,
    render_set_id: u32,
    best_k: Option<usize>,
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Bound derivation branches on optional budgets/slots; splitting would obscure the flow."
)]
fn prepare_selection(ctx: &SelectionContext<'_>) -> SelectionPrep {
    let per_slot_caps_active = ctx.budgets.per_slot.is_some();
    let slot_count = ctx.fileset_slots.map(|s| s.count);
    let selection_order = if per_slot_caps_active {
        ctx.fileset_slots.and_then(|slots| {
            super::round_robin_slot_priority(ctx.order_build, slots)
        })
    } else {
        None
    };
    let selection_order_ref: &[NodeId] = selection_order
        .as_deref()
        .unwrap_or(&ctx.order_build.by_priority);
    let available = selection_order_ref.len().max(1);
    let zero_global_cap =
        matches!(ctx.budgets.global, Some(Budget { cap: 0, .. }));
    let allow_zero =
        ctx.must_keep.is_some() || per_slot_caps_active || zero_global_cap;
    let mut base_lo = if allow_zero { 0 } else { ctx.min_k.max(1) };
    if per_slot_caps_active {
        base_lo = base_lo.max(slot_count.unwrap_or(0));
    }
    let capped_lo = base_lo.min(available);
    let hi = match ctx.budgets.global {
        Some(Budget { cap: 0, .. }) => 0,
        Some(Budget {
            kind: BudgetKind::Bytes,
            cap,
        }) => ctx.order_build.total_nodes.min(cap.max(1)),
        _ => ctx.order_build.total_nodes,
    }
    .min(available);
    let effective_lo = capped_lo;
    let effective_hi = hi.max(effective_lo);

    SelectionPrep {
        selection_order,
        per_slot_caps_active,
        slot_count,
        effective_lo,
        effective_hi,
        measure_chars: ctx.budgets.measure_chars(),
    }
}

fn compute_must_keep(
    ctx: &SelectionContext<'_>,
    prep: &SelectionPrep,
) -> MustKeepInfo {
    let apply = ctx.must_keep.is_some();
    if !apply {
        return MustKeepInfo {
            stats: None,
            slot_stats: None,
            apply,
        };
    }
    let free_allowance = super::effective_budgets_with_grep(
        ctx.order_build,
        ctx.measure_cfg,
        ctx.grep,
        ctx.state,
        ctx.fileset_slots,
        prep.measure_chars,
    );
    let Some(flags) = ctx.must_keep else {
        return MustKeepInfo {
            stats: None,
            slot_stats: None,
            apply: false,
        };
    };
    if let Some((mk, mk_slots)) = free_allowance {
        return MustKeepInfo {
            stats: Some(mk),
            slot_stats: if prep.per_slot_caps_active {
                mk_slots.or_else(|| Some(vec![mk]))
            } else {
                mk_slots
            },
            apply,
        };
    }
    let (mk, mk_slots) = measure_must_keep_with_slots(
        ctx.order_build,
        ctx.measure_cfg,
        flags,
        prep.measure_chars,
        ctx.fileset_slots,
    );
    MustKeepInfo {
        stats: Some(mk),
        slot_stats: if prep.per_slot_caps_active {
            mk_slots.or_else(|| Some(vec![mk]))
        } else {
            mk_slots
        },
        apply,
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Render measurement + budget checks are easiest to follow as a single pass."
)]
fn evaluate_mid(
    mid: usize,
    ctx: &SelectionContext<'_>,
    prep: &SelectionPrep,
    mk_info: &MustKeepInfo,
    selection_order_ref: &[NodeId],
    state: &mut SearchState,
) -> bool {
    let current_render_id = state.render_set_id;
    mark_custom_top_k_and_ancestors(
        ctx.order_build,
        selection_order_ref,
        mid,
        &mut state.inclusion_flags,
        current_render_id,
    );
    if mk_info.apply {
        if let Some(flags) = ctx.must_keep {
            super::include_must_keep(
                ctx.order_build,
                &mut state.inclusion_flags,
                current_render_id,
                flags,
            );
        }
    }
    let mut recorder = prep.slot_count.map(|n| {
        crate::serialization::output::SlotStatsRecorder::new(
            n,
            prep.measure_chars,
        )
    });
    let (s, mut slot_stats) =
        crate::serialization::render_from_render_set_with_slots(
            ctx.order_build,
            &state.inclusion_flags,
            current_render_id,
            ctx.measure_cfg,
            ctx.fileset_slots.map(|slots| slots.map.as_slice()),
            recorder.take(),
        );
    let render_stats =
        crate::utils::measure::count_output_stats(&s, prep.measure_chars);
    let mut adjusted_stats = render_stats;
    if let Some(mk) = mk_info.stats.as_ref() {
        adjusted_stats.bytes = adjusted_stats.bytes.saturating_sub(mk.bytes);
        adjusted_stats.chars = adjusted_stats.chars.saturating_sub(mk.chars);
        adjusted_stats.lines = adjusted_stats.lines.saturating_sub(mk.lines);
    }
    if prep.per_slot_caps_active && slot_stats.is_none() {
        slot_stats = Some(vec![render_stats]);
    }
    let fits_global = ctx
        .budgets
        .global
        .map(|b| !b.exceeds(&adjusted_stats))
        .unwrap_or(true);
    let fits_per_slot = if prep.per_slot_caps_active {
        fits_per_slot_cap(
            ctx.budgets.per_slot,
            &adjusted_stats,
            slot_stats.as_deref(),
            mk_info.slot_stats.as_deref(),
        )
    } else {
        true
    };
    state.render_set_id = state.render_set_id.wrapping_add(1).max(1);
    if fits_global && fits_per_slot {
        state.best_k = Some(mid);
        true
    } else {
        false
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Binary search over render sets remains branchy even after extraction."
)]
pub(crate) fn select_best_k(
    ctx: &SelectionContext<'_>,
) -> super::SelectionOutcome {
    let prep = prepare_selection(ctx);
    let mk_info = compute_must_keep(ctx, &prep);
    let mut search_state = SearchState {
        inclusion_flags: vec![0; ctx.order_build.total_nodes],
        render_set_id: 1,
        best_k: None,
    };

    if mk_info.apply {
        if let Some(b) = ctx.budgets.global {
            if b.cap == 0 {
                return super::SelectionOutcome {
                    k: Some(0),
                    inclusion_flags: search_state.inclusion_flags,
                    render_set_id: search_state.render_set_id,
                    selection_order: prep.selection_order,
                };
            }
        }
    }

    let effective_min_k = if mk_info.apply { prep.effective_lo } else { 0 };
    let selection_order_ref: &[NodeId] = prep
        .selection_order
        .as_deref()
        .unwrap_or(&ctx.order_build.by_priority);
    let _ = crate::pruner::search::binary_search_max(
        prep.effective_lo.max(effective_min_k),
        prep.effective_hi,
        |mid| {
            evaluate_mid(
                mid,
                ctx,
                &prep,
                &mk_info,
                selection_order_ref,
                &mut search_state,
            )
        },
    );
    super::SelectionOutcome {
        k: search_state.best_k,
        inclusion_flags: search_state.inclusion_flags,
        render_set_id: search_state.render_set_id,
        selection_order: prep.selection_order,
    }
}

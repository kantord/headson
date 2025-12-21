use crate::{Budget, BudgetKind, Budgets, OutputStats, binary_search_max};

pub trait PruningContext<Id: Copy> {
    fn total_nodes(&self) -> usize;
    fn priority_order(&self) -> &[Id];
    fn selection_order_for_slots(&self) -> Option<Vec<Id>>;
    fn slot_count(&self) -> Option<usize>;
    fn mark_top_k_and_ancestors(
        &self,
        order: &[Id],
        k: usize,
        flags: &mut [u32],
        render_id: u32,
    );
    fn include_must_keep(
        &self,
        flags: &mut [u32],
        render_id: u32,
        must_keep: &[bool],
    );
    fn measure(
        &self,
        flags: &[u32],
        render_id: u32,
        measure_chars: bool,
    ) -> (OutputStats, Option<Vec<OutputStats>>);
}

pub struct MustKeep<'a> {
    pub flags: &'a [bool],
    pub stats: MustKeepStats,
}

#[derive(Clone, Debug)]
pub struct MustKeepStats {
    pub total: OutputStats,
    pub per_slot: Option<Vec<OutputStats>>,
}

pub struct PruningConfig<'a, Id, C>
where
    Id: Copy,
    C: PruningContext<Id>,
{
    pub context: &'a C,
    pub budgets: Budgets,
    pub min_k: usize,
    pub must_keep: Option<MustKeep<'a>>,
    _marker: std::marker::PhantomData<Id>,
}

impl<'a, Id, C> PruningConfig<'a, Id, C>
where
    Id: Copy,
    C: PruningContext<Id>,
{
    pub fn new(
        context: &'a C,
        budgets: Budgets,
        min_k: usize,
        must_keep: Option<MustKeep<'a>>,
    ) -> Self {
        Self {
            context,
            budgets,
            min_k,
            must_keep,
            _marker: std::marker::PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct PruningResult<Id: Copy> {
    pub top_k: Option<usize>,
    pub inclusion_flags: Vec<u32>,
    pub render_set_id: u32,
    pub selection_order: Option<Vec<Id>>,
}

struct SelectionPrep<Id: Copy> {
    selection_order: Option<Vec<Id>>,
    per_slot_caps_active: bool,
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
fn prepare_selection<Id: Copy, C: PruningContext<Id>>(
    cfg: &PruningConfig<'_, Id, C>,
) -> SelectionPrep<Id> {
    let per_slot_caps_active = cfg.budgets.per_slot.is_some();
    let slot_count = cfg.context.slot_count();
    let selection_order = if per_slot_caps_active {
        cfg.context.selection_order_for_slots()
    } else {
        None
    };
    let selection_order_ref: &[Id] = selection_order
        .as_deref()
        .unwrap_or_else(|| cfg.context.priority_order());
    let available = selection_order_ref.len().max(1);
    let zero_global_cap =
        matches!(cfg.budgets.global, Some(Budget { cap: 0, .. }));
    let allow_zero =
        cfg.must_keep.is_some() || per_slot_caps_active || zero_global_cap;
    let mut base_lo = if allow_zero { 0 } else { cfg.min_k.max(1) };
    if per_slot_caps_active {
        base_lo = base_lo.max(slot_count.unwrap_or(0));
    }
    let capped_lo = base_lo.min(available);
    let hi = match cfg.budgets.global {
        Some(Budget { cap: 0, .. }) => 0,
        Some(Budget {
            kind: BudgetKind::Bytes,
            cap,
        }) => cfg.context.total_nodes().min(cap.max(1)),
        _ => cfg.context.total_nodes(),
    }
    .min(available);
    let effective_lo = capped_lo;
    let effective_hi = hi.max(effective_lo);

    SelectionPrep {
        selection_order,
        per_slot_caps_active,
        effective_lo,
        effective_hi,
        measure_chars: cfg.budgets.measure_chars(),
    }
}

fn compute_must_keep<Id: Copy, C: PruningContext<Id>>(
    cfg: &PruningConfig<'_, Id, C>,
    prep: &SelectionPrep<Id>,
) -> MustKeepInfo {
    let apply = cfg.must_keep.is_some();
    if !apply {
        return MustKeepInfo {
            stats: None,
            slot_stats: None,
            apply,
        };
    }
    let Some(mk) = cfg.must_keep.as_ref() else {
        return MustKeepInfo {
            stats: None,
            slot_stats: None,
            apply: false,
        };
    };
    let mut slot_stats = mk.stats.per_slot.clone();
    if prep.per_slot_caps_active && slot_stats.is_none() {
        slot_stats = Some(vec![mk.stats.total]);
    }
    MustKeepInfo {
        stats: Some(mk.stats.total),
        slot_stats,
        apply,
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Render measurement + budget checks are easiest to follow as a single pass."
)]
fn evaluate_mid<Id: Copy, C: PruningContext<Id>>(
    mid: usize,
    cfg: &PruningConfig<'_, Id, C>,
    prep: &SelectionPrep<Id>,
    mk_info: &MustKeepInfo,
    selection_order_ref: &[Id],
    state: &mut SearchState,
) -> bool {
    let current_render_id = state.render_set_id;
    cfg.context.mark_top_k_and_ancestors(
        selection_order_ref,
        mid,
        &mut state.inclusion_flags,
        current_render_id,
    );
    if mk_info.apply
        && let Some(mk) = cfg.must_keep.as_ref()
    {
        cfg.context.include_must_keep(
            &mut state.inclusion_flags,
            current_render_id,
            mk.flags,
        );
    }
    let (render_stats, mut slot_stats) = cfg.context.measure(
        &state.inclusion_flags,
        current_render_id,
        prep.measure_chars,
    );
    let mut adjusted_stats = render_stats;
    if let Some(mk) = mk_info.stats.as_ref() {
        adjusted_stats.bytes = adjusted_stats.bytes.saturating_sub(mk.bytes);
        adjusted_stats.chars = adjusted_stats.chars.saturating_sub(mk.chars);
        adjusted_stats.lines = adjusted_stats.lines.saturating_sub(mk.lines);
    }
    if prep.per_slot_caps_active && slot_stats.is_none() {
        slot_stats = Some(vec![render_stats]);
    }
    let fits_global = cfg
        .budgets
        .global
        .map(|b| !b.exceeds(&adjusted_stats))
        .unwrap_or(true);
    let fits_per_slot = if prep.per_slot_caps_active {
        fits_per_slot_cap(
            cfg.budgets.per_slot,
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
pub fn select_best_k<Id: Copy, C: PruningContext<Id>>(
    cfg: PruningConfig<'_, Id, C>,
) -> PruningResult<Id> {
    let prep = prepare_selection(&cfg);
    let mk_info = compute_must_keep(&cfg, &prep);
    let mut search_state = SearchState {
        inclusion_flags: vec![0; cfg.context.total_nodes()],
        render_set_id: 1,
        best_k: None,
    };

    if mk_info.apply
        && let Some(b) = cfg.budgets.global
        && b.cap == 0
    {
        return PruningResult {
            top_k: Some(0),
            inclusion_flags: search_state.inclusion_flags,
            render_set_id: search_state.render_set_id,
            selection_order: prep.selection_order,
        };
    }

    let effective_min_k = if mk_info.apply { prep.effective_lo } else { 0 };
    let selection_order_ref: &[Id] = prep
        .selection_order
        .as_deref()
        .unwrap_or_else(|| cfg.context.priority_order());
    let _ = binary_search_max(
        prep.effective_lo.max(effective_min_k),
        prep.effective_hi,
        |mid| {
            evaluate_mid(
                mid,
                &cfg,
                &prep,
                &mk_info,
                selection_order_ref,
                &mut search_state,
            )
        },
    );
    PruningResult {
        top_k: search_state.best_k,
        inclusion_flags: search_state.inclusion_flags,
        render_set_id: search_state.render_set_id,
        selection_order: prep.selection_order,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Clone)]
    struct FakeEngine {
        total: usize,
        order: Vec<usize>,
        slot_count: Option<usize>,
        slot_map: Option<Vec<usize>>,
    }

    impl FakeEngine {
        fn new(total: usize) -> Self {
            Self {
                total,
                order: (0..total).collect(),
                slot_count: None,
                slot_map: None,
            }
        }

        fn with_slots(
            mut self,
            slot_count: usize,
            slot_map: Vec<usize>,
        ) -> Self {
            self.slot_count = Some(slot_count);
            self.slot_map = Some(slot_map);
            self
        }
    }

    impl PruningContext<usize> for FakeEngine {
        fn total_nodes(&self) -> usize {
            self.total
        }

        fn priority_order(&self) -> &[usize] {
            &self.order
        }

        fn selection_order_for_slots(&self) -> Option<Vec<usize>> {
            let slot_count = self.slot_count?;
            let slot_map = self.slot_map.as_ref()?;
            let mut buckets: Vec<VecDeque<usize>> =
                vec![VecDeque::new(); slot_count];
            let mut unslotted = Vec::new();
            for &id in &self.order {
                if let Some(slot) = slot_map.get(id).copied()
                    && let Some(bucket) = buckets.get_mut(slot)
                {
                    bucket.push_back(id);
                    continue;
                }
                unslotted.push(id);
            }
            let mut out = Vec::with_capacity(self.order.len());
            let mut cursor = 0usize;
            let mut remaining: usize = buckets.iter().map(VecDeque::len).sum();
            while remaining > 0 {
                let slot = cursor % slot_count;
                if let Some(bucket) = buckets.get_mut(slot)
                    && let Some(node) = bucket.pop_front()
                {
                    out.push(node);
                    remaining = remaining.saturating_sub(1);
                }
                cursor = cursor.saturating_add(1);
            }
            out.extend(unslotted);
            Some(out)
        }

        fn slot_count(&self) -> Option<usize> {
            self.slot_count
        }

        fn mark_top_k_and_ancestors(
            &self,
            order: &[usize],
            k: usize,
            flags: &mut [u32],
            render_id: u32,
        ) {
            for idx in order.iter().take(k) {
                flags[*idx] = render_id;
            }
        }

        fn include_must_keep(
            &self,
            flags: &mut [u32],
            render_id: u32,
            must_keep: &[bool],
        ) {
            for (idx, keep) in must_keep.iter().enumerate() {
                if *keep {
                    flags[idx] = render_id;
                }
            }
        }

        fn measure(
            &self,
            flags: &[u32],
            render_id: u32,
            _measure_chars: bool,
        ) -> (OutputStats, Option<Vec<OutputStats>>) {
            let mut total = OutputStats {
                bytes: 0,
                chars: 0,
                lines: 0,
            };
            let mut per_slot = self.slot_count.map(|n| {
                vec![
                    OutputStats {
                        bytes: 0,
                        chars: 0,
                        lines: 0
                    };
                    n
                ]
            });
            for (idx, flag) in flags.iter().enumerate() {
                if *flag != render_id {
                    continue;
                }
                total.bytes += 1;
                total.chars += 1;
                total.lines += 1;
                if let Some(map) = self.slot_map.as_ref()
                    && let Some(stats) = per_slot.as_mut()
                {
                    let slot = map[idx];
                    if let Some(s) = stats.get_mut(slot) {
                        s.bytes += 1;
                        s.chars += 1;
                        s.lines += 1;
                    }
                }
            }
            (total, per_slot)
        }
    }

    #[test]
    fn picks_largest_k_under_global_cap() {
        let engine = FakeEngine::new(5);
        let cfg = PruningConfig::new(
            &engine,
            Budgets {
                global: Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: 3,
                }),
                per_slot: None,
            },
            1,
            None,
        );
        let out = select_best_k(cfg);
        assert_eq!(out.top_k, Some(3));
    }

    #[test]
    fn respects_per_slot_caps() {
        let engine = FakeEngine::new(4).with_slots(2, vec![0, 0, 1, 1]);
        let cfg = PruningConfig::new(
            &engine,
            Budgets {
                global: None,
                per_slot: Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: 1,
                }),
            },
            1,
            None,
        );
        let out = select_best_k(cfg);
        assert_eq!(out.top_k, Some(2));
    }

    #[test]
    fn must_keep_budget_applies_to_extra_nodes() {
        let engine = FakeEngine::new(4);
        let must_keep_flags = vec![true, true, false, false];
        let mk_stats = MustKeepStats {
            total: OutputStats {
                bytes: 2,
                chars: 2,
                lines: 2,
            },
            per_slot: None,
        };
        let cfg = PruningConfig::new(
            &engine,
            Budgets {
                global: Some(Budget {
                    kind: BudgetKind::Lines,
                    cap: 1,
                }),
                per_slot: None,
            },
            1,
            Some(MustKeep {
                flags: &must_keep_flags,
                stats: mk_stats,
            }),
        );
        let out = select_best_k(cfg);
        assert_eq!(out.top_k, Some(3));
    }
}

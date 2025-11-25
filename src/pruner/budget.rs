use crate::grep::{
    GrepState, compute_grep_state, reorder_priority_with_must_keep,
};
use crate::utils::measure::OutputStats;
use crate::{GrepConfig, PriorityOrder, RenderConfig};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Budgets {
    pub byte_budget: Option<usize>,
    pub char_budget: Option<usize>,
    pub line_budget: Option<usize>,
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
    let measure_cfg = measure_config(order_build, config);
    let grep_state = compute_grep_state(order_build, grep);
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
        config,
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
    let hi = match budgets.byte_budget {
        Some(c) => total.min(c.max(1)),
        None => total,
    };

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

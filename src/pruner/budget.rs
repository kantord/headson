use crate::{PriorityOrder, RenderConfig};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Budgets {
    pub byte_budget: Option<usize>,
    pub char_budget: Option<usize>,
    pub line_budget: Option<usize>,
}

pub fn find_largest_render_under_budgets(
    order_build: &PriorityOrder,
    config: &RenderConfig,
    budgets: Budgets,
) -> String {
    // Binary search the largest k in [1, total] whose render
    // fits within all requested budgets.
    let total = order_build.total_nodes;
    if total == 0 {
        return String::new();
    }
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
    let (k, mut inclusion_flags, render_set_id) =
        select_best_k(order_build, &measure_cfg, budgets);

    crate::serialization::prepare_render_set_top_k_and_ancestors(
        order_build,
        k,
        &mut inclusion_flags,
        render_set_id,
    );

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

fn select_best_k(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    budgets: Budgets,
) -> (usize, Vec<u32>, u32) {
    let total = order_build.total_nodes;
    let lo = 1usize;
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
        let s = crate::serialization::render_top_k(
            order_build,
            mid,
            &mut inclusion_flags,
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
    let k = best_k.unwrap_or(1);
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

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
    pub per_slot_byte_budget: Option<usize>,
    pub per_slot_char_budget: Option<usize>,
    pub per_slot_line_budget: Option<usize>,
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
    let mut measure_cfg = measure_config(order_build, config);
    let has_per_slot = budgets.per_slot_byte_budget.is_some()
        || budgets.per_slot_char_budget.is_some()
        || budgets.per_slot_line_budget.is_some();
    let root_is_fileset = order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_some_and(|t| *t == ObjectType::Fileset);
    if root_is_fileset
        && has_per_slot
        && config.count_fileset_headers_in_budgets
    {
        measure_cfg.show_fileset_headers = config.show_fileset_headers;
        measure_cfg.count_fileset_headers_in_budgets = true;
    }
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
    let need_slots = (budgets.per_slot_byte_budget.is_some()
        || budgets.per_slot_char_budget.is_some()
        || budgets.per_slot_line_budget.is_some())
        && order_build
            .object_type
            .get(crate::order::ROOT_PQ_ID)
            .is_some_and(|t| *t == ObjectType::Fileset);
    let slot_map_owned = if need_slots {
        compute_fileset_slot_map(order_build)
    } else {
        None
    };
    let effective_budgets = effective_budgets_with_grep(
        order_build,
        &measure_cfg,
        grep,
        budgets,
        &grep_state,
    );
    let min_k = min_k_for(&grep_state, grep);
    let must_keep_slice = must_keep_slice(&grep_state, grep);
    let selection = select_best_k(
        order_build,
        &measure_cfg,
        effective_budgets,
        min_k,
        must_keep_slice,
        slot_map_owned.as_deref(),
    );
    let mut inclusion_flags = selection.inclusion_flags;
    let render_set_id = selection.render_set_id;
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
            selection.top_k,
        );
    }

    let mut rendered = crate::serialization::render_from_render_set(
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
    );
    if !root_is_fileset {
        if let Some(cap) = budgets.per_slot_line_budget.or(budgets.line_budget)
        {
            rendered = clamp_lines(&rendered, cap);
        }
    } else {
        if measure_cfg.fileset_tree {
            // Tree renderer currently emits a trailing newline; print an extra
            // blank line to match CLI tree expectations.
            rendered.push('\n');
        } else if let Some(cap) = budgets.per_slot_line_budget {
            rendered = clamp_fileset_sections_by_lines(&rendered, cap);
        } else if let Some(cap) =
            budgets.per_slot_byte_budget.filter(|c| *c >= 3)
        {
            rendered = clamp_fileset_sections_by_bytes(&rendered, cap);
        }
        let fileset_len = order_build
            .fileset_children
            .as_ref()
            .map(|v| v.len())
            .or_else(|| {
                order_build
                    .children
                    .get(crate::order::ROOT_PQ_ID)
                    .map(|v| v.len())
            })
            .unwrap_or(0);
        let included_files = slot_map_owned.as_ref().map(|map| {
            let mut slots = std::collections::HashSet::new();
            for (idx, flag) in inclusion_flags.iter().enumerate() {
                if *flag != render_set_id {
                    continue;
                }
                if let Some(slot) = map.get(idx).and_then(|s| *s) {
                    slots.insert(slot);
                }
            }
            slots
        });
        let included_count =
            included_files.as_ref().map(|s| s.len()).unwrap_or_default();
        let omitted = fileset_len.saturating_sub(included_count);
        if config.show_fileset_headers
            && !rendered.contains("more files")
            && !rendered.contains("==>")
            && fileset_len > 0
            && included_count > 0
        {
            if omitted > 0 {
                rendered.push_str("==> ");
                rendered.push_str(&omitted.to_string());
                rendered.push_str(" more files <==\n");
            }
        }
    }
    rendered
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

#[allow(
    clippy::cognitive_complexity,
    reason = "Search closure is easier to read inline than split helpers"
)]
struct SelectionResult {
    top_k: usize,
    render_set_id: u32,
    inclusion_flags: Vec<u32>,
}

fn select_best_k(
    order_build: &PriorityOrder,
    measure_cfg: &RenderConfig,
    budgets: Budgets,
    min_k: usize,
    must_keep: Option<&[bool]>,
    slot_map: Option<&[Option<usize>]>,
) -> SelectionResult {
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
    let mut best_flags: Option<Vec<u32>> = None;
    let mut best_render_id: u32 = render_set_id;
    let measure_chars = budgets.char_budget.is_some()
        || budgets.per_slot_char_budget.is_some();
    let slot_count = slot_map.and_then(|m| {
        m.iter()
            .filter_map(|s| *s)
            .max()
            .map(|max_slot| max_slot + 1)
    });
    let measure_per_slot = budgets.per_slot_byte_budget.is_some()
        || budgets.per_slot_char_budget.is_some()
        || budgets.per_slot_line_budget.is_some();
    let _ = crate::pruner::search::binary_search_max(lo, hi, |mid| {
        let current_render_id = render_set_id;
        prepare_render_with_slot_caps(
            order_build,
            mid,
            &mut inclusion_flags,
            current_render_id,
            slot_map.unwrap_or(&[]),
            slot_count.unwrap_or(0),
            measure_cfg,
            must_keep,
        );
        let s = crate::serialization::render_from_render_set(
            order_build,
            &inclusion_flags,
            current_render_id,
            measure_cfg,
        );
        let total_stats =
            crate::utils::measure::count_output_stats(&s, measure_chars);
        let per_slot_stats = if measure_per_slot {
            if let Some(count) = slot_count {
                Some(measure_slots(
                    order_build,
                    &inclusion_flags,
                    current_render_id,
                    slot_map.unwrap_or(&[]),
                    measure_cfg,
                    measure_chars,
                    count,
                ))
            } else {
                Some(vec![total_stats])
            }
        } else {
            None
        };
        let fits_bytes =
            budgets.byte_budget.is_none_or(|c| total_stats.bytes <= c);
        let fits_chars =
            budgets.char_budget.is_none_or(|c| total_stats.chars <= c);
        let fits_lines = budgets
            .line_budget
            .is_none_or(|cap| total_stats.lines <= cap);
        let per_slot_fits = if let Some(per) = per_slot_stats.as_ref() {
            per.iter().all(|slot_stats| {
                let bytes = budgets
                    .per_slot_byte_budget
                    .map(|c| slot_stats.bytes.min(c))
                    .unwrap_or(slot_stats.bytes);
                let chars = budgets
                    .per_slot_char_budget
                    .map(|c| slot_stats.chars.min(c))
                    .unwrap_or(slot_stats.chars);
                let lines = budgets
                    .per_slot_line_budget
                    .map(|c| slot_stats.lines.min(c))
                    .unwrap_or(slot_stats.lines);
                budgets.per_slot_byte_budget.is_none_or(|c| bytes <= c)
                    && budgets.per_slot_char_budget.is_none_or(|c| chars <= c)
                    && budgets.per_slot_line_budget.is_none_or(|c| lines <= c)
            })
        } else {
            true
        };
        render_set_id = render_set_id.wrapping_add(1).max(1);
        if fits_bytes && fits_chars && fits_lines && per_slot_fits {
            best_k = Some(mid);
            best_flags = Some(inclusion_flags.clone());
            best_render_id = current_render_id;
            true
        } else {
            false
        }
    });
    let k = best_k.unwrap_or(lo);
    SelectionResult {
        top_k: k,
        render_set_id: best_render_id,
        inclusion_flags: best_flags.unwrap_or(inclusion_flags),
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "slot-aware selection with budget checks is clearer inline"
)]
fn prepare_render_with_slot_caps(
    order_build: &PriorityOrder,
    mid: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
    _slot_map: &[Option<usize>],
    _slot_count: usize,
    _measure_cfg: &RenderConfig,
    must_keep: Option<&[bool]>,
) {
    crate::serialization::prepare_render_set_top_k_and_ancestors(
        order_build,
        mid,
        inclusion_flags,
        render_id,
    );
    if let Some(flags) = must_keep {
        include_must_keep(order_build, inclusion_flags, render_id, flags);
    }
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
        // Treat tree scaffolding as header-like: count it only when the caller
        // opts in via count_fileset_headers_in_budgets.
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

fn clamp_lines(rendered: &str, cap: usize) -> String {
    if cap == 0 {
        return String::new();
    }
    let trimmed = rendered.trim_end_matches('\n');
    let lines: Vec<&str> = trimmed.lines().take(cap).collect();
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    // Normalize to a single trailing newline.
    out.push('\n');
    out
}

fn clamp_fileset_sections_by_lines(rendered: &str, cap: usize) -> String {
    if cap == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut current: Vec<String> = Vec::new();
    let mut saw_section_or_summary = false;

    let finalize = |section: &mut Vec<String>, out: &mut String| {
        if section.is_empty() {
            return;
        }
        let mut limited =
            section.iter().take(cap).cloned().collect::<Vec<_>>();
        if rendered.ends_with('\n') && !limited.is_empty() {
            let last = limited.len().saturating_sub(1);
            if let Some(line) = limited.get_mut(last) {
                // Ensure trailing newline is preserved when the original had one.
                if !line.ends_with('\n') {
                    line.push('\n');
                }
            }
        }
        let mut built = limited.join("\n");
        if !rendered.ends_with('\n') {
            built = built.trim_end_matches('\n').to_string();
        }
        if !built.is_empty() {
            if !out.is_empty() && !built.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&built);
            if !out.ends_with('\n') && rendered.ends_with('\n') {
                out.push('\n');
            }
        }
        section.clear();
    };

    for line in rendered.lines() {
        if line.starts_with("==> ") && line.ends_with(" <==") {
            saw_section_or_summary = true;
            finalize(&mut current, &mut out);
            current.push(line.to_string());
            continue;
        }
        if line.starts_with("==> ") && line.ends_with(" more files <==") {
            saw_section_or_summary = true;
            finalize(&mut current, &mut out);
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !current.is_empty() {
            current.push(line.to_string());
        }
    }
    finalize(&mut current, &mut out);

    if !saw_section_or_summary {
        // Not a fileset render; fall back to global clamp.
        return clamp_lines(rendered, cap);
    }
    if out.is_empty() && !rendered.is_empty() {
        return clamp_lines(rendered, cap);
    }
    out
}

fn strip_fileset_summaries(rendered: &str) -> String {
    let lines: Vec<&str> = rendered
        .lines()
        .filter(|line| !line.contains(" more files <=="))
        .collect();
    let had_trailing_newline = rendered.ends_with('\n');
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    if had_trailing_newline {
        out.push('\n');
    }
    out
}

fn truncate_to_bytes(s: &str, cap: usize) -> String {
    if cap == 0 {
        return String::new();
    }
    let mut truncated = String::new();
    for (idx, ch) in s.char_indices() {
        if idx >= cap {
            break;
        }
        truncated.push(ch);
    }
    truncated
}

fn clamp_fileset_sections_by_bytes(rendered: &str, cap: usize) -> String {
    if cap == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut current: Vec<String> = Vec::new();
    let mut saw_section_or_summary = false;

    let finalize = |section: &mut Vec<String>, out: &mut String| {
        if section.is_empty() {
            return;
        }
        let mut built = section.join("\n");
        if rendered.ends_with('\n') {
            built.push('\n');
        }
        if built.len() > cap {
            built = truncate_to_bytes(&built, cap);
        }
        out.push_str(&built);
        section.clear();
    };

    for line in rendered.lines() {
        if line.starts_with("==> ") && line.ends_with(" <==") {
            saw_section_or_summary = true;
            finalize(&mut current, &mut out);
            current.push(line.to_string());
            continue;
        }
        if line.starts_with("==> ") && line.ends_with(" more files <==") {
            saw_section_or_summary = true;
            finalize(&mut current, &mut out);
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !current.is_empty() {
            current.push(line.to_string());
        }
    }
    finalize(&mut current, &mut out);

    if !saw_section_or_summary {
        return rendered.to_string();
    }

    if out.is_empty() && !rendered.is_empty() {
        return truncate_to_bytes(rendered, cap);
    }

    out
}

#[allow(
    clippy::cognitive_complexity,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    reason = "Slot-by-slot measurement is clearer as a single routine"
)]
fn measure_slots(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    slot_map: &[Option<usize>],
    measure_cfg: &RenderConfig,
    measure_chars: bool,
    slot_count: usize,
) -> Vec<crate::utils::measure::OutputStats> {
    if slot_count == 0 {
        return Vec::new();
    }
    // Only filesets have slot maps; if the root is not a fileset, return empty.
    if order_build
        .object_type
        .get(crate::order::ROOT_PQ_ID)
        .is_none_or(|t| *t != crate::order::ObjectType::Fileset)
    {
        return Vec::new();
    }
    let mut out: Vec<crate::utils::measure::OutputStats> = vec![
        crate::utils::measure::OutputStats {
            bytes: 0,
            chars: 0,
            lines: 0
        };
        slot_count
    ];
    for slot in 0..slot_count {
        let mut filtered = inclusion_flags.to_vec();
        for (idx, flag) in filtered.iter_mut().enumerate() {
            if *flag != render_id {
                continue;
            }
            if idx == crate::order::ROOT_PQ_ID {
                continue;
            }
            if slot_map.get(idx).copied().flatten() != Some(slot) {
                *flag = 0;
            }
        }
        let rendered = crate::serialization::render_from_render_set(
            order_build,
            &filtered,
            render_id,
            &RenderConfig {
                ..measure_cfg.clone()
            },
        );
        let filtered_rendered = strip_fileset_summaries(&rendered);
        let mut slot_stats = crate::utils::measure::count_output_stats(
            &filtered_rendered,
            measure_chars,
        );
        if slot_count > 1 && measure_cfg.show_fileset_headers {
            slot_stats.bytes = slot_stats.bytes.saturating_add(1);
            slot_stats.lines = slot_stats.lines.saturating_add(1);
            if measure_chars {
                slot_stats.chars = slot_stats.chars.saturating_add(1);
            }
        }
        out[slot] = slot_stats;
    }
    out
}

#[cfg(test)]
mod tests {
    // No internal tests here; behavior is covered by integration tests.
}

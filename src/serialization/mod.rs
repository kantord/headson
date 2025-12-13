use crate::order::ObjectType;
use crate::order::{NodeKind, PriorityOrder, ROOT_PQ_ID};

pub mod color;
mod engine;
mod fileset;
mod highlight;
mod leaf;
pub mod output;
pub mod templates;
pub mod types;
mod util;

use crate::serialization::output::Out;
use engine::RenderEngine;
use util::{compute_max_index, digits};

/// Render using a previously prepared render set (inclusion flags matching `render_id`).
pub fn render_from_render_set(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    render_from_render_set_with_slots(
        order_build,
        inclusion_flags,
        render_id,
        config,
        None,
        None,
    )
    .0
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "Renderer needs the pre-pass/tree special-casing in one place to keep budget accounting clear."
)]
pub fn render_from_render_set_with_slots(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    config: &crate::RenderConfig,
    slot_map: Option<&[Option<usize>]>,
    recorder: Option<crate::serialization::output::SlotStatsRecorder>,
) -> (String, Option<Vec<crate::utils::measure::OutputStats>>) {
    render_from_render_set_with_slots_impl(
        order_build,
        inclusion_flags,
        render_id,
        config,
        slot_map,
        recorder,
        true,
    )
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Renderer + measurement pass need shared branching; splitting would obscure the budget logic."
)]
fn render_from_render_set_with_slots_impl(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    config: &crate::RenderConfig,
    slot_map: Option<&[Option<usize>]>,
    recorder: Option<crate::serialization::output::SlotStatsRecorder>,
    allow_separate_slot_render: bool,
) -> (String, Option<Vec<crate::utils::measure::OutputStats>>) {
    let needs_separate_slot_render = allow_separate_slot_render
        && recorder.is_some()
        && slot_map.is_some()
        && config.fileset_tree;
    if needs_separate_slot_render {
        let (rendered, _) = render_from_render_set_with_slots_impl(
            order_build,
            inclusion_flags,
            render_id,
            &crate::RenderConfig {
                debug: config.debug,
                grep_highlight: config.grep_highlight.clone(),
                ..config.clone()
            },
            slot_map,
            None,
            false,
        );
        let mut slot_measure_cfg = config.clone();
        if slot_measure_cfg.fileset_tree
            && !slot_measure_cfg.count_fileset_headers_in_budgets
        {
            slot_measure_cfg.fileset_tree = false;
            slot_measure_cfg.show_fileset_headers = false;
        }
        let (_, slot_stats) = render_from_render_set_with_slots_impl(
            order_build,
            inclusion_flags,
            render_id,
            &slot_measure_cfg,
            slot_map,
            recorder,
            false,
        );
        return (rendered, slot_stats);
    }
    let root_id = ROOT_PQ_ID;
    let root_is_fileset =
        order_build.object_type.get(root_id) == Some(&ObjectType::Fileset);
    let should_measure_line_numbers =
        matches!(config.template, crate::OutputTemplate::Code)
            || (matches!(config.template, crate::OutputTemplate::Auto)
                && root_is_fileset);
    let line_number_width = if should_measure_line_numbers {
        let max_index = compute_max_index(
            order_build,
            inclusion_flags,
            render_id,
            root_id,
        );
        Some(digits(max_index.saturating_add(1)))
    } else {
        None
    };
    let mut engine = RenderEngine::new(
        order_build,
        inclusion_flags,
        render_id,
        config,
        line_number_width,
        slot_map,
    );
    let mut s = String::new();
    let mut out =
        Out::new_with_recorder(&mut s, config, line_number_width, recorder);
    engine.write_node(root_id, 0, false, &mut out);
    let slot_stats = out.into_slot_stats();
    (s, slot_stats)
}

pub fn prepare_render_set_top_k_and_ancestors(
    order_build: &PriorityOrder,
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
) {
    if inclusion_flags.len() < order_build.total_nodes {
        inclusion_flags.resize(order_build.total_nodes, 0);
    }
    let k = top_k.min(order_build.total_nodes);
    crate::utils::graph::mark_top_k_and_ancestors(
        order_build,
        k,
        inclusion_flags,
        render_id,
    );
}

/// Convenience: prepare the render set for `top_k` nodes and render in one call.
#[allow(dead_code, reason = "Used by tests and pruner budget measurements")]
pub fn render_top_k(
    order_build: &PriorityOrder,
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    prepare_render_set_top_k_and_ancestors(
        order_build,
        top_k,
        inclusion_flags,
        render_id,
    );
    render_from_render_set(order_build, inclusion_flags, render_id, config)
}

#[cfg(test)]
mod tests;

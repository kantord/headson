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

struct RenderRun<'a> {
    order_build: &'a PriorityOrder,
    inclusion_flags: &'a [u32],
    render_id: u32,
    config: &'a crate::RenderConfig,
    slot_map: Option<&'a [Option<usize>]>,
    recorder: Option<crate::serialization::output::SlotStatsRecorder>,
}

impl<'a> RenderRun<'a> {
    fn new(
        order_build: &'a PriorityOrder,
        inclusion_flags: &'a [u32],
        render_id: u32,
        config: &'a crate::RenderConfig,
        slot_map: Option<&'a [Option<usize>]>,
        recorder: Option<crate::serialization::output::SlotStatsRecorder>,
    ) -> Self {
        Self {
            order_build,
            inclusion_flags,
            render_id,
            config,
            slot_map,
            recorder,
        }
    }

    fn root_is_fileset(&self) -> bool {
        self.order_build.object_type.get(ROOT_PQ_ID)
            == Some(&ObjectType::Fileset)
    }

    fn line_number_width(&self) -> Option<usize> {
        let should_measure_line_numbers =
            matches!(self.config.template, crate::OutputTemplate::Code)
                || (matches!(
                    self.config.template,
                    crate::OutputTemplate::Auto
                ) && self.root_is_fileset());
        if !should_measure_line_numbers {
            return None;
        }
        let max_index = compute_max_index(
            self.order_build,
            self.inclusion_flags,
            self.render_id,
            ROOT_PQ_ID,
        );
        Some(digits(max_index.saturating_add(1)))
    }

    fn guard_slot_stats(
        &self,
        slot_stats: Option<Vec<crate::utils::measure::OutputStats>>,
        recorded: bool,
    ) -> Option<Vec<crate::utils::measure::OutputStats>> {
        if recorded && self.slot_map.is_some() {
            slot_stats
        } else {
            None
        }
    }

    fn render(
        mut self,
    ) -> (String, Option<Vec<crate::utils::measure::OutputStats>>) {
        let line_number_width = self.line_number_width();
        let mut engine = RenderEngine::new(
            self.order_build,
            self.inclusion_flags,
            self.render_id,
            self.config,
            line_number_width,
            self.slot_map,
        );
        let mut s = String::new();
        let recorded = self.recorder.is_some();
        let mut out = Out::new_with_recorder(
            &mut s,
            self.config,
            line_number_width,
            self.recorder.take(),
        );
        engine.write_node(ROOT_PQ_ID, 0, false, &mut out);
        let slot_stats = out.into_slot_stats();
        (s, self.guard_slot_stats(slot_stats, recorded))
    }
}

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

pub fn render_from_render_set_with_slots(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    config: &crate::RenderConfig,
    slot_map: Option<&[Option<usize>]>,
    recorder: Option<crate::serialization::output::SlotStatsRecorder>,
) -> (String, Option<Vec<crate::utils::measure::OutputStats>>) {
    RenderRun::new(
        order_build,
        inclusion_flags,
        render_id,
        config,
        slot_map,
        recorder,
    )
    .render()
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

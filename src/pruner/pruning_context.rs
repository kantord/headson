use crate::order::NodeId;
use crate::utils::measure::{OutputStats, count_output_stats};
use crate::{PriorityOrder, RenderConfig};
use prunist::PruningContext;

use super::budget::{FilesetSlots, round_robin_slot_priority};

pub(crate) struct HeadsonPruningContext<'a> {
    pub(crate) order_build: &'a PriorityOrder,
    pub(crate) measure_cfg: &'a RenderConfig,
    pub(crate) fileset_slots: Option<&'a FilesetSlots>,
}

impl PruningContext<NodeId> for HeadsonPruningContext<'_> {
    fn total_nodes(&self) -> usize {
        self.order_build.total_nodes
    }

    fn priority_order(&self) -> &[NodeId] {
        &self.order_build.by_priority
    }

    fn selection_order_for_slots(&self) -> Option<Vec<NodeId>> {
        self.fileset_slots.and_then(|slots| {
            round_robin_slot_priority(self.order_build, slots)
        })
    }

    fn slot_count(&self) -> Option<usize> {
        self.fileset_slots.map(|s| s.count)
    }

    fn mark_top_k_and_ancestors(
        &self,
        order: &[NodeId],
        k: usize,
        flags: &mut [u32],
        render_id: u32,
    ) {
        super::budget::mark_custom_top_k_and_ancestors(
            self.order_build,
            order,
            k,
            flags,
            render_id,
        );
    }

    fn include_must_keep(
        &self,
        flags: &mut [u32],
        render_id: u32,
        must_keep: &[bool],
    ) {
        super::budget::include_must_keep(
            self.order_build,
            flags,
            render_id,
            must_keep,
        );
    }

    fn measure(
        &self,
        flags: &[u32],
        render_id: u32,
        measure_chars: bool,
    ) -> (OutputStats, Option<Vec<OutputStats>>) {
        let mut recorder = self.fileset_slots.map(|slots| {
            crate::serialization::output::SlotStatsRecorder::new(
                slots.count,
                measure_chars,
            )
        });
        let (rendered, slot_stats) =
            crate::serialization::render_from_render_set_with_slots(
                self.order_build,
                flags,
                render_id,
                self.measure_cfg,
                self.fileset_slots.map(|slots| slots.map.as_slice()),
                recorder.take(),
            );
        (count_output_stats(&rendered, measure_chars), slot_stats)
    }
}

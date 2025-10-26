#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "tests may use unwrap/expect for brevity"
    )
)]

use anyhow::Result;

mod json_ingest;
mod order;
mod serialization;
mod utils;
pub use order::{
    NodeId, NodeKind, PriorityConfig, PriorityOrder, RankedNode, build_order,
};
pub use order::types::ArrayBias;

pub use serialization::types::{OutputTemplate, RenderConfig};

pub fn headson(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    // Streaming arena parse from owned bytes + frontier adapter
    let arena = crate::json_ingest::build_json_tree_arena_from_bytes(
        input,
        priority_cfg,
    )?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budget(&order_build, config, budget);
    Ok(out)
}

pub fn headson_many(
    inputs: Vec<(String, Vec<u8>)>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    let arena = crate::json_ingest::build_json_tree_arena_from_many(
        inputs,
        priority_cfg,
    )?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budget(&order_build, config, budget);
    Ok(out)
}

fn find_largest_render_under_budget(
    order_build: &PriorityOrder,
    config: &RenderConfig,
    char_budget: usize,
) -> String {
    // Binary search the largest k in [1, total] whose render
    // fits within `char_budget`.
    let total = order_build.total_nodes;
    if total == 0 {
        return String::new();
    }
    // Each included node contributes at least some output; cap hi by budget.
    let lo = 1usize;
    let hi = total.min(char_budget.max(1));
    // Reuse render-inclusion flags across render attempts to avoid clearing the vector.
    // A node participates in the current render attempt when inclusion_flags[id] == render_set_id.
    let mut inclusion_flags: Vec<u32> = vec![0; total];
    // Each render attempt bumps this non-zero identifier to create a fresh inclusion set.
    let mut render_set_id: u32 = 1;
    let mut best_str: Option<String> = None;

    let _ = crate::utils::search::binary_search_max(lo, hi, |mid| {
        let s = crate::serialization::render_top_k(
            order_build,
            mid,
            &mut inclusion_flags,
            render_set_id,
            config,
        );
        render_set_id = render_set_id.wrapping_add(1).max(1);
        if s.len() <= char_budget {
            best_str = Some(s);
            true
        } else {
            false
        }
    });

    if let Some(s) = best_str {
        s
    } else {
        // Fallback: always render a single node (k=1) to produce the
        // shortest possible preview, even if it exceeds the byte budget.
        crate::serialization::render_top_k(
            order_build,
            1,
            &mut inclusion_flags,
            render_set_id,
            config,
        )
    }
}

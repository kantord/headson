use anyhow::Result;

mod order;
mod serialization;
mod stream_arena;
mod utils;
// templates moved under serialization
// tests that used to live under `tree` moved into serialization; no tree module
pub use order::{
    NodeId, NodeKind, ParentId, PriorityConfig, PriorityOrder, RankedNode,
    build_priority_order_from_arena,
};

pub use serialization::types::{OutputTemplate, RenderConfig};

pub fn headson(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    // Streaming arena parse from owned bytes + frontier adapter
    let arena = crate::stream_arena::build_stream_arena_from_bytes(
        input,
        priority_cfg,
    )?;
    let order_build =
        order::build_priority_order_from_arena(&arena, priority_cfg)?;
    let out = find_largest_render_under_budget(&order_build, config, budget)?;
    // Apply newline preference: allow replacing default "\n" with configured sequence
    // (supports "" for one-line output).
    let out = if config.newline != "\n" {
        out.replace('\n', &config.newline)
    } else {
        out
    };
    Ok(out)
}

fn find_largest_render_under_budget(
    order_build: &PriorityOrder,
    config: &RenderConfig,
    char_budget: usize,
) -> Result<String> {
    // Binary search the largest k in [1, total] whose render
    // fits within `char_budget`.
    let total = order_build.total_nodes;
    if total == 0 {
        return Ok(String::new());
    }
    // Each included node contributes at least some output; cap hi by budget.
    let lo = 1usize;
    let hi = total.min(char_budget.max(1));
    // Reusable inclusion marks to avoid clearing per probe
    let mut marks: Vec<u32> = vec![0; total];
    let mut mark_gen: u32 = 1;
    let mut best_str: Option<String> = None;

    let _ = crate::utils::search::binary_search_max(lo, hi, |mid| {
        let s = match crate::serialization::render_arena_with_marks(
            order_build,
            mid,
            &mut marks,
            mark_gen,
            config,
        ) {
            Ok(v) => v,
            Err(_) => return false,
        };
        mark_gen = mark_gen.wrapping_add(1).max(1);
        if s.len() <= char_budget {
            best_str = Some(s);
            true
        } else {
            false
        }
    });

    if let Some(s) = best_str {
        Ok(s)
    } else {
        // Fallback: always render a single node (k=1) to produce the
        // shortest possible preview, even if it exceeds the byte budget.
        let s = crate::serialization::render_arena_with_marks(
            order_build,
            1,
            &mut marks,
            mark_gen,
            config,
        )?;
        Ok(s)
    }
}

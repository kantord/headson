use anyhow::Result;

mod queue;
mod render;
mod stream_arena;
mod tree;
pub use queue::{
    NodeId, NodeKind, PQBuild, PQConfig, ParentId, QueueItem,
    build_priority_queue_from_arena,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OutputTemplate {
    Json,
    Pseudo,
    Js,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderConfig {
    pub template: OutputTemplate,
    pub indent_unit: String,
    pub space: String,
    pub profile: bool,
}

pub fn headson(
    input: Vec<u8>,
    config: &RenderConfig,
    pq_cfg: &PQConfig,
    budget: usize,
) -> Result<String> {
    let do_prof = config.profile;
    let t0 = std::time::Instant::now();
    // Streaming arena parse from owned bytes + frontier adapter
    let arena =
        crate::stream_arena::build_stream_arena_from_bytes(input, pq_cfg)?;
    let t1 = std::time::Instant::now();
    let pq_build = queue::build_priority_queue_from_arena(&arena, pq_cfg)?;
    let t2 = std::time::Instant::now();
    let out = best_render_under_char_budget(&pq_build, config, budget)?;
    let t3 = std::time::Instant::now();
    if do_prof {
        let p = &pq_build.profile;
        eprintln!(
            "pq breakdown: walk={}ms (strings={}, chars={})",
            p.walk_ms, p.strings, p.string_chars,
        );
        eprintln!(
            "pq details: arrays={} (items_total={}), objects={} (props_total={}), maxlens: array={}, object={}, string={}, edges={}",
            p.arrays,
            p.arrays_items_total,
            p.objects,
            p.objects_props_total,
            p.max_array_len,
            p.max_object_len,
            p.max_string_len,
            p.children_edges_total,
        );
        eprintln!(
            "timings: parse={}ms, pq={}ms, search+render={}ms, total={}ms",
            (t1 - t0).as_millis(),
            (t2 - t1).as_millis(),
            (t3 - t2).as_millis(),
            (t3 - t0).as_millis()
        );
    }
    Ok(out)
}

fn best_render_under_char_budget(
    pq_build: &PQBuild,
    config: &RenderConfig,
    char_budget: usize,
) -> Result<String> {
    // Binary search the largest k in [1, total] whose render fits into char_budget
    let total = pq_build.total_nodes;
    if total == 0 {
        return Ok(String::new());
    }
    if char_budget == 0 {
        return Ok(String::new());
    }
    let mut lo = 1usize;
    // Each included node contributes at least some output; cap upper bound by budget.
    let mut hi = total.min(char_budget.max(1));
    let mut best: Option<String> = None;
    let do_prof = config.profile;
    // Reusable inclusion marks to avoid clearing per probe
    let mut marks: Vec<u32> = vec![0; total];
    let mut mark_gen: u32 = 1;

    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let t_render = std::time::Instant::now();
        let s = crate::tree::render_arena_with_marks(
            pq_build, mid, &mut marks, mark_gen, config, do_prof,
        )?;
        let t_end = std::time::Instant::now();
        mark_gen = mark_gen.wrapping_add(1).max(1); // avoid 0 sentinel and handle wrap
        if do_prof {
            eprintln!(
                "probe k={}, render_ms={}, size={}",
                mid,
                (t_end - t_render).as_millis(),
                s.len()
            );
        }
        if s.len() <= char_budget {
            best = Some(s);
            lo = mid + 1;
        } else {
            hi = mid.saturating_sub(1);
        }
    }

    Ok(best.unwrap_or_default())
}

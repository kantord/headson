use anyhow::Result;
use serde_json::Value;
 
mod queue;
mod tree;
mod render;
mod stream_arena;
pub use queue::{build_priority_queue, build_priority_queue_with_config, PQConfig, NodeId, ParentId, NodeKind, QueueItem, PQBuild};


pub fn parse_json(input: &str, _budget: usize) -> Result<Value> {
    // Stage 1: swap serde_json for simd-json (serde bridge) for faster parsing.
    // simd-json parses in-place, so we need a mutable buffer.
    let mut bytes = input.as_bytes().to_vec();
    let v: Value = simd_json::serde::from_slice(&mut bytes)?;
    Ok(v)
}

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

// legacy helper no longer used

pub fn headson(input: &str, config: RenderConfig, budget: usize) -> Result<String> {
    let do_prof = config.profile;
    let t0 = std::time::Instant::now();
    let parsed = parse_json(input, budget)?;
    let t1 = std::time::Instant::now();
    let pq_build = build_priority_queue(&parsed)?;
    let t2 = std::time::Instant::now();
    let out = best_render_under_char_budget(&pq_build, config.clone(), budget)?;
    let t3 = std::time::Instant::now();
    if do_prof {
        let p = &pq_build.profile;
        eprintln!(
            "pq breakdown: walk={}ms (strings={}, chars={}, enum={}ms) sort={}ms maps={}ms",
            p.walk_ms,
            p.strings,
            p.string_chars,
            p.string_enum_ns / 1_000_000,
            p.sort_ms,
            p.maps_ms
        );
        eprintln!(
            "pq details: arrays={} (items_total={}), objects={} (props_total={}), maxlens: array={}, object={}, string={}, long_strings(1k/10k)={}/{}, edges={}, fill={}ms, child_sort={}ms",
            p.arrays, p.arrays_items_total, p.objects, p.objects_props_total,
            p.max_array_len, p.max_object_len, p.max_string_len,
            p.long_strings_over_1k, p.long_strings_over_10k,
            p.children_edges_total,
            p.map_fill_ns as u128 / 1_000_000,
            p.child_sort_ns as u128 / 1_000_000,
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

pub fn headson_with_cfg(input: &str, config: RenderConfig, pq_cfg: &PQConfig, budget: usize) -> Result<String> {
    let do_prof = config.profile;
    let t0 = std::time::Instant::now();
    // Stage 2: streaming arena parse + frontier adapter
    let arena = crate::stream_arena::build_stream_arena(input, pq_cfg)?;
    let t1 = std::time::Instant::now();
    let pq_build = queue::build_priority_queue_from_arena(&arena, pq_cfg)?;
    let t2 = std::time::Instant::now();
    let out = best_render_under_char_budget(&pq_build, config.clone(), budget)?;
    let t3 = std::time::Instant::now();
    if do_prof {
        let p = &pq_build.profile;
        eprintln!(
            "pq breakdown: walk={}ms (strings={}, chars={}, enum={}ms) sort={}ms maps={}ms",
            p.walk_ms,
            p.strings,
            p.string_chars,
            p.string_enum_ns / 1_000_000,
            p.sort_ms,
            p.maps_ms
        );
        eprintln!(
            "pq details: arrays={} (items_total={}), objects={} (props_total={}), maxlens: array={}, object={}, string={}, long_strings(1k/10k)={}/{}, edges={}, fill={}ms, child_sort={}ms",
            p.arrays, p.arrays_items_total, p.objects, p.objects_props_total,
            p.max_array_len, p.max_object_len, p.max_string_len,
            p.long_strings_over_1k, p.long_strings_over_10k,
            p.children_edges_total,
            p.map_fill_ns as u128 / 1_000_000,
            p.child_sort_ns as u128 / 1_000_000,
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

fn best_render_under_char_budget(pq_build: &PQBuild, config: RenderConfig, char_budget: usize) -> Result<String> {
    // Binary search the largest k in [1, total] whose render fits into char_budget
    let total = pq_build.total_nodes;
    if total == 0 { return Ok(String::new()); }
    if char_budget == 0 { return Ok(String::new()); }
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
        let t_build = std::time::Instant::now();
        let s = crate::tree::render_arena_with_marks(pq_build, mid, &mut marks, mark_gen, &config, do_prof)?;
        let t_end = std::time::Instant::now();
        mark_gen = mark_gen.wrapping_add(1).max(1); // avoid 0 sentinel and handle wrap
        if do_prof {
            eprintln!(
                "probe k={}, build={}ms, render={}ms, size={}",
                mid,
                0,
                (t_end - t_build).as_millis(),
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

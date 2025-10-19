use anyhow::Result;
use serde_json::Value;
 
mod queue;
mod tree;
mod render;
pub use queue::{build_priority_queue, NodeId, ParentId, NodeKind, QueueItem, PQBuild};
pub use tree::{build_tree, TreeKind, TreeNode};


pub fn parse_json(input: &str, _budget: usize) -> Result<Value> {
    let parsed_value: Value = serde_json::from_str(input)?;
    Ok(parsed_value)
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
    let mut lo = 1usize;
    let mut hi = total;
    let mut best: Option<String> = None;
    let do_prof = config.profile;

    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let t_build = std::time::Instant::now();
        let tree = build_tree(pq_build, mid)?;
        let t_render_start = std::time::Instant::now();
        let s = tree.serialize(&config);
        let t_end = std::time::Instant::now();
        if do_prof {
            eprintln!(
                "probe k={}, build={}ms, render={}ms, size={}",
                mid,
                (t_render_start - t_build).as_millis(),
                (t_end - t_render_start).as_millis(),
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


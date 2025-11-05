#![doc = include_str!("../README.md")]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![allow(
    clippy::multiple_crate_versions,
    reason = "Dependency graph pulls distinct versions (e.g., yaml-rust2)."
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

mod debug;
mod format;
mod ingest;
mod order;
mod serialization;
mod utils;
pub use order::types::{ArrayBias, ArraySamplerStrategy};
pub use order::{
    NodeId, NodeKind, PriorityConfig, PriorityOrder, RankedNode, build_order,
};

pub use serialization::color::resolve_color_enabled;
pub use serialization::types::{
    ColorMode, OutputTemplate, RenderConfig, Style,
};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Budgets {
    pub byte_budget: Option<usize>,
    pub char_budget: Option<usize>,
    pub line_budget: Option<usize>,
}

pub fn headson(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    let arena = crate::ingest::parse_json_one(input, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budgets(
        &order_build,
        config,
        Budgets {
            byte_budget: Some(budget),
            char_budget: None,
            line_budget: None,
        },
    );
    Ok(out)
}

pub fn headson_many(
    inputs: Vec<(String, Vec<u8>)>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    let arena = crate::ingest::parse_json_many(inputs, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budgets(
        &order_build,
        config,
        Budgets {
            byte_budget: Some(budget),
            char_budget: None,
            line_budget: None,
        },
    );
    Ok(out)
}

/// Same as `headson` but using the YAML ingest path.
pub fn headson_yaml(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    let arena = crate::ingest::parse_yaml_one(input, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budgets(
        &order_build,
        config,
        Budgets {
            byte_budget: Some(budget),
            char_budget: None,
            line_budget: None,
        },
    );
    Ok(out)
}

/// Same as `headson_many` but using the YAML ingest path.
pub fn headson_many_yaml(
    inputs: Vec<(String, Vec<u8>)>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    let arena = crate::ingest::parse_yaml_many(inputs, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budgets(
        &order_build,
        config,
        Budgets {
            byte_budget: Some(budget),
            char_budget: None,
            line_budget: None,
        },
    );
    Ok(out)
}

/// Same as `headson` but using the Text ingest path.
pub fn headson_text(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    let arena = crate::ingest::parse_text_one(input, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budgets(
        &order_build,
        config,
        Budgets {
            byte_budget: Some(budget),
            char_budget: None,
            line_budget: None,
        },
    );
    Ok(out)
}

/// Same as `headson_many` but using the Text ingest path.
pub fn headson_many_text(
    inputs: Vec<(String, Vec<u8>)>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budget: usize,
) -> Result<String> {
    let arena = crate::ingest::parse_text_many(inputs, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    let out = find_largest_render_under_budgets(
        &order_build,
        config,
        Budgets {
            byte_budget: Some(budget),
            char_budget: None,
            line_budget: None,
        },
    );
    Ok(out)
}

/// New generalized budgeting: enforce optional char and/or line caps.
fn find_largest_render_under_budgets(
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
    // Each included node contributes at least some output; cap hi by budget.
    let lo = 1usize;
    // For the upper bound, when a byte budget is present, we can safely cap by it;
    // otherwise, cap by total.
    let hi = match budgets.byte_budget {
        Some(c) => total.min(c.max(1)),
        None => total,
    };
    // Reuse render-inclusion flags across render attempts to avoid clearing the vector.
    // A node participates in the current render attempt when inclusion_flags[id] == render_set_id.
    let mut inclusion_flags: Vec<u32> = vec![0; total];
    // Each render attempt bumps this non-zero identifier to create a fresh inclusion set.
    let mut render_set_id: u32 = 1;
    // Measure length without color so ANSI escapes do not count toward the
    // byte budget. Then render once more with the requested color setting.
    let mut best_k: Option<usize> = None;
    let mut measure_cfg = config.clone();
    measure_cfg.color_enabled = false;

    let _ = crate::utils::search::binary_search_max(lo, hi, |mid| {
        let s = crate::serialization::render_top_k(
            order_build,
            mid,
            &mut inclusion_flags,
            render_set_id,
            &measure_cfg,
        );
        render_set_id = render_set_id.wrapping_add(1).max(1);
        // Measure output using a unified stats helper and enforce
        // all provided caps (chars and/or lines).
        let stats = crate::utils::measure::count_output_stats(
            &s,
            budgets.char_budget.is_some(),
        );
        let fits_bytes = budgets.byte_budget.is_none_or(|c| stats.bytes <= c);
        let fits_chars = budgets.char_budget.is_none_or(|c| stats.chars <= c);
        let fits_lines = budgets.line_budget.is_none_or(|l| stats.lines <= l);
        if fits_bytes && fits_chars && fits_lines {
            best_k = Some(mid);
            true
        } else {
            false
        }
    });

    let k = best_k.unwrap_or(1);

    // Prepare final inclusion set once and optionally build debug JSON.
    crate::serialization::prepare_render_set_top_k_and_ancestors(
        order_build,
        k,
        &mut inclusion_flags,
        render_set_id,
    );

    // If debug is enabled, emit a one-shot debug JSON built from this set.
    if config.debug {
        let mut no_color_cfg = config.clone();
        no_color_cfg.color_enabled = false;
        let measured = crate::serialization::render_from_render_set(
            order_build,
            &inclusion_flags,
            render_set_id,
            &no_color_cfg,
        );
        let stats = crate::utils::measure::count_output_stats(
            &measured,
            budgets.char_budget.is_some(),
        );
        let constrained_by = constrained_dimensions(budgets, &stats);
        let out_stats = crate::debug::OutputStatsDbg {
            bytes: stats.bytes,
            chars: stats.chars,
            lines: stats.lines,
        };
        let input_format = "json"; // format-agnostic fallback for debug metadata
        let array_sampler = crate::ArraySamplerStrategy::Default;
        let dbg = crate::debug::build_render_debug_json(
            crate::debug::RenderDebugArgs {
                order: order_build,
                inclusion_flags: &inclusion_flags,
                render_id: render_set_id,
                cfg: config,
                budgets,
                input_format,
                style: config.style,
                array_sampler,
                top_k: k,
                output_stats: out_stats,
                constrained_by,
            },
        );
        #[allow(
            clippy::print_stderr,
            reason = "Debug mode emits JSON to stderr to aid troubleshooting"
        )]
        {
            eprintln!("{dbg}");
        }
    }

    // Final render with requested color settings
    crate::serialization::render_from_render_set(
        order_build,
        &inclusion_flags,
        render_set_id,
        config,
    )
}

// (removed) render_final helper was inlined to centralize optional debug dump

// Optional new public API that accepts both budgets explicitly.
pub fn headson_with_budgets(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budgets: Budgets,
) -> Result<String> {
    let arena = crate::ingest::parse_json_one(input, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

pub fn headson_many_with_budgets(
    inputs: Vec<(String, Vec<u8>)>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budgets: Budgets,
) -> Result<String> {
    let arena = crate::ingest::parse_json_many(inputs, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

pub fn headson_yaml_with_budgets(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budgets: Budgets,
) -> Result<String> {
    let arena = crate::ingest::parse_yaml_one(input, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

pub fn headson_many_yaml_with_budgets(
    inputs: Vec<(String, Vec<u8>)>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budgets: Budgets,
) -> Result<String> {
    let arena = crate::ingest::parse_yaml_many(inputs, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

pub fn headson_text_with_budgets(
    input: Vec<u8>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budgets: Budgets,
) -> Result<String> {
    let arena = crate::ingest::parse_text_one(input, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

pub fn headson_many_text_with_budgets(
    inputs: Vec<(String, Vec<u8>)>,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budgets: Budgets,
) -> Result<String> {
    let arena = crate::ingest::parse_text_many(inputs, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;
    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

// Debug-enabled public APIs (CLI uses these when --debug is set). These emit a
// JSON trace to stderr that reflects the exact inclusion set used for stdout.

fn constrained_dimensions(
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

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

pub mod budget;
mod debug;
mod grep;
mod ingest;
mod order;
mod pruner;
mod serialization;
mod utils;
pub use grep::build_grep_config;
pub use grep::{GrepConfig, GrepShow};
pub use ingest::fileset::{FilesetInput, FilesetInputKind};
pub use order::types::{ArrayBias, ArraySamplerStrategy};
pub use order::{
    NodeId, NodeKind, PriorityConfig, PriorityOrder, RankedNode, build_order,
};
pub use utils::extensions;
pub use utils::templates::map_json_template_for_style;

pub use pruner::budget::find_largest_render_under_budgets;
pub use prunist::{Budget, BudgetKind, Budgets};
pub use serialization::color::resolve_color_enabled;
pub use serialization::types::{
    ColorMode, ColorStrategy, OutputTemplate, RenderConfig, Style,
};

#[derive(Copy, Clone, Debug)]
pub enum TextMode {
    Plain,
    CodeLike,
}

pub enum InputKind {
    Json(Vec<u8>),
    Yaml(Vec<u8>),
    Text { bytes: Vec<u8>, mode: TextMode },
    Fileset(Vec<FilesetInput>),
}

pub fn headson(
    input: InputKind,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    grep: &GrepConfig,
    budgets: Budgets,
) -> Result<(String, Vec<String>)> {
    let mut prio = *priority_cfg;
    if grep.regex.is_some() && !grep.weak {
        // Avoid sampling away potential matches in strong grep mode.
        prio.array_max_items = usize::MAX;
    }
    let (arena, notices) = match input {
        InputKind::Fileset(inputs) => {
            ingest::fileset::parse_fileset_multi_with_notices(inputs, &prio)
        }
        other => (crate::ingest::ingest_into_arena(other, &prio)?, Vec::new()),
    };
    let mut order_build = order::build_order(&arena, &prio)?;
    let out = find_largest_render_under_budgets(
        &mut order_build,
        config,
        grep,
        budgets,
    );
    Ok((out, notices))
}

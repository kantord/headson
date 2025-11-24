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
mod ingest;
mod order;
mod pruner;
mod serialization;
mod utils;
pub use ingest::fileset::{FilesetInput, FilesetInputKind};
pub use order::types::{ArrayBias, ArraySamplerStrategy};
pub use order::{
    NodeId, NodeKind, PriorityConfig, PriorityOrder, RankedNode, build_order,
};
pub use utils::extensions;

pub use pruner::budget::{Budgets, find_largest_render_under_budgets};
pub use serialization::color::resolve_color_enabled;
pub use serialization::types::{
    ColorMode, OutputTemplate, RenderConfig, Style,
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
    budgets: Budgets,
) -> Result<String> {
    let arena = crate::ingest::ingest_into_arena(input, priority_cfg)?;
    let order_build = order::build_order(&arena, priority_cfg)?;

    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

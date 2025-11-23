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

pub enum InputKind {
    Json(Vec<u8>),
    JsonMany(Vec<(String, Vec<u8>)>),
    Yaml(Vec<u8>),
    YamlMany(Vec<(String, Vec<u8>)>),
    Text {
        bytes: Vec<u8>,
        atomic: bool,
    },
    TextMany {
        inputs: Vec<(String, Vec<u8>)>,
        atomic: bool,
    },
    Fileset(Vec<FilesetInput>),
}

pub fn headson(
    input: InputKind,
    config: &RenderConfig,
    priority_cfg: &PriorityConfig,
    budgets: Budgets,
) -> Result<String> {
    let order_build = match input {
        InputKind::Json(bytes) => {
            let arena = crate::ingest::parse_json_one(bytes, priority_cfg)?;
            order::build_order(&arena, priority_cfg)?
        }
        InputKind::JsonMany(inputs) => {
            let arena = crate::ingest::parse_json_many(inputs, priority_cfg)?;
            order::build_order(&arena, priority_cfg)?
        }
        InputKind::Yaml(bytes) => {
            let arena = crate::ingest::parse_yaml_one(bytes, priority_cfg)?;
            order::build_order(&arena, priority_cfg)?
        }
        InputKind::YamlMany(inputs) => {
            let arena = crate::ingest::parse_yaml_many(inputs, priority_cfg)?;
            order::build_order(&arena, priority_cfg)?
        }
        InputKind::Text { bytes, atomic } => {
            let arena =
                crate::ingest::formats::text::build_text_tree_arena_from_bytes_with_mode(
                    bytes,
                    priority_cfg,
                    atomic,
                )?;
            order::build_order(&arena, priority_cfg)?
        }
        InputKind::TextMany { inputs, .. } => {
            let arena = crate::ingest::parse_text_many(inputs, priority_cfg)?;
            order::build_order(&arena, priority_cfg)?
        }
        InputKind::Fileset(inputs) => {
            let arena = crate::ingest::fileset::parse_fileset_multi(
                inputs,
                priority_cfg,
            )?;
            order::build_order(&arena, priority_cfg)?
        }
    };
    Ok(find_largest_render_under_budgets(
        &order_build,
        config,
        budgets,
    ))
}

use headson::{ArraySamplerStrategy, PriorityConfig, RenderConfig};

use crate::Cli;

// CLI-facing budget helpers: compute effective caps and priority tuning derived from flag inputs.
// Default per-input byte cap when no explicit budgets are provided anywhere.
pub const DEFAULT_BYTES_PER_INPUT: usize = 500;
// When only line budgets are active, allow this many graphemes before trimming strings.
pub const LINE_ONLY_FREE_PREFIX_GRAPHEMES: usize = 40;

#[derive(Debug, Copy, Clone)]
pub struct EffectiveBudgets {
    // Final budgets passed to the renderer/search.
    pub budgets: headson::Budgets,
    // Per-file budget used to size priority heuristics (e.g., array_max_items in PriorityConfig).
    pub per_file_for_priority: usize,
    // Whether only line caps are active (no bytes); used to lift array limits and string trimming
    // during ordering and render prep so structure survives in line-only mode.
    pub line_only: bool,
}

pub(crate) fn compute_effective(
    cli: &Cli,
    input_count: usize,
) -> EffectiveBudgets {
    let any_bytes = cli.bytes.is_some() || cli.global_bytes.is_some();
    let any_lines = cli.lines.is_some() || cli.global_lines.is_some();
    let any_chars = cli.chars.is_some();

    let effective_bytes = effective_bytes(cli, input_count);
    let effective_chars = effective_chars(cli, input_count);
    let effective_lines = effective_lines(cli, input_count);
    let byte_budget =
        compute_byte_budget(any_bytes, any_lines, any_chars, effective_bytes);

    let budgets = headson::Budgets {
        byte_budget,
        char_budget: if any_chars { effective_chars } else { None },
        line_budget: effective_lines,
    };

    let chosen_global =
        compute_global_cap(any_bytes, effective_bytes, effective_chars);
    let per_file_for_priority = (chosen_global / input_count.max(1)).max(1);

    EffectiveBudgets {
        budgets,
        per_file_for_priority,
        line_only: any_lines && !any_bytes,
    }
}

fn effective_bytes(cli: &Cli, input_count: usize) -> usize {
    match (cli.global_bytes, cli.bytes) {
        (Some(g), Some(n)) => g.min(n.saturating_mul(input_count)),
        (Some(g), None) => g,
        (None, Some(n)) => n.saturating_mul(input_count),
        (None, None) => DEFAULT_BYTES_PER_INPUT.saturating_mul(input_count),
    }
}

fn effective_chars(cli: &Cli, input_count: usize) -> Option<usize> {
    cli.chars.map(|n| n.saturating_mul(input_count))
}

fn effective_lines(cli: &Cli, input_count: usize) -> Option<usize> {
    match (cli.global_lines, cli.lines) {
        (Some(g), Some(n)) => Some(g.min(n.saturating_mul(input_count))),
        (Some(g), None) => Some(g),
        (None, Some(n)) => Some(n.saturating_mul(input_count)),
        (None, None) => None,
    }
}

fn compute_byte_budget(
    any_bytes: bool,
    any_lines: bool,
    any_chars: bool,
    effective_bytes: usize,
) -> Option<usize> {
    if any_bytes {
        Some(effective_bytes)
    } else if any_lines || any_chars {
        None
    } else {
        Some(effective_bytes)
    }
}

fn compute_global_cap(
    any_bytes: bool,
    effective_bytes: usize,
    effective_chars: Option<usize>,
) -> usize {
    if any_bytes {
        effective_bytes
    } else if let Some(c) = effective_chars {
        c
    } else {
        effective_bytes
    }
}

// Return a rendering config adjusted for active budget modes (pure; does not mutate caller state).
// In practice this only lifts string trimming when running line-only (lines set, no bytes).
pub(crate) fn render_config_for_budgets(
    mut cfg: RenderConfig,
    effective: &EffectiveBudgets,
) -> RenderConfig {
    if effective.budgets.byte_budget.is_none()
        && effective.budgets.char_budget.is_none()
        && effective.budgets.line_budget.is_some()
    {
        cfg.string_free_prefix_graphemes =
            Some(LINE_ONLY_FREE_PREFIX_GRAPHEMES);
    }
    cfg
}

pub(crate) fn build_priority_config(
    cli: &Cli,
    effective: &EffectiveBudgets,
) -> PriorityConfig {
    let sampler = if cli.tail {
        ArraySamplerStrategy::Tail
    } else if cli.head {
        ArraySamplerStrategy::Head
    } else {
        ArraySamplerStrategy::Default
    };
    PriorityConfig::for_budget(
        cli.string_cap,
        effective.per_file_for_priority,
        cli.tail,
        sampler,
        effective.line_only,
    )
}

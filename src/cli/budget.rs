use anyhow::{Result, bail};
use headson::{
    ArraySamplerStrategy, Budget, BudgetKind, Budgets, PriorityConfig,
    RenderConfig,
};

use crate::Cli;

// CLI-facing budget helpers: compute effective caps and priority tuning derived from flag inputs.
// Default per-input byte cap when no explicit budgets are provided anywhere.
pub const DEFAULT_BYTES_PER_INPUT: usize = 500;
// When only line budgets are active, allow this many graphemes before trimming strings.
pub const LINE_ONLY_FREE_PREFIX_GRAPHEMES: usize = 40;

#[derive(Debug, Copy, Clone)]
pub struct EffectiveBudgets {
    // Final budgets passed to the renderer/search.
    pub budgets: Budgets,
    // Per-file budget used to size priority heuristics (e.g., array_max_items in PriorityConfig).
    // Ignored when line_only is true (line-only mode lifts array caps entirely).
    pub per_file_for_priority: usize,
    // Whether only line caps are active (no bytes); used to lift array limits and string trimming
    // during ordering and render prep so structure survives in line-only mode.
    pub line_only: bool,
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Validation + default wiring is clearer in one routine; splitting would scatter the budget rules."
)]
pub(crate) fn compute_effective(
    cli: &Cli,
    input_count: usize,
) -> EffectiveBudgets {
    let mut per_slot = per_slot_budget(cli);
    let explicit_global = explicit_global_budget(cli);

    // Defaults and implicit roll-ups:
    // - If no budgets provided anywhere, default to a global 500-byte cap scaled by input count.
    // - If a per-slot byte/char budget is provided without an explicit global, roll a matching
    //   global cap by multiplying by input count (existing behavior: per-file caps add up).
    // - Line caps stay per-slot unless the user passes a global line cap.
    let mut global = explicit_global;
    if global.is_none() {
        match per_slot {
            Some(Budget {
                kind: BudgetKind::Bytes,
                cap,
            }) => {
                global = Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: cap.saturating_mul(input_count),
                });
            }
            Some(Budget {
                kind: BudgetKind::Chars,
                cap,
            }) => {
                global = Some(Budget {
                    kind: BudgetKind::Chars,
                    cap: cap.saturating_mul(input_count),
                });
            }
            Some(Budget {
                kind: BudgetKind::Lines,
                ..
            }) => {
                // No implicit global for line caps.
            }
            None => {
                per_slot = Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: DEFAULT_BYTES_PER_INPUT,
                });
                global = Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: DEFAULT_BYTES_PER_INPUT.saturating_mul(input_count),
                });
            }
        }
    }

    let budgets = Budgets { global, per_slot };

    let has_lines = matches!(
        budgets.global,
        Some(Budget {
            kind: BudgetKind::Lines,
            ..
        })
    ) || matches!(
        budgets.per_slot,
        Some(Budget {
            kind: BudgetKind::Lines,
            ..
        })
    );
    let has_bytes_or_chars = matches!(
        budgets.global,
        Some(Budget {
            kind: BudgetKind::Bytes | BudgetKind::Chars,
            ..
        })
    ) || matches!(
        budgets.per_slot,
        Some(Budget {
            kind: BudgetKind::Bytes | BudgetKind::Chars,
            ..
        })
    );
    let line_only = has_lines && !has_bytes_or_chars;

    let chosen_global = budgets
        .global
        .map(|b| b.cap)
        .unwrap_or(DEFAULT_BYTES_PER_INPUT.saturating_mul(input_count));
    // In line-only mode, PriorityConfig lifts array limits entirely; make that
    // explicit by using usize::MAX instead of a byte/char-derived heuristic.
    let per_file_for_priority = if line_only {
        usize::MAX
    } else {
        (chosen_global / input_count.max(1)).max(1)
    };

    EffectiveBudgets {
        budgets,
        per_file_for_priority,
        line_only,
    }
}

pub(crate) fn validate(cli: &Cli) -> Result<()> {
    let per_slot_flags = [
        cli.bytes.is_some(),
        cli.chars.is_some(),
        cli.lines.is_some(),
    ];
    let per_slot_set = per_slot_flags.iter().filter(|b| **b).count();
    if per_slot_set > 1 {
        bail!(
            "only one per-file budget (--bytes/--chars/--lines) can be set at once"
        );
    }
    let global_flags =
        [cli.global_bytes.is_some(), cli.global_lines.is_some()];
    let global_set = global_flags.iter().filter(|b| **b).count();
    if global_set > 1 {
        bail!(
            "only one global budget (--global-bytes/--global-lines) can be set at once"
        );
    }
    Ok(())
}

fn per_slot_budget(cli: &Cli) -> Option<Budget> {
    cli.bytes
        .map(|b| Budget {
            kind: BudgetKind::Bytes,
            cap: b,
        })
        .or_else(|| {
            cli.chars.map(|c| Budget {
                kind: BudgetKind::Chars,
                cap: c,
            })
        })
        .or_else(|| {
            cli.lines.map(|l| Budget {
                kind: BudgetKind::Lines,
                cap: l,
            })
        })
}

fn explicit_global_budget(cli: &Cli) -> Option<Budget> {
    cli.global_bytes
        .map(|b| Budget {
            kind: BudgetKind::Bytes,
            cap: b,
        })
        .or_else(|| {
            cli.global_lines.map(|l| Budget {
                kind: BudgetKind::Lines,
                cap: l,
            })
        })
}

// Return a rendering config adjusted for active budget modes (pure; does not mutate caller state).
// In practice this only lifts string trimming when running line-only (lines set, no bytes).
pub(crate) fn render_config_for_budgets(
    mut cfg: RenderConfig,
    effective: &EffectiveBudgets,
) -> RenderConfig {
    if effective.line_only {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::Cli;
    use clap::Parser;

    fn parse(args: &[&str]) -> Cli {
        let mut full_args = vec!["hson"];
        full_args.extend(args.iter().copied());
        Cli::parse_from(full_args)
    }

    #[test]
    fn default_per_file_budget_is_500_bytes() {
        let cli = parse(&[]);
        let effective = compute_effective(&cli, 2);
        assert_eq!(
            effective.budgets.global,
            Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 1000
            }),
            "default byte budget should scale by input count (500 each)"
        );
        assert_eq!(
            effective.budgets.per_slot,
            Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 500
            }),
            "defaults should still enforce a per-file 500-byte cap so later files cannot be starved"
        );
        assert_eq!(
            effective.per_file_for_priority, 500,
            "priority tuning should still use 500 per file by default"
        );
    }

    #[test]
    fn mixed_level_metrics_are_allowed() {
        let cli = parse(&["-n", "3", "-C", "120"]);
        let effective = compute_effective(&cli, 1);
        assert_eq!(
            effective.budgets.per_slot,
            Some(Budget {
                kind: BudgetKind::Lines,
                cap: 3
            }),
            "per-file line cap should be set when provided"
        );
        assert_eq!(
            effective.budgets.global,
            Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 120
            }),
            "global byte cap should propagate when provided"
        );
    }
}

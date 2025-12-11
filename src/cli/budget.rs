use anyhow::{Result, bail};
use headson::budget::{
    DEFAULT_BYTES_PER_INPUT, EffectiveBudgets, compute_effective_budgets,
};
use headson::{
    ArraySamplerStrategy, Budget, BudgetKind, PriorityConfig, RenderConfig,
};

use crate::Cli;

pub(crate) fn compute_effective(
    cli: &Cli,
    input_count: usize,
) -> EffectiveBudgets {
    let mut per_slot = per_slot_budget(cli);
    let explicit_global = explicit_global_budget(cli);
    if per_slot.is_none() && explicit_global.is_none() {
        per_slot = Some(Budget {
            kind: BudgetKind::Bytes,
            cap: DEFAULT_BYTES_PER_INPUT,
        });
    }
    compute_effective_budgets(
        per_slot,
        explicit_global,
        input_count,
        DEFAULT_BYTES_PER_INPUT,
    )
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
    cfg: RenderConfig,
    effective: &EffectiveBudgets,
) -> RenderConfig {
    headson::budget::render_config_for_budgets(cfg, effective)
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

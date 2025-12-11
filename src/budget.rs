use crate::{Budget, BudgetKind, Budgets};

/// Default per-input byte cap when no explicit budgets are provided.
pub const DEFAULT_BYTES_PER_INPUT: usize = 500;
/// When only line budgets are active, allow this many graphemes before trimming strings.
pub const LINE_ONLY_FREE_PREFIX_GRAPHEMES: usize = 40;

#[derive(Debug, Copy, Clone)]
pub struct EffectiveBudgets {
    /// Final budgets passed to the renderer/search.
    pub budgets: Budgets,
    /// Per-file budget used to size priority heuristics (e.g., array_max_items).
    pub per_file_for_priority: usize,
    /// Whether only line caps are active (no bytes); used to lift array limits and string trimming.
    pub line_only: bool,
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Keeps budget roll-up rules in one place; splitting would scatter the defaults."
)]
pub fn compute_effective_budgets(
    per_slot: Option<Budget>,
    explicit_global: Option<Budget>,
    input_count: usize,
    default_per_input: usize,
) -> EffectiveBudgets {
    let mut per_slot = per_slot;
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
            }) => {}
            None => {
                per_slot = Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: default_per_input,
                });
                global = Some(Budget {
                    kind: BudgetKind::Bytes,
                    cap: default_per_input.saturating_mul(input_count),
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

    let per_file_for_priority =
        priority_cap(&budgets, input_count, default_per_input, line_only);

    EffectiveBudgets {
        budgets,
        per_file_for_priority,
        line_only,
    }
}

fn priority_cap(
    budgets: &Budgets,
    input_count: usize,
    default_per_input: usize,
    line_only: bool,
) -> usize {
    if line_only {
        return usize::MAX;
    }
    let per_slot_cap = budgets
        .per_slot
        .and_then(|b| cap_for_priority(b, true, input_count));
    let global_cap = budgets
        .global
        .and_then(|b| cap_for_priority(b, false, input_count));

    per_slot_cap
        .into_iter()
        .chain(global_cap)
        .min()
        .unwrap_or(default_per_input)
}

fn cap_for_priority(
    budget: Budget,
    is_per_slot: bool,
    input_count: usize,
) -> Option<usize> {
    match budget.kind {
        BudgetKind::Bytes | BudgetKind::Chars => {
            if is_per_slot {
                Some(budget.cap)
            } else {
                Some((budget.cap / input_count.max(1)).max(1))
            }
        }
        BudgetKind::Lines => None,
    }
}

/// Adjust render configuration based on effective budget modes (shared across CLI/Python).
pub fn render_config_for_budgets(
    mut cfg: crate::RenderConfig,
    effective: &EffectiveBudgets,
) -> crate::RenderConfig {
    if effective.line_only {
        cfg.string_free_prefix_graphemes =
            Some(LINE_ONLY_FREE_PREFIX_GRAPHEMES);
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_file_priority_prefers_per_slot_cap_when_global_is_lines() {
        let effective = compute_effective_budgets(
            Some(Budget {
                kind: BudgetKind::Bytes,
                cap: 1024,
            }),
            Some(Budget {
                kind: BudgetKind::Lines,
                cap: 5,
            }),
            1,
            DEFAULT_BYTES_PER_INPUT,
        );
        assert_eq!(
            effective.per_file_for_priority, 1024,
            "per-file priority tuning should respect per-slot byte caps even when the global budget is in lines"
        );
    }
}

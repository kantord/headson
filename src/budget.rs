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

    let chosen_global = budgets
        .global
        .map(|b| b.cap)
        .unwrap_or(default_per_input.saturating_mul(input_count));
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

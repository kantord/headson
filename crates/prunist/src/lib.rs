mod search;
mod select;

pub use search::binary_search_max;
pub use select::{
    MustKeep, MustKeepStats, PruningConfig, PruningContext, PruningResult,
    select_best_k,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BudgetKind {
    Bytes,
    Chars,
    Lines,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Budget {
    pub kind: BudgetKind,
    pub cap: usize,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Budgets {
    pub global: Option<Budget>,
    pub per_slot: Option<Budget>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OutputStats {
    pub bytes: usize,
    pub chars: usize,
    pub lines: usize,
}

impl Budget {
    pub fn exceeds(&self, stats: &OutputStats) -> bool {
        match self.kind {
            BudgetKind::Bytes => stats.bytes > self.cap,
            BudgetKind::Chars => stats.chars > self.cap,
            BudgetKind::Lines => stats.lines > self.cap,
        }
    }
}

impl Budgets {
    pub fn measure_chars(&self) -> bool {
        matches!(
            self.global,
            Some(Budget {
                kind: BudgetKind::Chars,
                ..
            })
        ) || matches!(
            self.per_slot,
            Some(Budget {
                kind: BudgetKind::Chars,
                ..
            })
        )
    }

    pub fn measure_lines(&self) -> bool {
        matches!(
            self.global,
            Some(Budget {
                kind: BudgetKind::Lines,
                ..
            })
        ) || matches!(
            self.per_slot,
            Some(Budget {
                kind: BudgetKind::Lines,
                ..
            })
        )
    }

    pub fn per_slot_active(&self) -> bool {
        self.per_slot.is_some()
    }

    pub fn global_active(&self) -> bool {
        self.global.is_some()
    }

    pub fn per_slot_kind(&self) -> Option<BudgetKind> {
        self.per_slot.map(|b| b.kind)
    }

    pub fn global_kind(&self) -> Option<BudgetKind> {
        self.global.map(|b| b.kind)
    }

    pub fn per_slot_cap_for(&self, kind: BudgetKind) -> Option<usize> {
        match self.per_slot {
            Some(b) if b.kind == kind => Some(b.cap),
            _ => None,
        }
    }

    pub fn global_cap_for(&self, kind: BudgetKind) -> Option<usize> {
        match self.global {
            Some(b) if b.kind == kind => Some(b.cap),
            _ => None,
        }
    }

    pub fn per_slot_zero_cap(&self) -> bool {
        matches!(self.per_slot, Some(b) if b.cap == 0)
    }
}

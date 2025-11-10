// Hard ceiling on number of PQ nodes built to prevent degenerate inputs
// from blowing up memory/time while exploring the frontier.
pub(crate) const SAFETY_CAP: usize = 2_000_000;

/// Root starts at a fixed minimal score so its children naturally follow.
pub(crate) const ROOT_BASE_SCORE: u128 = 1;

/// Small base increment so array children follow the parent.
pub(crate) const ARRAY_CHILD_BASE_INCREMENT: u128 = 1;
/// Strong cubic index term to bias earlier array items far ahead of later ones.
/// The large multiplier ensures array index dominates depth ties.
pub(crate) const ARRAY_INDEX_CUBIC_WEIGHT: u128 = 1_000_000_000_000;

/// Small base increment so object properties appear right after their object.
pub(crate) const OBJECT_CHILD_BASE_INCREMENT: u128 = 1;

/// Base increment so string grapheme expansions follow their parent string.
pub(crate) const STRING_CHILD_BASE_INCREMENT: u128 = 1;
/// Linear weight to prefer earlier graphemes strongly.
pub(crate) const STRING_CHILD_LINEAR_WEIGHT: u128 = 1;
/// Index after which we penalize graphemes quadratically to de-prioritize
/// very deep string expansions vs. structural nodes.
pub(crate) const STRING_INDEX_INFLECTION: usize = 20;
/// Quadratic penalty multiplier for string grapheme expansions beyond the
/// inflection point.
pub(crate) const STRING_INDEX_QUADRATIC_WEIGHT: u128 = 1;

/// Extra penalty applied to blank atomic lines in code contexts so they trail
/// real lines but can still appear when budgets allow.
pub(crate) const CODE_EMPTY_LINE_PENALTY: u128 = ARRAY_INDEX_CUBIC_WEIGHT * 4;

/// Bonus applied to top-level code lines that introduce nested blocks so they
/// surface before plain leaf lines under tight budgets.
pub(crate) const CODE_PARENT_LINE_BONUS: u128 = ARRAY_CHILD_BASE_INCREMENT * 4;

/// Trimmed length below this threshold is considered "very short" for code lines.
pub(crate) const CODE_SHORT_LINE_THRESHOLD: usize = 5;
/// Trimmed length above this threshold is considered "very long" for code lines.
pub(crate) const CODE_LONG_LINE_THRESHOLD: usize = 180;
/// Mild penalty applied to extremely short/long code lines to deemphasize them.
pub(crate) const CODE_EXTREME_LINE_PENALTY: u128 = 5;
/// Strong penalty for brace-only lines so they yield to more informative content.
pub(crate) const CODE_BRACE_ONLY_PENALTY: u128 = ARRAY_INDEX_CUBIC_WEIGHT;
/// Penalty for shallow code arrays (e.g., standalone braces) under tight line budgets.
pub(crate) const CODE_SHALLOW_ARRAY_PENALTY: u128 =
    ARRAY_INDEX_CUBIC_WEIGHT / 2;

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

/// Bias to lift string nodes that match any weak-grep regex.
pub(crate) const GREP_WEAK_BIAS: u128 = 1_000_000_000_000;

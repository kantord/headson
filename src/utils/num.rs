/// Saturating cast from `u128` to `usize`. Returns `usize::MAX` if `v`
/// exceeds the range of `usize` on the current platform.
pub(crate) fn saturating_cast_u128_to_usize(v: u128) -> usize {
    if v > usize::MAX as u128 {
        usize::MAX
    } else {
        v as usize
    }
}

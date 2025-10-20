// Generic helper: largest integer in [lo, hi] satisfying `pred`.
pub(crate) fn binary_search_max(
    mut lo: usize,
    mut hi: usize,
    mut pred: impl FnMut(usize) -> bool,
) -> Option<usize> {
    let mut best: Option<usize> = None;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        if pred(mid) {
            best = Some(mid);
            lo = mid.saturating_add(1);
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    best
}


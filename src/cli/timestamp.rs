//! Minimal ISO-8601 UTC timestamp formatting without a chrono dep.
//!
//! Lives here (and not in `session_middleware`) because the date math is
//! self-contained and the only consumer is the session query log writer.

fn is_leap_year(y: u64) -> bool {
    y % 400 == 0 || (y % 4 == 0 && y % 100 != 0)
}

fn year_from_unix_secs(secs: u64) -> (u64, u64) {
    let (mut y, mut rem) = (1970u64, secs);
    loop {
        let sy = if is_leap_year(y) { 366 } else { 365 } * 86_400;
        if rem < sy {
            break;
        }
        rem -= sy;
        y += 1;
    }
    (y, rem)
}

fn month_day_from_year_secs(year: u64, mut rem: u64) -> (u64, u64) {
    let month_days: [u64; 12] = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u64;
    for &d in &month_days {
        let sm = d * 86_400;
        if rem < sm {
            break;
        }
        rem -= sm;
        mo += 1;
    }
    (mo, rem / 86_400 + 1)
}

pub(crate) fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, rem) = year_from_unix_secs(secs);
    let (mo, day) = month_day_from_year_secs(y, rem);
    let time_rem = rem % 86_400;
    let (h, m, s) = (time_rem / 3600, time_rem % 3600 / 60, time_rem % 60);
    format!("{y:04}-{mo:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

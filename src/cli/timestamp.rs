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

fn format_timestamp(secs: u64) -> String {
    let (y, rem) = year_from_unix_secs(secs);
    let (mo, day) = month_day_from_year_secs(y, rem);
    let time_rem = rem % 86_400;
    let (h, m, s) = (time_rem / 3600, time_rem % 3600 / 60, time_rem % 60);
    format!("{y:04}-{mo:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

pub(crate) fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_timestamp(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected strings verified against `date -u -d @<secs>`.

    #[test]
    fn epoch_formats_as_1970_01_01() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_recent_timestamp_formats_correctly() {
        assert_eq!(format_timestamp(1_781_094_896), "2026-06-10T12:34:56Z");
    }

    #[test]
    fn leap_year_feb_29_is_produced() {
        assert_eq!(format_timestamp(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(format_timestamp(1_709_251_199), "2024-02-29T23:59:59Z");
    }

    #[test]
    fn end_of_year_rollover_is_exact() {
        assert_eq!(format_timestamp(1_704_067_199), "2023-12-31T23:59:59Z");
        assert_eq!(format_timestamp(1_704_067_200), "2024-01-01T00:00:00Z");
    }
}

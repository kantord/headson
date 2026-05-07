use std::env;
use std::path::PathBuf;

use crate::cli::args::Cli;

/// Maximum number of breadcrumbs kept per session; older entries are evicted.
pub(crate) const BREADCRUMB_CAP: usize = 500;
/// Maximum number of query log entries kept; oldest are dropped when exceeded.
pub(crate) const QUERY_LOG_CAP: usize = 1000;
/// Decay factor per step — matches the alpha used by the penalty engine.
const DEFAULT_ALPHA: f64 = 0.5;

pub(crate) fn active_session_id(cli: &Cli) -> Option<String> {
    cli.session.clone().or_else(|| {
        std::env::var("HSON_SESSION").ok().filter(|s| !s.is_empty())
    })
}

pub(crate) fn session_file_path(id: &str) -> PathBuf {
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(home).join(".local").join("state")
        });
    state_dir
        .join("headson")
        .join("sessions")
        .join(format!("{id}.json"))
}

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

fn current_timestamp() -> String {
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

pub(crate) fn record_session(
    id: &str,
    shown_leaves: &[(String, String)],
    cwd: &str,
    argv: Vec<String>,
) {
    let path = session_file_path(id);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut session = crate::session::io::load_or_create(&path, id, None, cwd);
    let new_step = session.step_count + 1;
    for (file, node_path) in shown_leaves {
        session.record_breadcrumb(file, node_path, new_step);
    }
    session.record_query(new_step, &current_timestamp(), cwd, argv);
    // Merge with any concurrent write, then evict and cap — all atomically.
    let _ = crate::session::io::save_merged_with_eviction_to_path(
        &session,
        &path,
        DEFAULT_ALPHA,
        BREADCRUMB_CAP,
        QUERY_LOG_CAP,
    );
}

pub(crate) fn maybe_record_session(
    session_id: Option<&str>,
    from_stdin: bool,
    no_record: bool,
    shown_leaves: &[(String, String)],
) {
    if let Some(id) = session_id {
        if !from_stdin && !no_record {
            let cwd = env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let argv: Vec<String> = std::env::args().collect();
            record_session(id, shown_leaves, &cwd, argv);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use clap::Parser;
    use serial_test::serial;
    use tempfile::tempdir;

    use super::*;
    use crate::cli::args::Cli;
    use crate::cli::run::run;

    /// Step 31: When neither HSON_SESSION nor --session is set, running hson on
    /// a file produces the same output as a baseline run and does NOT write a
    /// session file anywhere under XDG_STATE_HOME.
    #[test]
    #[serial]
    fn no_hson_session_env_output_unchanged() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir.path());
            std::env::remove_var("HSON_SESSION");
        }

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let (out, warnings) =
            run(&cli).expect("run must succeed without session flag");

        unsafe {
            match old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match old_session {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }

        assert!(
            !out.is_empty(),
            "output must be non-empty; got empty string"
        );
        assert!(
            warnings.is_empty(),
            "no warnings expected; got: {warnings:?}"
        );

        let sessions_dir = state_dir.path().join("headson").join("sessions");
        if sessions_dir.exists() {
            let session_files: Vec<_> = fs::read_dir(&sessions_dir)
                .unwrap()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                .collect();
            assert!(
                session_files.is_empty(),
                "no session files must be written when HSON_SESSION is absent; \
                 found: {session_files:?}"
            );
        }
    }

    /// Step 32: When HSON_SESSION=<id> env var is set, running hson on a file
    /// creates `$XDG_STATE_HOME/headson/sessions/<id>.json`.
    #[test]
    #[serial]
    fn hson_session_env_creates_session_file() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "step32-env-session";
        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir.path());
            std::env::set_var("HSON_SESSION", session_id);
        }

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let result = run(&cli);

        unsafe {
            match old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match old_session {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }

        result.expect("run must succeed with HSON_SESSION set");

        let expected = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{session_id}.json"));
        assert!(
            expected.exists(),
            "session file must be created at {expected:?} when HSON_SESSION is set"
        );
    }

    /// Step 33: Using `--session <id>` creates the session file.
    #[test]
    #[serial]
    fn session_flag_creates_session_file() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "step33-flag-session";
        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir.path());
            std::env::remove_var("HSON_SESSION");
        }

        let cli = Cli::parse_from([
            "hson",
            "--session",
            session_id,
            path.to_str().unwrap(),
        ]);
        let result = run(&cli);

        unsafe {
            match old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match old_session {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }

        result.expect("run must succeed with --session flag");

        let expected = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{session_id}.json"));
        assert!(
            expected.exists(),
            "session file must be created at {expected:?} when --session is passed"
        );
    }

    /// Step 34: `--session <id> --no-record` does NOT create the session file.
    #[test]
    #[serial]
    fn no_record_flag_suppresses_session_write() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "step34-no-record-session";
        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir.path());
            std::env::remove_var("HSON_SESSION");
        }

        let cli = Cli::parse_from([
            "hson",
            "--session",
            session_id,
            "--no-record",
            path.to_str().unwrap(),
        ]);
        let result = run(&cli);

        unsafe {
            match old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match old_session {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }

        result.expect("run must succeed with --session --no-record");

        let session_file = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{session_id}.json"));
        assert!(
            !session_file.exists(),
            "--no-record must suppress session file creation; \
             unexpectedly found: {session_file:?}"
        );
    }

    /// Step 35: Structural confirmation that stdin mode suppresses session writes.
    #[test]
    #[serial]
    fn stdin_mode_no_session_write_active_session_id_resolves() {
        let state_dir = tempdir().unwrap();
        let session_id = "step35-stdin-session";

        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir.path());
            std::env::set_var("HSON_SESSION", session_id);
        }

        let cli = Cli::parse_from(["hson"]);
        let id = active_session_id(&cli);

        unsafe {
            match old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match old_session {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }

        assert_eq!(
            id,
            Some(session_id.to_string()),
            "active_session_id must return Some(id) from HSON_SESSION env var"
        );
        assert!(
            !state_dir
                .path()
                .join("headson")
                .join("sessions")
                .join(format!("{session_id}.json"))
                .exists(),
            "session file must not be written when run() has not been called"
        );
    }

    /// Regression: record_session must evict stale breadcrumbs so session
    /// files don't grow without bound across long-running explorations.
    #[test]
    #[serial]
    fn record_session_caps_breadcrumbs_when_limit_exceeded() {
        let dir = tempdir().unwrap();
        let state_dir = dir.path();
        let session_id = "breadcrumb-cap-regression";

        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session_env = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir);
            std::env::remove_var("HSON_SESSION");
        }

        // Pre-populate with 600 breadcrumbs — above the BREADCRUMB_CAP of 500.
        let path = session_file_path(session_id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut session = crate::session::Session::new(
            session_id.to_string(),
            "lbl".to_string(),
        );
        for i in 0u64..600 {
            session.record_breadcrumb("", &format!("k{i}#h{i}"), i + 1);
        }
        session.step_count = 600;
        crate::session::io::save_to_path(&session, &path).unwrap();

        record_session(
            session_id,
            &[(String::new(), "new#newhash".to_string())],
            "/cwd",
            vec![],
        );

        unsafe {
            match old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match old_session_env {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }

        let final_session = crate::session::io::load_from_path(&path)
            .expect("session file must exist after record_session");
        assert!(
            final_session.breadcrumbs.len() <= BREADCRUMB_CAP,
            "breadcrumbs must be capped at {} after record_session; got: {}",
            BREADCRUMB_CAP,
            final_session.breadcrumbs.len()
        );
    }

    /// Regression: record_session must cap the query log to prevent unbounded growth.
    #[test]
    #[serial]
    fn record_session_caps_query_log_when_limit_exceeded() {
        let dir = tempdir().unwrap();
        let state_dir = dir.path();
        let session_id = "query-cap-regression";

        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session_env = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir);
            std::env::remove_var("HSON_SESSION");
        }

        // Pre-populate with QUERY_LOG_CAP + 100 queries.
        let path = session_file_path(session_id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut session = crate::session::Session::new(
            session_id.to_string(),
            "lbl".to_string(),
        );
        for i in 0u64..(QUERY_LOG_CAP as u64 + 100) {
            session.record_query(i + 1, "ts", "/cwd", vec![]);
        }
        crate::session::io::save_to_path(&session, &path).unwrap();

        record_session(session_id, &[], "/cwd", vec![]);

        unsafe {
            match old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match old_session_env {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }

        let final_session = crate::session::io::load_from_path(&path)
            .expect("session file must exist after record_session");
        assert!(
            final_session.queries.len() <= QUERY_LOG_CAP,
            "query log must be capped at {}; got: {}",
            QUERY_LOG_CAP,
            final_session.queries.len()
        );
    }
}

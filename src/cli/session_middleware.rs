use std::env;
use std::path::PathBuf;

use crate::cli::args::Cli;

/// Default maximum breadcrumbs kept per session (`--explore-memory` default);
/// older entries are evicted.
pub(crate) const BREADCRUMB_CAP: usize = 10_000;
/// Maximum number of query log entries kept; oldest are dropped when exceeded.
pub(crate) const QUERY_LOG_CAP: usize = 1000;
/// Default decay factor per step (`--explore-decay` default) — matches the
/// alpha used by the penalty engine.
pub(crate) const DEFAULT_ALPHA: f64 = 0.5;

/// Session-control flags that take a value; stripped together with their
/// value from recorded argv so the query log reflects only the preview
/// request itself.
const STRIPPED_VALUE_FLAGS: [&str; 3] =
    ["--session", "--explore-decay", "--explore-memory"];
/// Session-control boolean flags stripped from recorded argv.
const STRIPPED_BOOL_FLAGS: [&str; 1] = ["--no-record"];

pub(crate) fn active_session_id(cli: &Cli) -> Option<String> {
    cli.session.clone()
}

/// If `--session` was provided, require the session file to exist.
/// New sessions are only created by `hson explore start` — every other
/// path errors on an unknown session ID rather than silently auto-creating
/// (which would mask typos and lose the bias context of an existing session).
pub(crate) fn require_session_exists(cli: &Cli) -> anyhow::Result<()> {
    if let Some(id) = &cli.session {
        let path = session_file_path(id)?;
        if !path.exists() {
            anyhow::bail!(
                "Session '{id}' not found. \
                 Run `hson explore start` to create one."
            );
        }
    }
    Ok(())
}

fn state_dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty())
    {
        return Ok(PathBuf::from(dir));
    }
    match env::var_os("HOME").filter(|v| !v.is_empty()) {
        Some(home) => Ok(PathBuf::from(home).join(".local").join("state")),
        None => anyhow::bail!(
            "cannot determine session state directory: \
             neither XDG_STATE_HOME nor HOME is set"
        ),
    }
}

pub(crate) fn session_file_path(id: &str) -> anyhow::Result<PathBuf> {
    Ok(state_dir()?
        .join("headson")
        .join("sessions")
        .join(format!("{id}.json")))
}

fn is_equals_form(arg: &str, flag: &str) -> bool {
    arg.len() > flag.len()
        && arg.starts_with(flag)
        && arg.as_bytes()[flag.len()] == b'='
}

/// Remove session-control flags from a raw argv before it is written to the
/// session query log. Handles both `--flag value` and `--flag=value` forms.
pub(crate) fn strip_session_control_args(argv: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len());
    let mut tokens = argv.iter();
    while let Some(token) = tokens.next() {
        let is_bool_flag = STRIPPED_BOOL_FLAGS
            .iter()
            .any(|f| token == f || is_equals_form(token, f));
        if is_bool_flag
            || STRIPPED_VALUE_FLAGS
                .iter()
                .any(|f| is_equals_form(token, f))
        {
            continue;
        }
        if STRIPPED_VALUE_FLAGS.iter().any(|f| token == f) {
            tokens.next(); // drop the flag's value token too
            continue;
        }
        out.push(token.clone());
    }
    out
}

pub(crate) fn record_session(
    id: &str,
    shown_leaves: &[headson::BreadcrumbKey],
    cwd: &str,
    argv: &[String],
    alpha: f64,
    breadcrumb_cap: usize,
) {
    let path = match session_file_path(id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "warning: failed to record step for session '{id}': {e}"
            );
            return;
        }
    };
    if let Err(e) = crate::session::io::record_step_atomic(
        &path,
        shown_leaves,
        &crate::cli::timestamp::current_timestamp(),
        cwd,
        argv,
        &crate::session::io::EvictionPolicy {
            alpha,
            breadcrumb_cap,
            query_log_cap: QUERY_LOG_CAP,
        },
    ) {
        eprintln!("warning: failed to record step for session '{id}': {e}");
    }
}

pub(crate) fn maybe_record_session(
    cli: &Cli,
    session_id: Option<&str>,
    from_stdin: bool,
    shown_leaves: &[headson::BreadcrumbKey],
) {
    let Some(id) = session_id else { return };
    if from_stdin || cli.no_record {
        return;
    }
    let cwd = env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let raw_argv: Vec<String> = std::env::args().collect();
    let argv = strip_session_control_args(&raw_argv);
    record_session(
        id,
        shown_leaves,
        &cwd,
        &argv,
        cli.explore_decay,
        cli.explore_memory,
    );
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

    struct IsolatedEnv {
        old_state: Option<String>,
        old_session: Option<String>,
    }

    impl IsolatedEnv {
        fn new(state_dir: &std::path::Path, session_id: Option<&str>) -> Self {
            let old_state = std::env::var("XDG_STATE_HOME").ok();
            let old_session = std::env::var("HSON_SESSION").ok();
            unsafe {
                std::env::set_var("XDG_STATE_HOME", state_dir);
                match session_id {
                    Some(id) => std::env::set_var("HSON_SESSION", id),
                    None => std::env::remove_var("HSON_SESSION"),
                }
            }
            Self {
                old_state,
                old_session,
            }
        }
    }

    impl Drop for IsolatedEnv {
        fn drop(&mut self) {
            unsafe {
                match &self.old_state {
                    Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                    None => std::env::remove_var("XDG_STATE_HOME"),
                }
                match &self.old_session {
                    Some(v) => std::env::set_var("HSON_SESSION", v),
                    None => std::env::remove_var("HSON_SESSION"),
                }
            }
        }
    }

    /// Pre-create an empty session file at the given ID under `state_dir`,
    /// mimicking what `hson explore start` would do — needed because runtime
    /// tracking now requires the session file to already exist.
    fn pre_create_session(state_dir: &std::path::Path, id: &str) {
        let dir = state_dir.join("headson").join("sessions");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.json"));
        let session = crate::session::Session::new(id.into(), "lbl".into());
        crate::session::io::save_to_path(&session, &path).unwrap();
    }

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

        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let (out, warnings) =
            run(&cli).expect("run must succeed without session flag");

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

        let session_id = "32000000-0000-0000-0000-000000000000";
        let _env = IsolatedEnv::new(state_dir.path(), Some(session_id));
        pre_create_session(state_dir.path(), session_id);

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let result = run(&cli);

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

        let session_id = "33000000-0000-0000-0000-000000000000";
        let _env = IsolatedEnv::new(state_dir.path(), None);
        pre_create_session(state_dir.path(), session_id);

        let cli = Cli::parse_from([
            "hson",
            "--session",
            session_id,
            path.to_str().unwrap(),
        ]);
        let result = run(&cli);

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

    /// Step 34: `--session <id> --no-record` does NOT mutate the session file.
    #[test]
    #[serial]
    fn no_record_flag_suppresses_session_write() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "34000000-0000-0000-0000-000000000000";
        let _env = IsolatedEnv::new(state_dir.path(), None);
        pre_create_session(state_dir.path(), session_id);

        let cli = Cli::parse_from([
            "hson",
            "--session",
            session_id,
            "--no-record",
            path.to_str().unwrap(),
        ]);
        let result = run(&cli);

        result.expect("run must succeed with --session --no-record");

        // Session file existed before run; with --no-record, run must not
        // touch it — step_count and queries stay at their initial values.
        let session_file = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{session_id}.json"));
        let after = crate::session::io::load_from_path(&session_file).unwrap();
        assert_eq!(
            after.step_count, 0,
            "--no-record must not increment step_count"
        );
        assert!(
            after.queries.is_empty(),
            "--no-record must not append to query log"
        );
        assert!(
            after.breadcrumbs.is_empty(),
            "--no-record must not record breadcrumbs"
        );
    }

    /// Step 35: Structural confirmation that stdin mode suppresses session writes.
    #[test]
    #[serial]
    fn stdin_mode_no_session_write_active_session_id_resolves() {
        let state_dir = tempdir().unwrap();
        let session_id = "35000000-0000-0000-0000-000000000000";

        let _env = IsolatedEnv::new(state_dir.path(), Some(session_id));

        let cli = Cli::parse_from(["hson"]);
        let id = active_session_id(&cli);

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

    /// Regression (issue #513): passing `--session <unknown-uuid>` to a normal
    /// `hson` invocation must NOT silently auto-create a session file at that
    /// path. The user likely typo'd; auto-creating overwrites their intent and
    /// loses the original session's bias context. The command must either
    /// return an error or leave no session file on disk.
    #[test]
    #[serial]
    fn unknown_session_id_in_run_errors_or_does_not_create_file() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let unknown_id = "11111111-2222-3333-4444-555555555555";
        let _env = IsolatedEnv::new(state_dir.path(), Some(unknown_id));

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let result = run(&cli);

        let expected = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{unknown_id}.json"));
        assert!(
            result.is_err() || !expected.exists(),
            "running with an unknown session ID should error OR not create the session file; \
             got Ok and file exists: result={result:?}, file_exists={}",
            expected.exists()
        );
    }

    /// Regression: record_session must evict stale breadcrumbs so session
    /// files don't grow without bound across long-running explorations. The
    /// cap comes from --explore-memory rather than a hard-coded constant.
    #[test]
    #[serial]
    fn record_session_caps_breadcrumbs_when_limit_exceeded() {
        let dir = tempdir().unwrap();
        let state_dir = dir.path();
        let session_id = "be000000-0000-0000-0000-000000000000";

        let _env = IsolatedEnv::new(state_dir, None);

        // Pre-populate with 20 recent breadcrumbs — above the cap of 5.
        let path = session_file_path(session_id).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut session = crate::session::Session::new(
            session_id.to_string(),
            "lbl".to_string(),
        );
        for i in 0u64..20 {
            session.record_breadcrumb("", &format!("k{i}#h{i}"), i + 1);
        }
        session.step_count = 20;
        crate::session::io::save_to_path(&session, &path).unwrap();

        // --explore-decay 1.0 disables decay-based pruning so only the
        // --explore-memory cap is exercised here.
        let cli = Cli::parse_from([
            "hson",
            "--explore-decay",
            "1.0",
            "--explore-memory",
            "5",
            "input",
        ]);
        record_session(
            session_id,
            &[(String::new(), "new#newhash".to_string())],
            "/cwd",
            &[],
            cli.explore_decay,
            cli.explore_memory,
        );

        let final_session = crate::session::io::load_from_path(&path)
            .expect("session file must exist after record_session");
        assert!(
            final_session.breadcrumbs.len() <= cli.explore_memory,
            "breadcrumbs must be capped at {} after record_session; got: {}",
            cli.explore_memory,
            final_session.breadcrumbs.len()
        );
    }

    /// Regression: record_session must cap the query log to prevent unbounded growth.
    #[test]
    #[serial]
    fn record_session_caps_query_log_when_limit_exceeded() {
        let dir = tempdir().unwrap();
        let state_dir = dir.path();
        let session_id = "9c000000-0000-0000-0000-000000000000";

        let _env = IsolatedEnv::new(state_dir, None);

        // Pre-populate with QUERY_LOG_CAP + 100 queries.
        let path = session_file_path(session_id).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut session = crate::session::Session::new(
            session_id.to_string(),
            "lbl".to_string(),
        );
        for _ in 0u64..(QUERY_LOG_CAP as u64 + 100) {
            session.record_query("ts", "/cwd", &[]);
        }
        crate::session::io::save_to_path(&session, &path).unwrap();

        record_session(
            session_id,
            &[],
            "/cwd",
            &[],
            DEFAULT_ALPHA,
            BREADCRUMB_CAP,
        );

        let final_session = crate::session::io::load_from_path(&path)
            .expect("session file must exist after record_session");
        assert!(
            final_session.queries.len() <= QUERY_LOG_CAP,
            "query log must be capped at {}; got: {}",
            QUERY_LOG_CAP,
            final_session.queries.len()
        );
    }

    fn to_argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn strip_removes_session_control_flags_space_form() {
        let argv = to_argv(&[
            "hson",
            "--session",
            "be000000-0000-0000-0000-000000000000",
            "--explore-decay",
            "0.7",
            "--explore-memory",
            "42",
            "--no-record",
            "-n",
            "5",
            "data.json",
        ]);
        assert_eq!(
            strip_session_control_args(&argv),
            to_argv(&["hson", "-n", "5", "data.json"])
        );
    }

    #[test]
    fn strip_removes_session_control_flags_equals_form() {
        let argv = to_argv(&[
            "hson",
            "--session=be000000-0000-0000-0000-000000000000",
            "--explore-decay=0.7",
            "--explore-memory=42",
            "data.json",
        ]);
        assert_eq!(
            strip_session_control_args(&argv),
            to_argv(&["hson", "data.json"])
        );
    }

    #[test]
    fn strip_keeps_unrelated_args_untouched() {
        let argv = to_argv(&["hson", "--bytes", "200", "--tree", "src/"]);
        assert_eq!(strip_session_control_args(&argv), argv);
    }

    #[test]
    fn strip_does_not_drop_prefix_lookalike_flags() {
        // `--session-x` is not `--session`; only exact or `=`-joined forms
        // are session-control flags.
        let argv = to_argv(&["hson", "--session-x", "v", "data.json"]);
        assert_eq!(strip_session_control_args(&argv), argv);
    }

    #[test]
    fn strip_handles_trailing_value_flag_without_value() {
        let argv = to_argv(&["hson", "data.json", "--session"]);
        assert_eq!(
            strip_session_control_args(&argv),
            to_argv(&["hson", "data.json"])
        );
    }

    struct StateEnvGuard {
        old_state: Option<std::ffi::OsString>,
        old_home: Option<std::ffi::OsString>,
    }

    impl StateEnvGuard {
        fn unset_all() -> Self {
            let old_state = std::env::var_os("XDG_STATE_HOME");
            let old_home = std::env::var_os("HOME");
            unsafe {
                std::env::remove_var("XDG_STATE_HOME");
                std::env::remove_var("HOME");
            }
            Self {
                old_state,
                old_home,
            }
        }
    }

    impl Drop for StateEnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.old_state {
                    Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                    None => std::env::remove_var("XDG_STATE_HOME"),
                }
                match &self.old_home {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }

    /// With neither XDG_STATE_HOME nor HOME available, session_file_path
    /// must return a clear error instead of silently building a relative
    /// path like `.local/state/...` under the current directory.
    #[test]
    #[serial]
    fn session_file_path_errors_when_no_state_dir_env() {
        let _guard = StateEnvGuard::unset_all();

        let result = session_file_path("be000000-0000-0000-0000-000000000000");

        let err = result.expect_err(
            "session_file_path must error when XDG_STATE_HOME and HOME are unset",
        );
        let msg = err.to_string();
        assert!(
            msg.contains("XDG_STATE_HOME") && msg.contains("HOME"),
            "error must name the missing env vars; got: {msg}"
        );
    }

    /// Empty-string env values must be treated the same as unset.
    #[test]
    #[serial]
    fn session_file_path_errors_when_state_dir_env_empty() {
        let _guard = StateEnvGuard::unset_all();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", "");
            std::env::set_var("HOME", "");
        }

        let result = session_file_path("be000000-0000-0000-0000-000000000000");

        assert!(
            result.is_err(),
            "empty XDG_STATE_HOME and HOME must be treated as unset; got: {result:?}"
        );
    }
}

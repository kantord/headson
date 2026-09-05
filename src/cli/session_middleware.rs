use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Context;

use crate::cli::args::Cli;

/// Resolved environment inputs for session/state-dir resolution, captured
/// once per invocation so the rest of the pipeline is pure and testable
/// without mutating real process env vars (which isn't safe across tests
/// running in parallel).
#[derive(Clone, Debug, Default)]
pub(crate) struct SessionEnv {
    pub(crate) hson_session: Option<OsString>,
    pub(crate) xdg_state_home: Option<OsString>,
    pub(crate) home: Option<OsString>,
}

impl SessionEnv {
    pub(crate) fn from_process_env() -> Self {
        Self {
            hson_session: env::var_os("HSON_SESSION"),
            xdg_state_home: env::var_os("XDG_STATE_HOME"),
            home: env::var_os("HOME"),
        }
    }
}

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
const STRIPPED_BOOL_FLAGS: [&str; 2] = ["--no-record", "--explore"];

/// Resolve the active session ID with precedence: an explicit `--session`
/// flag (already UUID-validated by clap) wins over the `HSON_SESSION`
/// environment variable.
pub(crate) fn resolve_session_id(
    cli: &Cli,
    env: &SessionEnv,
) -> anyhow::Result<Option<String>> {
    if let Some(id) = &cli.session {
        return Ok(Some(id.clone()));
    }
    session_id_from_env(env)
}

/// Read `HSON_SESSION`. Empty or whitespace-only values act as unset (so
/// `export HSON_SESSION=""` doesn't break every invocation); any other
/// non-UUID value errors, naming the env var rather than the --session flag.
fn session_id_from_env(env: &SessionEnv) -> anyhow::Result<Option<String>> {
    let Some(raw) = env.hson_session.as_ref() else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    match uuid::Uuid::parse_str(value) {
        Ok(uuid) => Ok(Some(uuid.to_string())),
        Err(e) => anyhow::bail!(
            "invalid HSON_SESSION environment variable {value:?} \
             (must be a UUID): {e}. Unset HSON_SESSION or run \
             `hson explore start` to create a fresh session."
        ),
    }
}

/// Fixed namespace for deriving `--explore`'s implicit per-directory session
/// IDs via UUID v5. Arbitrary but must never change: changing it would
/// silently orphan every implicit session already on disk (each directory
/// would start mapping to a different ID, losing its accumulated novelty
/// bias). Generated once with `uuidgen`; has no meaning beyond being a fixed
/// constant.
const IMPLICIT_SESSION_NAMESPACE: uuid::Uuid = uuid::Uuid::from_bytes([
    0x6a, 0x1e, 0x3f, 0x2c, 0x8b, 0x77, 0x4b, 0x0a, 0x9e, 0x21, 0x5d, 0x3a,
    0x0c, 0xf4, 0x9d, 0x88,
]);

/// Derive `--explore`'s implicit session ID: a UUID v5 hash of the resolved
/// absolute cwd, so repeated invocations in the same directory reuse the
/// same session with nothing to pass around. Deliberately deterministic
/// (not `Uuid::new_v4`, which `hson explore start` uses) so no lookup file
/// or persisted mapping is needed between cwd and session ID.
fn implicit_session_id_for_cwd(cwd: &std::path::Path) -> String {
    uuid::Uuid::new_v5(
        &IMPLICIT_SESSION_NAMESPACE,
        cwd.to_string_lossy().as_bytes(),
    )
    .to_string()
}

/// Resolve the active session, including `--explore`'s implicit per-directory
/// fallback. An explicit `--session`/`HSON_SESSION` always takes precedence
/// and stays strict (per `require_session_exists`: an unknown explicit ID
/// errors rather than auto-creating, to protect against typos). `--explore`
/// alone has no explicit ID to typo, so it derives a deterministic
/// per-directory ID and creates that session's file on first use — the
/// zero-setup counterpart to `hson explore start` + `--session <uuid>`.
pub(crate) fn resolve_or_create_session_id(
    cli: &Cli,
    env: &SessionEnv,
) -> anyhow::Result<Option<String>> {
    if let Some(id) = resolve_session_id(cli, env)? {
        return Ok(Some(id));
    }
    if !cli.explore {
        return Ok(None);
    }
    let cwd = env::current_dir().context(
        "failed to read current directory for --explore's implicit session",
    )?;
    let id = implicit_session_id_for_cwd(&cwd);
    let path = session_file_path(&id, env)?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create session directory {}",
                    parent.display()
                )
            })?;
        }
        let session = crate::session::Session::new(
            id.clone(),
            format!("Implicit --explore session for {}", cwd.display()),
        );
        crate::session::io::save_to_path(&session, &path).map_err(|e| {
            anyhow::anyhow!(
                "failed to create implicit session file for --explore: {e}"
            )
        })?;
    }
    Ok(Some(id))
}

/// If a session is active, require the session file to exist.
/// New sessions are only created by `hson explore start` — every other
/// path errors on an unknown session ID rather than silently auto-creating
/// (which would mask typos and lose the bias context of an existing session).
pub(crate) fn require_session_exists(
    session_id: Option<&str>,
    env: &SessionEnv,
) -> anyhow::Result<()> {
    if let Some(id) = session_id {
        let path = session_file_path(id, env)?;
        if !path.exists() {
            anyhow::bail!(
                "Session '{id}' not found. \
                 Run `hson explore start` to create one."
            );
        }
    }
    Ok(())
}

fn state_dir(env: &SessionEnv) -> anyhow::Result<PathBuf> {
    if let Some(dir) = env.xdg_state_home.as_ref().filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    match env.home.as_ref().filter(|v| !v.is_empty()) {
        Some(home) => Ok(PathBuf::from(home).join(".local").join("state")),
        None => anyhow::bail!(
            "cannot determine session state directory: \
             neither XDG_STATE_HOME nor HOME is set"
        ),
    }
}

pub(crate) fn session_file_path(
    id: &str,
    env: &SessionEnv,
) -> anyhow::Result<PathBuf> {
    Ok(state_dir(env)?
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
    policy: &crate::session::io::EvictionPolicy,
    env: &SessionEnv,
) {
    let path = match session_file_path(id, env) {
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
        policy,
    ) {
        eprintln!("warning: failed to record step for session '{id}': {e}");
    }
}

/// Lossily convert OS-level argv to `String`s. `std::env::args()` PANICS on
/// non-Unicode arguments (e.g. a filename with invalid UTF-8 bytes), which
/// would abort after the preview was already computed; degrading to U+FFFD
/// replacement characters in the query log is the right trade-off.
fn argv_to_string_lossy(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Vec<String> {
    args.into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

pub(crate) fn maybe_record_session(
    cli: &Cli,
    session_id: Option<&str>,
    from_stdin: bool,
    shown_leaves: &[headson::BreadcrumbKey],
    env: &SessionEnv,
) {
    let Some(id) = session_id else { return };
    if from_stdin || cli.no_record {
        return;
    }
    let cwd = env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let raw_argv = argv_to_string_lossy(env::args_os());
    let argv = strip_session_control_args(&raw_argv);
    record_session(
        id,
        shown_leaves,
        &cwd,
        &argv,
        &crate::session::io::EvictionPolicy {
            alpha: cli.explore_decay,
            breadcrumb_cap: cli.explore_memory,
            query_log_cap: QUERY_LOG_CAP,
        },
        env,
    );
}

#[cfg(test)]
mod tests {
    use std::fs;

    use clap::Parser;
    use tempfile::tempdir;

    use super::*;
    use crate::cli::args::Cli;
    use crate::cli::run::run_with_env;

    /// Build a `SessionEnv` for tests: an explicit state dir plus an optional
    /// `HSON_SESSION` value, with no real process env involved.
    fn env_with(
        state_dir: &std::path::Path,
        session_id: Option<&str>,
    ) -> SessionEnv {
        SessionEnv {
            hson_session: session_id.map(OsString::from),
            xdg_state_home: Some(OsString::from(state_dir)),
            home: None,
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

    /// Build a minimal Cli with `--explore` set (or not) and no inputs.
    fn make_explore_cli(explore: bool) -> Cli {
        let mut args = vec!["hson"];
        if explore {
            args.push("--explore");
        }
        Cli::parse_from(args)
    }

    /// `--explore` alone (no `--session`/`HSON_SESSION`) must auto-create an
    /// implicit session file rather than returning `None`.
    #[test]
    fn explore_flag_alone_creates_implicit_session() {
        let state_dir = tempdir().unwrap();
        let env = env_with(state_dir.path(), None);
        let cli = make_explore_cli(true);

        let id = resolve_or_create_session_id(&cli, &env)
            .expect("must succeed")
            .expect("--explore alone must yield a session id");

        let path = session_file_path(&id, &env).unwrap();
        assert!(
            path.exists(),
            "--explore must create the implicit session file at {path:?}"
        );
    }

    /// Two invocations of `--explore` from the same directory (same `env`,
    /// no chdir between calls) must resolve to the same session id, so
    /// novelty bias accumulates instead of resetting every call.
    #[test]
    fn explore_flag_reuses_same_implicit_session_across_calls() {
        let state_dir = tempdir().unwrap();
        let env = env_with(state_dir.path(), None);
        let cli = make_explore_cli(true);

        let id1 = resolve_or_create_session_id(&cli, &env)
            .unwrap()
            .expect("first call must yield a session id");
        let id2 = resolve_or_create_session_id(&cli, &env)
            .unwrap()
            .expect("second call must yield a session id");

        assert_eq!(
            id1, id2,
            "repeated --explore calls from the same directory must reuse \
             the same implicit session id"
        );
    }

    /// Without `--explore` and without any explicit session, resolution must
    /// stay `None` — `--explore` must not become the default.
    #[test]
    fn no_explore_flag_and_no_explicit_session_resolves_to_none() {
        let state_dir = tempdir().unwrap();
        let env = env_with(state_dir.path(), None);
        let cli = make_explore_cli(false);

        let id = resolve_or_create_session_id(&cli, &env).unwrap();
        assert!(
            id.is_none(),
            "no session must be active without --explore or an explicit \
             session; got: {id:?}"
        );
    }

    /// An explicit `--session` must take precedence over `--explore` when
    /// both are given, and must NOT trigger implicit-session auto-creation
    /// semantics (the explicit id still must already exist elsewhere).
    #[test]
    fn explicit_session_wins_over_explore_flag() {
        let state_dir = tempdir().unwrap();
        let session_id = "99999999-9999-9999-9999-999999999999";
        pre_create_session(state_dir.path(), session_id);
        let env = env_with(state_dir.path(), None);

        let cli =
            Cli::parse_from(["hson", "--explore", "--session", session_id]);

        let id = resolve_or_create_session_id(&cli, &env)
            .unwrap()
            .expect("must resolve to the explicit session");
        assert_eq!(
            id, session_id,
            "--session must take precedence over --explore"
        );
    }

    /// `HSON_SESSION` must also take precedence over `--explore`, matching
    /// `--session`'s precedence.
    #[test]
    fn hson_session_env_wins_over_explore_flag() {
        let state_dir = tempdir().unwrap();
        let session_id = "88888888-8888-8888-8888-888888888888";
        pre_create_session(state_dir.path(), session_id);
        let env = env_with(state_dir.path(), Some(session_id));
        let cli = make_explore_cli(true);

        let id = resolve_or_create_session_id(&cli, &env)
            .unwrap()
            .expect("must resolve to the HSON_SESSION session");
        assert_eq!(
            id, session_id,
            "HSON_SESSION must take precedence over --explore"
        );
    }

    /// Step 31: When neither HSON_SESSION nor --session is set, running hson on
    /// a file produces the same output as a baseline run and does NOT write a
    /// session file anywhere under XDG_STATE_HOME.
    #[test]
    fn no_hson_session_env_output_unchanged() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let env = env_with(state_dir.path(), None);

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let (out, warnings) = run_with_env(&cli, &env)
            .expect("run must succeed without session flag");

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
    fn hson_session_env_creates_session_file() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "32000000-0000-0000-0000-000000000000";
        let env = env_with(state_dir.path(), Some(session_id));
        pre_create_session(state_dir.path(), session_id);

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let result = run_with_env(&cli, &env);

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

    /// Recorded breadcrumbs identify the input file by its resolved absolute
    /// path; the path component is the in-file dot-path plus content hash,
    /// with no filename embedded (issue #513 review).
    #[test]
    fn recorded_breadcrumbs_carry_absolute_file_and_inner_path() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "3b000000-0000-0000-0000-000000000000";
        let env = env_with(state_dir.path(), Some(session_id));
        pre_create_session(state_dir.path(), session_id);

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        run_with_env(&cli, &env).expect("run must succeed");

        let session_file = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{session_id}.json"));
        let session =
            crate::session::io::load_from_path(&session_file).unwrap();
        let expected_file = path.canonicalize().unwrap();
        assert!(!session.breadcrumbs.is_empty(), "must record breadcrumbs");
        for crumb in &session.breadcrumbs {
            assert_eq!(
                crumb.file,
                expected_file.to_string_lossy(),
                "breadcrumb file must be the resolved absolute input path"
            );
            assert!(
                crumb.path.starts_with("x#"),
                "path must be the in-file dot-path plus hash, with no \
                 filename embedded; got: {:?}",
                crumb.path
            );
        }
    }

    /// Step 33: Using `--session <id>` creates the session file.
    #[test]
    fn session_flag_creates_session_file() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "33000000-0000-0000-0000-000000000000";
        let env = env_with(state_dir.path(), None);
        pre_create_session(state_dir.path(), session_id);

        let cli = Cli::parse_from([
            "hson",
            "--session",
            session_id,
            path.to_str().unwrap(),
        ]);
        let result = run_with_env(&cli, &env);

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
    fn no_record_flag_suppresses_session_write() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let session_id = "34000000-0000-0000-0000-000000000000";
        let env = env_with(state_dir.path(), None);
        pre_create_session(state_dir.path(), session_id);

        let cli = Cli::parse_from([
            "hson",
            "--session",
            session_id,
            "--no-record",
            path.to_str().unwrap(),
        ]);
        let result = run_with_env(&cli, &env);

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
    fn stdin_mode_no_session_write_active_session_id_resolves() {
        let state_dir = tempdir().unwrap();
        let session_id = "35000000-0000-0000-0000-000000000000";

        let env = env_with(state_dir.path(), Some(session_id));

        let cli = Cli::parse_from(["hson"]);
        let id = resolve_session_id(&cli, &env)
            .expect("valid HSON_SESSION must resolve without error");

        assert_eq!(
            id,
            Some(session_id.to_string()),
            "resolve_session_id must return Some(id) from HSON_SESSION env var"
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
    /// loses the original session's bias context. The command must return an
    /// error AND leave no session file on disk.
    #[test]
    fn unknown_session_id_in_run_errors_and_does_not_create_file() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let unknown_id = "11111111-2222-3333-4444-555555555555";
        let env = env_with(state_dir.path(), Some(unknown_id));

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let result = run_with_env(&cli, &env);

        let expected = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{unknown_id}.json"));
        assert!(
            result.is_err(),
            "running with an unknown session ID must error; got: {result:?}"
        );
        assert!(
            !expected.exists(),
            "running with an unknown session ID must not create the session \
             file at {expected:?}"
        );
    }

    /// An EMPTY exported HSON_SESSION must behave exactly as if it were
    /// unset: the run succeeds and no session file is written.
    #[test]
    fn empty_hson_session_env_is_treated_as_unset() {
        let dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, r#"{"x": 1}"#).unwrap();

        let env = env_with(state_dir.path(), Some(""));

        let cli = Cli::parse_from(["hson", path.to_str().unwrap()]);
        let id = resolve_session_id(&cli, &env)
            .expect("empty HSON_SESSION must not be an error");
        assert_eq!(id, None, "empty HSON_SESSION must resolve to no session");

        let (out, _) = run_with_env(&cli, &env).expect("run must succeed");
        assert!(!out.is_empty(), "output must be non-empty");
    }

    /// Whitespace-only HSON_SESSION is also treated as unset.
    #[test]
    fn whitespace_hson_session_env_is_treated_as_unset() {
        let state_dir = tempdir().unwrap();
        let env = env_with(state_dir.path(), Some("  \t "));

        let cli = Cli::parse_from(["hson"]);
        let id = resolve_session_id(&cli, &env)
            .expect("whitespace-only HSON_SESSION must not be an error");
        assert_eq!(id, None);
    }

    /// A non-empty, non-UUID HSON_SESSION must produce a clear error that
    /// names HSON_SESSION (not the --session flag the user never typed).
    #[test]
    fn invalid_hson_session_env_errors_naming_the_env_var() {
        let state_dir = tempdir().unwrap();
        let env = env_with(state_dir.path(), Some("not-a-uuid"));

        let cli = Cli::parse_from(["hson"]);
        let err = resolve_session_id(&cli, &env)
            .expect_err("non-UUID HSON_SESSION must be an error");

        let msg = err.to_string();
        assert!(
            msg.contains("HSON_SESSION"),
            "error must name HSON_SESSION; got: {msg}"
        );
        assert!(
            !msg.contains("--session"),
            "error must not blame the --session flag; got: {msg}"
        );
    }

    /// An explicit --session flag takes precedence over HSON_SESSION, even
    /// when the env value is garbage.
    #[test]
    fn session_flag_takes_precedence_over_env() {
        let state_dir = tempdir().unwrap();
        let env = env_with(state_dir.path(), Some("not-a-uuid"));

        let flag_id = "ab000000-0000-0000-0000-000000000000";
        let cli = Cli::parse_from(["hson", "--session", flag_id]);
        let id = resolve_session_id(&cli, &env)
            .expect("--session flag must win over an invalid env value");
        assert_eq!(id.as_deref(), Some(flag_id));
    }

    /// Non-UTF-8 argv must be lossily converted, never panic — a panic here
    /// would discard the already-rendered preview (issue #513 review).
    #[test]
    #[cfg(unix)]
    fn argv_to_string_lossy_replaces_invalid_utf8() {
        use std::os::unix::ffi::OsStringExt;

        let bad = std::ffi::OsString::from_vec(b"bad\xff.json".to_vec());
        let argv =
            argv_to_string_lossy(vec![std::ffi::OsString::from("hson"), bad]);

        assert_eq!(argv[0], "hson");
        assert_eq!(
            argv[1], "bad\u{FFFD}.json",
            "invalid UTF-8 bytes must degrade to U+FFFD, not panic"
        );
    }

    /// Regression: record_session must evict stale breadcrumbs so session
    /// files don't grow without bound across long-running explorations. The
    /// cap comes from --explore-memory rather than a hard-coded constant.
    #[test]
    fn record_session_caps_breadcrumbs_when_limit_exceeded() {
        let dir = tempdir().unwrap();
        let state_dir = dir.path();
        let session_id = "be000000-0000-0000-0000-000000000000";

        let env = env_with(state_dir, None);

        // Pre-populate with 20 recent breadcrumbs — above the cap of 5.
        let path = session_file_path(session_id, &env).unwrap();
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
            &crate::session::io::EvictionPolicy {
                alpha: cli.explore_decay,
                breadcrumb_cap: cli.explore_memory,
                query_log_cap: QUERY_LOG_CAP,
            },
            &env,
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
    fn record_session_caps_query_log_when_limit_exceeded() {
        let dir = tempdir().unwrap();
        let state_dir = dir.path();
        let session_id = "9c000000-0000-0000-0000-000000000000";

        let env = env_with(state_dir, None);

        // Pre-populate with QUERY_LOG_CAP + 100 queries.
        let path = session_file_path(session_id, &env).unwrap();
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
            &crate::session::io::EvictionPolicy {
                alpha: DEFAULT_ALPHA,
                breadcrumb_cap: BREADCRUMB_CAP,
                query_log_cap: QUERY_LOG_CAP,
            },
            &env,
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

    /// With neither XDG_STATE_HOME nor HOME available, session_file_path
    /// must return a clear error instead of silently building a relative
    /// path like `.local/state/...` under the current directory.
    #[test]
    fn session_file_path_errors_when_no_state_dir_env() {
        let env = SessionEnv::default();

        let result =
            session_file_path("be000000-0000-0000-0000-000000000000", &env);

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
    fn session_file_path_errors_when_state_dir_env_empty() {
        let env = SessionEnv {
            hson_session: None,
            xdg_state_home: Some(OsString::from("")),
            home: Some(OsString::from("")),
        };

        let result =
            session_file_path("be000000-0000-0000-0000-000000000000", &env);

        assert!(
            result.is_err(),
            "empty XDG_STATE_HOME and HOME must be treated as unset; got: {result:?}"
        );
    }
}

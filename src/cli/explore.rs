use anyhow::Result;
use uuid::Uuid;

use crate::cli::args::{Cli, ExploreSubcommand};
use crate::cli::session_middleware::{
    active_session_id, require_session_exists, session_file_path,
};

const NO_SESSION_MSG: &str =
    "No active session. Run `hson explore start` to begin.";

fn load_active_session(
    cli: &Cli,
) -> Option<(String, crate::session::Session)> {
    let id = active_session_id(cli)?;
    let path = session_file_path(&id);
    // After `require_session_exists`, the file is guaranteed to exist on every
    // path that calls this. A read failure here means the file became corrupt
    // or unreadable between the two calls — surface that as "no session".
    let session = crate::session::io::load_from_path(&path).ok()?;
    Some((id, session))
}

/// Format argv for display: relativize paths that are under `cwd`, and
/// for argv[0] (the binary) use just the filename when not under `cwd`.
fn display_argv_relative(argv: &[String], cwd: &str) -> String {
    let cwd_path = std::path::Path::new(cwd);
    argv.iter()
        .enumerate()
        .map(|(i, arg)| {
            let p = std::path::Path::new(arg);
            if let Ok(rel) = p.strip_prefix(cwd_path) {
                rel.to_string_lossy().into_owned()
            } else if i == 0 {
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| arg.clone())
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn run_subcommand(
    cmd: &ExploreSubcommand,
    cli: &Cli,
) -> Result<String> {
    match cmd {
        ExploreSubcommand::Start { label } => {
            let id = Uuid::new_v4().to_string();
            let path = session_file_path(&id);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let cwd = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let resolved_label = match label {
                Some(l) => l.clone(),
                None => format!("Explore session started originally in {cwd}"),
            };
            let session =
                crate::session::Session::new(id.clone(), resolved_label);
            crate::session::io::save_to_path(&session, &path).map_err(
                |e| anyhow::anyhow!("failed to create session file: {e}"),
            )?;
            Ok(id)
        }
        ExploreSubcommand::Status => {
            require_session_exists(cli)?;
            let Some((_, session)) = load_active_session(cli) else {
                return Ok(NO_SESSION_MSG.to_string());
            };
            let last_active = session
                .queries
                .last()
                .map_or("never", |q| q.timestamp.as_str());
            Ok(format!(
                "Session:     {}\n\
                 Label:       {}\n\
                 Steps:       {}\n\
                 Breadcrumbs: {}\n\
                 Last active: {}",
                session.id,
                session.label,
                session.step_count,
                session.breadcrumbs.len(),
                last_active,
            ))
        }
        ExploreSubcommand::Clear => {
            require_session_exists(cli)?;
            let Some((id, mut session)) = load_active_session(cli) else {
                return Ok(NO_SESSION_MSG.to_string());
            };
            let path = session_file_path(&id);
            session.clear();
            crate::session::io::save_to_path(&session, &path)
                .map_err(|e| anyhow::anyhow!("failed to save session: {e}"))?;
            Ok(String::new())
        }
        ExploreSubcommand::List => {
            require_session_exists(cli)?;
            let Some((_, session)) = load_active_session(cli) else {
                return Ok(NO_SESSION_MSG.to_string());
            };
            let lines: Vec<String> = session
                .queries
                .iter()
                .map(|q| {
                    let display_argv = display_argv_relative(&q.argv, &q.cwd);
                    format!("[{}] {} {}", q.timestamp, q.cwd, display_argv)
                })
                .collect();
            Ok(lines.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::ExploreSubcommand;
    use crate::session::io::{load_from_path, save_to_path};
    use crate::session::{QueryEntry, Session};
    use clap::Parser;
    use serial_test::serial;
    use tempfile::tempdir;

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

    /// Build a minimal Cli with an optional --session value and no inputs.
    /// XDG_STATE_HOME must already be set before this is called.
    fn make_cli(session_id: Option<&str>) -> Cli {
        let mut args = vec!["hson"];
        if let Some(id) = session_id {
            args.push("--session");
            args.push(id);
        }
        Cli::parse_from(args)
    }

    /// Step 36: `explore start` with no label returns a bare UUID string — no
    /// trailing whitespace or newline, matches the canonical UUID pattern.
    #[test]
    #[serial]
    fn explore_start_returns_uuid_string() {
        let state_dir = tempdir().unwrap();
        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cli = make_cli(None);
        let result =
            run_subcommand(&ExploreSubcommand::Start { label: None }, &cli);

        let output = result.expect("run_subcommand(Start) must succeed");
        let uuid_re = regex::Regex::new(
            r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
        )
        .unwrap();
        assert!(
            uuid_re.is_match(output.trim()),
            "expected UUID string (no trailing whitespace/newline), got: {output:?}"
        );
        assert_eq!(
            output,
            output.trim(),
            "output must have no leading/trailing whitespace; got: {output:?}"
        );
    }

    /// Step 37: `explore start --label "my label"` stores `label = Some("my label")`
    /// in the session file on disk.
    #[test]
    #[serial]
    fn explore_start_with_label_stores_label_in_session_file() {
        let state_dir = tempdir().unwrap();
        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cli = make_cli(None);
        let result = run_subcommand(
            &ExploreSubcommand::Start {
                label: Some("my label".to_string()),
            },
            &cli,
        );

        let output = result.expect("run_subcommand(Start) must succeed");

        // Load the session file that was just created
        let session_path = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{output}.json"));

        assert!(
            session_path.exists(),
            "session file must exist at {session_path:?}"
        );
        let session = load_from_path(&session_path)
            .expect("session file must be valid JSON");
        assert_eq!(
            session.label, "my label",
            "session.label must be 'my label'; got: {:?}",
            session.label
        );
    }

    /// Step 38: `explore start` with no label stores a label that contains the
    /// last path component of the current working directory.
    #[test]
    #[serial]
    fn explore_start_no_label_stores_cwd_derived_label() {
        let state_dir = tempdir().unwrap();
        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cwd = std::env::current_dir()
            .expect("must be able to get cwd")
            .to_string_lossy()
            .into_owned();
        let last_component = std::path::Path::new(&cwd)
            .file_name()
            .expect("cwd must have a last component")
            .to_string_lossy()
            .into_owned();

        let cli = make_cli(None);
        let result =
            run_subcommand(&ExploreSubcommand::Start { label: None }, &cli);

        let output = result.expect("run_subcommand(Start) must succeed");

        let session_path = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{output}.json"));

        assert!(
            session_path.exists(),
            "session file must exist at {session_path:?}"
        );
        let session = load_from_path(&session_path)
            .expect("session file must be valid JSON");
        assert!(
            session.label.contains(&last_component),
            "session.label must contain last cwd component '{last_component}'; got: {:?}",
            session.label
        );
    }

    /// Step 39: `explore status` with a session containing step_count=3 returns
    /// a string that mentions the step count ("3") and the session ID.
    #[test]
    #[serial]
    fn explore_status_shows_step_count_and_uuid() {
        let state_dir = tempdir().unwrap();
        let session_id = "39000000-0000-0000-0000-000000000000";

        // Write a session file with step_count=3
        let sessions_dir = state_dir.path().join("headson").join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let session_path = sessions_dir.join(format!("{session_id}.json"));
        let mut session =
            Session::new(session_id.to_string(), "test label".to_string());
        session.step_count = 3;
        save_to_path(&session, &session_path).unwrap();

        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cli = make_cli(Some(session_id));
        let result = run_subcommand(&ExploreSubcommand::Status, &cli);

        let output = result.expect("run_subcommand(Status) must succeed");
        assert!(
            output.contains("3"),
            "status output must contain the step count '3'; got: {output:?}"
        );
        assert!(
            output.contains(session_id),
            "status output must contain the session ID '{session_id}'; got: {output:?}"
        );
    }

    /// Step 40: `explore status` with no active session returns Ok and a helpful
    /// message that includes "start" (suggesting `hson explore start`).
    #[test]
    #[serial]
    fn explore_status_no_session_prints_helpful_message() {
        let state_dir = tempdir().unwrap();
        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cli = make_cli(None);
        let result = run_subcommand(&ExploreSubcommand::Status, &cli);

        let output = result.expect(
            "run_subcommand(Status) with no session must return Ok, not Err",
        );
        assert!(
            output.contains("start"),
            "no-session status output must contain 'start' (suggesting hson explore start); got: {output:?}"
        );
    }

    /// Step 41: `explore clear` zeroes breadcrumbs, step_count, and queries.
    #[test]
    #[serial]
    fn explore_clear_zeroes_breadcrumbs_step_count_and_queries() {
        let state_dir = tempdir().unwrap();
        let session_id = "41000000-0000-0000-0000-000000000000";

        // Create a session with breadcrumbs and a query
        let sessions_dir = state_dir.path().join("headson").join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let session_path = sessions_dir.join(format!("{session_id}.json"));
        let mut session =
            Session::new(session_id.to_string(), "clear test".to_string());
        session.record_breadcrumb("a.json", "users.0", 1);
        session.record_breadcrumb("b.json", "items.1", 2);
        session.queries.push(QueryEntry {
            step: 1,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            cwd: "/home/user/project".to_string(),
            argv: vec!["src/".to_string()],
        });
        save_to_path(&session, &session_path).unwrap();

        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cli = make_cli(Some(session_id));
        let result = run_subcommand(&ExploreSubcommand::Clear, &cli);

        result.expect("run_subcommand(Clear) must succeed");

        // Reload and verify
        let updated = load_from_path(&session_path)
            .expect("session file must still be valid after clear");
        assert!(
            updated.breadcrumbs.is_empty(),
            "breadcrumbs must be empty after clear; got: {:?}",
            updated.breadcrumbs
        );
        assert_eq!(
            updated.step_count, 0,
            "step_count must be 0 after clear; got: {}",
            updated.step_count
        );
        assert!(
            updated.queries.is_empty(),
            "queries must be cleared after clear; got: {:?}",
            updated.queries
        );
    }

    /// Regression (issue #513): `explore status` with an unknown session ID
    /// must error (e.g. "Session '<id>' not found. Run `hson explore start`
    /// to create one.") instead of silently treating the unknown ID as a
    /// brand-new empty session.
    #[test]
    #[serial]
    fn unknown_session_id_in_explore_status_errors() {
        let state_dir = tempdir().unwrap();

        let unknown_id = "22222222-3333-4444-5555-666666666666";
        let _env = IsolatedEnv::new(state_dir.path(), Some(unknown_id));

        let cli = make_cli(None);
        let result = run_subcommand(&ExploreSubcommand::Status, &cli);

        assert!(
            result.is_err(),
            "explore status with an unknown session ID must return Err; \
             today it silently fabricates an empty session and returns Ok. \
             got: {result:?}"
        );
    }

    /// Regression (issue #513): `explore list` with an unknown session ID
    /// must error instead of silently treating the unknown ID as a brand-new
    /// empty session and returning an empty query list.
    #[test]
    #[serial]
    fn unknown_session_id_in_explore_list_errors() {
        let state_dir = tempdir().unwrap();

        let unknown_id = "33333333-4444-5555-6666-777777777777";
        let _env = IsolatedEnv::new(state_dir.path(), Some(unknown_id));

        let cli = make_cli(None);
        let result = run_subcommand(&ExploreSubcommand::List, &cli);

        assert!(
            result.is_err(),
            "explore list with an unknown session ID must return Err; \
             today it silently fabricates an empty session and returns Ok. \
             got: {result:?}"
        );
    }

    /// Regression (issue #513): `explore clear` with an unknown session ID
    /// must error instead of silently materializing a session file at that
    /// path. The user likely typo'd; auto-creating overwrites their intent.
    #[test]
    #[serial]
    fn unknown_session_id_in_explore_clear_errors() {
        let state_dir = tempdir().unwrap();
        // Pre-create the sessions directory so save_to_path won't fail for an
        // unrelated reason (missing parent dir). We want the test to exercise
        // the unknown-ID semantic, not a filesystem-error red herring.
        let sessions_dir = state_dir.path().join("headson").join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let unknown_id = "44444444-5555-6666-7777-888888888888";
        let _env = IsolatedEnv::new(state_dir.path(), Some(unknown_id));

        let cli = make_cli(None);
        let result = run_subcommand(&ExploreSubcommand::Clear, &cli);

        let expected = sessions_dir.join(format!("{unknown_id}.json"));
        assert!(
            result.is_err() || !expected.exists(),
            "explore clear with an unknown session ID must error OR not create the session file; \
             today it silently fabricates an empty session and writes it to disk. \
             got: result={result:?}, file_exists={}",
            expected.exists()
        );
    }

    /// Step 42: `explore list` returns all recorded queries in chronological
    /// order (ascending timestamp / step order).
    #[test]
    #[serial]
    fn explore_list_prints_queries_chronologically() {
        let state_dir = tempdir().unwrap();
        let session_id = "42000000-0000-0000-0000-000000000000";

        // Create a session with 3 queries at distinct timestamps
        let sessions_dir = state_dir.path().join("headson").join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let session_path = sessions_dir.join(format!("{session_id}.json"));
        let mut session =
            Session::new(session_id.to_string(), "list test".to_string());
        session.queries.push(QueryEntry {
            step: 1,
            timestamp: "2026-01-01T10:00:00Z".to_string(),
            cwd: "/home/user/alpha".to_string(),
            argv: vec![],
        });
        session.queries.push(QueryEntry {
            step: 2,
            timestamp: "2026-01-01T11:00:00Z".to_string(),
            cwd: "/home/user/beta".to_string(),
            argv: vec![],
        });
        session.queries.push(QueryEntry {
            step: 3,
            timestamp: "2026-01-01T12:00:00Z".to_string(),
            cwd: "/home/user/gamma".to_string(),
            argv: vec![],
        });
        save_to_path(&session, &session_path).unwrap();

        let _env = IsolatedEnv::new(state_dir.path(), None);

        let cli = make_cli(Some(session_id));
        let result = run_subcommand(&ExploreSubcommand::List, &cli);

        let output = result.expect("run_subcommand(List) must succeed");

        // All 3 cwd values must appear
        assert!(
            output.contains("/home/user/alpha"),
            "output must contain '/home/user/alpha'; got: {output:?}"
        );
        assert!(
            output.contains("/home/user/beta"),
            "output must contain '/home/user/beta'; got: {output:?}"
        );
        assert!(
            output.contains("/home/user/gamma"),
            "output must contain '/home/user/gamma'; got: {output:?}"
        );

        // Chronological order: alpha before beta before gamma
        let pos_alpha = output
            .find("/home/user/alpha")
            .expect("alpha must be in output");
        let pos_beta = output
            .find("/home/user/beta")
            .expect("beta must be in output");
        let pos_gamma = output
            .find("/home/user/gamma")
            .expect("gamma must be in output");
        assert!(
            pos_alpha < pos_beta,
            "alpha (step 1) must appear before beta (step 2) in output; alpha@{pos_alpha}, beta@{pos_beta}"
        );
        assert!(
            pos_beta < pos_gamma,
            "beta (step 2) must appear before gamma (step 3) in output; beta@{pos_beta}, gamma@{pos_gamma}"
        );
    }

    /// Issue #513: `explore status` must surface the number of breadcrumbs in
    /// the active session so users can gauge how much novelty bias has built
    /// up. The existing output only shows Session/Label/Steps and hides this.
    #[test]
    #[serial]
    fn explore_status_shows_breadcrumb_count() {
        let state_dir = tempdir().unwrap();
        let session_id = "33333333-3333-3333-3333-333333333333";

        let path = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{session_id}.json"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut session =
            Session::new(session_id.to_string(), "lbl".to_string());
        session.record_breadcrumb("a.json", "x", 1);
        session.record_breadcrumb("b.json", "y", 2);
        session.record_breadcrumb("c.json", "z", 3);
        session.step_count = 3;
        save_to_path(&session, &path).unwrap();

        let _env = IsolatedEnv::new(state_dir.path(), Some(session_id));

        let cli = make_cli(Some(session_id));
        let out = run_subcommand(&ExploreSubcommand::Status, &cli)
            .expect("run_subcommand(Status) must succeed");

        assert!(
            out.contains("Breadcrumbs:") || out.contains("breadcrumbs:"),
            "status output must include a Breadcrumbs line; got:\n{out}"
        );
        assert!(
            out.contains('3'),
            "status output must include the breadcrumb count (3); got:\n{out}"
        );
    }

    /// Issue #513: `explore status` must surface the timestamp of the most
    /// recent recorded query so users can tell when a session was last active.
    #[test]
    #[serial]
    fn explore_status_shows_last_active_timestamp() {
        let state_dir = tempdir().unwrap();
        let session_id = "44444444-4444-4444-4444-444444444444";

        let path = state_dir
            .path()
            .join("headson")
            .join("sessions")
            .join(format!("{session_id}.json"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut session =
            Session::new(session_id.to_string(), "lbl".to_string());
        session.record_query("2026-05-08T12:34:56Z", "/cwd", &[]);
        save_to_path(&session, &path).unwrap();

        let _env = IsolatedEnv::new(state_dir.path(), Some(session_id));

        let cli = make_cli(Some(session_id));
        let out = run_subcommand(&ExploreSubcommand::Status, &cli)
            .expect("run_subcommand(Status) must succeed");

        assert!(
            out.contains("Last active") || out.contains("last active"),
            "status output must include a Last active line; got:\n{out}"
        );
        assert!(
            out.contains("2026-05-08T12:34:56Z"),
            "status output must include the most recent query timestamp; got:\n{out}"
        );
    }
}

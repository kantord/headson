use std::fs;
use std::io;
use std::path::Path;

use super::types::Session;

pub fn save_to_path(session: &Session, path: &Path) -> Result<(), io::Error> {
    let json = serde_json::to_string(session)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    // Write to a .tmp file first, then atomically rename to final path.
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &json)?;
    fs::rename(&tmp_path, path)
}

pub fn load_from_path(path: &Path) -> Result<Session, io::Error> {
    let data = fs::read_to_string(path)?;
    serde_json::from_str(&data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "used only in session/io tests")
)]
#[allow(
    clippy::cognitive_complexity,
    reason = "merge logic touches multiple parallel collections; splitting would obscure the invariant"
)]
pub fn save_merged_to_path(
    new_session: &Session,
    path: &Path,
) -> Result<(), io::Error> {
    // Re-read the on-disk session (or start fresh if missing/unreadable)
    let mut base = if path.exists() {
        load_from_path(path).unwrap_or_else(|_| {
            Session::new(new_session.id.clone(), new_session.label.clone())
        })
    } else {
        Session::new(new_session.id.clone(), new_session.label.clone())
    };

    // Upsert breadcrumbs: new_session's entries win on conflict
    for bc in &new_session.breadcrumbs {
        if let Some(existing) = base
            .breadcrumbs
            .iter_mut()
            .find(|b| b.file == bc.file && b.path == bc.path)
        {
            *existing = bc.clone();
        } else {
            base.breadcrumbs.push(bc.clone());
        }
    }

    // Append queries, deduplicating by step to avoid double-counting when
    // new_session was derived from the same base as what's on disk.
    let existing_steps: std::collections::HashSet<u64> =
        base.queries.iter().map(|q| q.step).collect();
    for q in &new_session.queries {
        if !existing_steps.contains(&q.step) {
            base.queries.push(q.clone());
        }
    }

    save_to_path(&base, path)
}

/// Merge, evict, and cap in one atomic operation.
///
/// 1. Re-read on-disk state to pick up concurrent writes.
/// 2. Upsert breadcrumbs from `new_session` (new_session wins on conflict).
/// 3. Append queries from `new_session`, deduplicating by step.
/// 4. Apply `Session::evict` on the merged result.
/// 5. Truncate the query log to `query_log_cap` most-recent entries.
/// 6. Atomic-rename write the final state.
#[allow(
    clippy::cognitive_complexity,
    reason = "merge + evict + cap steps all touch the same mutable state; splitting would require passing it around"
)]
pub fn save_merged_with_eviction_to_path(
    new_session: &Session,
    path: &Path,
    alpha: f64,
    breadcrumb_cap: usize,
    query_log_cap: usize,
) -> Result<(), io::Error> {
    let mut base = if path.exists() {
        load_from_path(path).unwrap_or_else(|_| {
            Session::new(new_session.id.clone(), new_session.label.clone())
        })
    } else {
        Session::new(new_session.id.clone(), new_session.label.clone())
    };

    for bc in &new_session.breadcrumbs {
        if let Some(existing) = base
            .breadcrumbs
            .iter_mut()
            .find(|b| b.file == bc.file && b.path == bc.path)
        {
            *existing = bc.clone();
        } else {
            base.breadcrumbs.push(bc.clone());
        }
    }

    let existing_steps: std::collections::HashSet<u64> =
        base.queries.iter().map(|q| q.step).collect();
    for q in &new_session.queries {
        if !existing_steps.contains(&q.step) {
            base.queries.push(q.clone());
        }
    }

    base.step_count = new_session.step_count;
    base.evict(new_session.step_count, alpha, breadcrumb_cap);

    if base.queries.len() > query_log_cap {
        let excess = base.queries.len() - query_log_cap;
        base.queries.drain(0..excess);
    }

    save_to_path(&base, path)
}

pub fn load_or_create(
    path: &Path,
    id: &str,
    label: Option<&str>,
    cwd: &str,
) -> Session {
    if path.exists() {
        if let Ok(session) = load_from_path(path) {
            return session;
        }
    }
    match label {
        Some(lbl) => Session::new(id.to_string(), lbl.to_string()),
        None => Session::new_with_cwd(id.to_string(), cwd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{QueryEntry, Session};
    use headson::Breadcrumb;
    use tempfile::tempdir;

    #[test]
    fn save_merged_preserves_breadcrumbs_from_both_writers() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        // Writer 1: save a session with breadcrumb ("a.json", "x")
        let mut session1 = Session::new("sid".to_string(), "lbl".to_string());
        session1.record_breadcrumb("a.json", "x", 1);
        save_to_path(&session1, &path).unwrap();

        // Writer 2: load the session, add breadcrumb ("b.json", "y"), then merge-write
        let mut session2 = load_from_path(&path).unwrap();
        session2.record_breadcrumb("b.json", "y", 2);
        save_merged_to_path(&session2, &path).unwrap();

        // On-disk session must contain both breadcrumbs
        let on_disk = load_from_path(&path).unwrap();
        let has_a = on_disk
            .breadcrumbs
            .iter()
            .any(|b| b.file == "a.json" && b.path == "x");
        let has_b = on_disk
            .breadcrumbs
            .iter()
            .any(|b| b.file == "b.json" && b.path == "y");
        assert!(has_a, "breadcrumb (a.json, x) missing from merged session");
        assert!(has_b, "breadcrumb (b.json, y) missing from merged session");
    }

    #[test]
    fn save_to_path_leaves_no_tmp_sibling_after_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let session = Session::new("id".to_string(), "lbl".to_string());

        save_to_path(&session, &path).unwrap();

        // No sibling ending in .tmp should remain
        let tmp_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            tmp_files.is_empty(),
            "unexpected .tmp files: {:?}",
            tmp_files
                .iter()
                .map(std::fs::DirEntry::file_name)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn load_or_create_with_nonexistent_path_returns_fresh_session_with_cwd_label()
     {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");

        // label=None → Session::new_with_cwd, which uses cwd to derive the label
        let session =
            load_or_create(&path, "my-id", None, "/home/user/project");

        assert!(session.breadcrumbs.is_empty());
        assert!(session.queries.is_empty());
        assert_eq!(session.step_count, 0);
        assert_eq!(session.id, "my-id");
        // Label must mention the cwd path
        assert!(
            session.label.contains("/home/user/project"),
            "expected label to contain cwd, got: {}",
            session.label
        );
    }

    #[test]
    fn save_and_load_round_trips_all_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let mut session =
            Session::new("test-id".to_string(), "test-label".to_string());
        session.step_count = 3;
        session.breadcrumbs.push(Breadcrumb {
            file: "a.json".to_string(),
            path: "users.0.name".to_string(),
            count: 2,
            last_step: 2,
        });
        session.queries.push(QueryEntry {
            step: 1,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            cwd: "/home/user".to_string(),
            argv: vec!["src/".to_string()],
        });

        save_to_path(&session, &path).unwrap();
        let loaded = load_from_path(&path).unwrap();

        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.label, session.label);
        assert_eq!(loaded.step_count, session.step_count);
        assert_eq!(loaded.breadcrumbs.len(), 1);
        assert_eq!(loaded.breadcrumbs[0].file, "a.json");
        assert_eq!(loaded.breadcrumbs[0].path, "users.0.name");
        assert_eq!(loaded.breadcrumbs[0].count, 2);
        assert_eq!(loaded.breadcrumbs[0].last_step, 2);
        assert_eq!(loaded.queries.len(), 1);
        assert_eq!(loaded.queries[0].step, 1);
        assert_eq!(loaded.queries[0].cwd, "/home/user");
    }
}

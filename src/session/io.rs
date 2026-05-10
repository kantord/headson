use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::types::Session;

/// Counter for unique-per-process tmp filenames in `save_to_path`.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn save_to_path(session: &Session, path: &Path) -> Result<(), io::Error> {
    let json = serde_json::to_string(session)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    // Each writer gets a unique tmp filename so concurrent invocations on the
    // same destination don't race on the rename target.
    let pid = std::process::id();
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_path = path.with_extension(format!("tmp.{pid}.{seq}"));
    fs::write(&tmp_path, &json)?;
    fs::rename(&tmp_path, path)
}

pub fn load_from_path(path: &Path) -> Result<Session, io::Error> {
    let data = fs::read_to_string(path)?;
    serde_json::from_str(&data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// RAII guard for a sibling-file advisory lock acquired via `O_CREAT|O_EXCL`.
/// Released on drop. Stale locks (from crashed processes) require manual
/// cleanup — acceptable trade-off for zero deps and a CLI use case.
struct SessionLock {
    path: PathBuf,
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_session_lock(session_path: &Path) -> io::Result<SessionLock> {
    let lock_path = session_path.with_extension("lock");
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let max_attempts: u32 = 1000; // ~10s at 10ms per attempt
    for _ in 0..max_attempts {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => return Ok(SessionLock { path: lock_path }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "could not acquire session lock at {} after {max_attempts} \
             attempts; remove the file if you're sure no other hson is running",
            lock_path.display()
        ),
    ))
}

/// Eviction policy for `record_step_atomic`.
pub struct EvictionPolicy {
    pub alpha: f64,
    pub breadcrumb_cap: usize,
    pub query_log_cap: usize,
}

/// Atomically record one step against the session at `path`:
///  1. Acquire a sibling-file lock (serializes concurrent writers).
///  2. Read current state from disk (so step_count is fresh).
///  3. Append the query (which assigns the next step number).
///  4. Record breadcrumbs at that step.
///  5. Evict and cap, then atomic-rename write.
///  6. Release the lock.
///
/// If the on-disk file is corrupt or missing, return an error rather than
/// silently overwriting it.
pub fn record_step_atomic(
    path: &Path,
    new_breadcrumbs: &[(String, String)],
    timestamp: &str,
    cwd: &str,
    argv: &[String],
    policy: &EvictionPolicy,
) -> Result<(), io::Error> {
    let _lock = acquire_session_lock(path)?;
    let mut session = load_from_path(path).map_err(|e| {
        eprintln!(
            "warning: session file {} is unreadable ({e}); refusing to \
             overwrite",
            path.display()
        );
        e
    })?;

    session.record_query(timestamp, cwd, argv);
    let new_step = session.step_count;
    for (file, p) in new_breadcrumbs {
        session.record_breadcrumb(file, p, new_step);
    }

    session.evict(new_step, policy.alpha, policy.breadcrumb_cap);
    if session.queries.len() > policy.query_log_cap {
        let excess = session.queries.len() - policy.query_log_cap;
        session.queries.drain(0..excess);
    }
    save_to_path(&session, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{QueryEntry, Session};
    use headson::Breadcrumb;
    use tempfile::tempdir;

    #[test]
    fn record_step_atomic_preserves_breadcrumbs_across_calls() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        save_to_path(&Session::new("sid".into(), "lbl".into()), &path)
            .unwrap();

        record_step_atomic(
            &path,
            &[("a.json".into(), "x".into())],
            "ts1",
            "/",
            &[],
            &EvictionPolicy {
                alpha: 0.5,
                breadcrumb_cap: 500,
                query_log_cap: 1000,
            },
        )
        .unwrap();
        record_step_atomic(
            &path,
            &[("b.json".into(), "y".into())],
            "ts2",
            "/",
            &[],
            &EvictionPolicy {
                alpha: 0.5,
                breadcrumb_cap: 500,
                query_log_cap: 1000,
            },
        )
        .unwrap();

        let on_disk = load_from_path(&path).unwrap();
        let has_a = on_disk
            .breadcrumbs
            .iter()
            .any(|b| b.file == "a.json" && b.path == "x");
        let has_b = on_disk
            .breadcrumbs
            .iter()
            .any(|b| b.file == "b.json" && b.path == "y");
        assert!(has_a, "breadcrumb (a.json, x) missing after second record");
        assert!(has_b, "breadcrumb (b.json, y) missing after second record");
    }

    #[test]
    fn concurrent_record_sessions_preserve_distinct_steps() {
        use std::collections::HashSet;
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        save_to_path(&Session::new("id".into(), "lbl".into()), &path).unwrap();

        const N: usize = 10;
        std::thread::scope(|s| {
            for i in 0..N {
                let path = path.clone();
                s.spawn(move || {
                    let bc = vec![("f".into(), format!("p{i}"))];
                    record_step_atomic(
                        &path,
                        &bc,
                        "ts",
                        "/cwd",
                        &[],
                        &EvictionPolicy {
                            alpha: 0.5,
                            breadcrumb_cap: 500,
                            query_log_cap: 1000,
                        },
                    )
                    .unwrap();
                });
            }
        });

        let final_session = load_from_path(&path).unwrap();
        assert_eq!(
            final_session.step_count, N as u64,
            "step_count should equal number of concurrent writers"
        );
        assert_eq!(
            final_session.queries.len(),
            N,
            "queries log should have one entry per concurrent writer"
        );
        let distinct_steps: HashSet<u64> =
            final_session.queries.iter().map(|q| q.step).collect();
        assert_eq!(
            distinct_steps.len(),
            N,
            "every query should have a distinct step value"
        );
    }

    #[test]
    fn save_to_path_leaves_no_tmp_sibling_after_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let session = Session::new("id".to_string(), "lbl".to_string());

        save_to_path(&session, &path).unwrap();

        let tmp_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
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

    #[test]
    fn corrupt_session_file_is_not_silently_overwritten() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let garbage = b"\x00\x01garbage{not-json,\"";
        std::fs::write(&path, garbage).unwrap();

        let result = record_step_atomic(
            &path,
            &[],
            "ts",
            "/cwd",
            &[],
            &EvictionPolicy {
                alpha: 0.5,
                breadcrumb_cap: 500,
                query_log_cap: 1000,
            },
        );

        let on_disk_after = std::fs::read(&path).unwrap_or_default();
        assert!(
            result.is_err() || on_disk_after == garbage,
            "corrupt session file must not be silently overwritten; \
             got Ok result and on-disk bytes changed from garbage to: {:?}",
            String::from_utf8_lossy(&on_disk_after)
        );
    }
}

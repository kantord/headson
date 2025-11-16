use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use filetime::{FileTime, set_file_mtime};
use git2::{Repository, Signature, Time};
use tempfile::tempdir;

fn rel_to_workdir(path: &Path, repo: &Repository) -> PathBuf {
    let workdir = repo.workdir().expect("workdir");
    let workdir_canon = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let path_canon =
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path.strip_prefix(workdir)
        .or_else(|_| path_canon.strip_prefix(&workdir_canon))
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            path.file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| path.to_path_buf())
        })
}

/// Build a tiny git repo with commits at controlled timestamps (seconds since epoch).
fn init_repo_with_commits(
    specs: &[(&str, i64, usize)],
) -> (tempfile::TempDir, Repository) {
    let dir = tempdir().expect("tempdir");
    let repo = Repository::init(dir.path()).expect("init repo");
    for (idx, (path, seconds, repeats)) in specs.iter().enumerate() {
        let abs = dir.path().join(path);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        for rep in 0..*repeats {
            fs::write(&abs, format!("file {idx} rev {rep}"))
                .expect("write file");
            stage_and_commit(&repo, &abs, *seconds).expect("commit file");
        }
    }
    (dir, repo)
}

fn stage_and_commit(
    repo: &Repository,
    file: &Path,
    timestamp: i64,
) -> Result<git2::Oid, git2::Error> {
    let mut index = repo.index()?;
    let rel = rel_to_workdir(file, repo);
    index.add_path(rel.as_path())?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let time = Time::new(timestamp, 0);
    let sig = Signature::new("tester", "tester@example.com", &time)?;
    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, "test commit", &tree, &parents)
}

fn parse_header_order(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            line.strip_prefix("==> ")
                .and_then(|rest| rest.strip_suffix(" <=="))
                .map(str::to_string)
        })
        .collect()
}

#[test]
fn frecency_orders_fileset_by_recent_commit() {
    // Two files: file_b committed most recently, file_a older; expect file_b first.
    let base = 1_700_000_000i64;
    let specs = [
        ("file_a.txt", base - 86_400 * 10, 1),
        ("file_b.txt", base - 86_400 * 2, 1),
    ];
    let (dir, _repo) = init_repo_with_commits(&specs);
    // Make mtime favor file_a so we can detect frecency winning over mtime.
    set_file_mtime(
        dir.path().join("file_a.txt"),
        FileTime::from_unix_time(200, 0),
    )
    .expect("mtime a");
    set_file_mtime(
        dir.path().join("file_b.txt"),
        FileTime::from_unix_time(100, 0),
    )
    .expect("mtime b");

    // Sanity-check frecency ordering from the library directly.
    unsafe {
        std::env::set_var("XDG_CACHE_HOME", dir.path().join("cache"));
    }
    let mut scores = frecenfile::analyze_repo(dir.path(), None, None)
        .expect("frecenfile scores");
    scores.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    assert_eq!(scores[0].0, Path::new("file_b.txt"));

    let mut cmd = cargo_bin_cmd!("hson");
    let assert = cmd
        .current_dir(dir.path())
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .args([
            "--no-color",
            "-i",
            "text",
            "--debug",
            "-c",
            "1000",
            "file_a.txt",
            "file_b.txt",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let names = parse_header_order(&out);
    assert_eq!(names, vec!["file_b.txt", "file_a.txt"]);
}

#[test]
fn non_git_repo_uses_mtime_order() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "a").expect("write a");
    fs::write(&b, "b").expect("write b");
    // Make b newer than a.
    set_file_mtime(&a, FileTime::from_unix_time(1, 0)).expect("mtime a");
    // Ensure a small gap to avoid same-second ordering.
    set_file_mtime(&b, FileTime::from_unix_time(3, 0)).expect("mtime b");

    let mut cmd = cargo_bin_cmd!("hson");
    let assert = cmd
        .current_dir(dir.path())
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .args([
            "--no-color",
            "-i",
            "text",
            "--debug",
            "-c",
            "1000",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let names = parse_header_order(&out);
    assert_eq!(names, vec!["b.txt", "a.txt"]);
}

#[test]
fn non_git_repo_no_sort_keeps_input_order() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "a").expect("write a");
    fs::write(&b, "b").expect("write b");
    // Make b newer than a so default sorting would flip them.
    set_file_mtime(&a, FileTime::from_unix_time(1, 0)).expect("mtime a");
    set_file_mtime(&b, FileTime::from_unix_time(3, 0)).expect("mtime b");

    let mut cmd = cargo_bin_cmd!("hson");
    let assert = cmd
        .current_dir(dir.path())
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .args([
            "--no-color",
            "--no-sort",
            "-i",
            "text",
            "--debug",
            "-c",
            "1000",
            "a.txt",
            "b.txt",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let names = parse_header_order(&out);
    assert_eq!(names, vec!["a.txt", "b.txt"]);
}

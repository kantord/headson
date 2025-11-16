use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use git2::Repository;

const FRECENFILE_TOP_N: usize = 100;

#[derive(Debug, Clone)]
pub(crate) struct FrecencyContext {
    pub(crate) repo_root: PathBuf,
    pub(crate) ranks: HashMap<OsString, usize>,
}

impl FrecencyContext {
    pub(crate) fn rank_for(&self, rel: &Path) -> Option<usize> {
        self.ranks.get(rel.as_os_str()).copied()
    }
}

#[derive(Clone)]
struct PathOrderEntry {
    idx: usize,
    path: PathBuf,
    rank: Option<usize>,
    modified: Option<u64>,
}

pub(crate) fn sort_paths_for_fileset(paths: &[PathBuf]) -> Vec<PathBuf> {
    if paths.len() <= 1 {
        return paths.to_vec();
    }
    let Ok(cwd) = env::current_dir() else {
        return paths.to_vec();
    };
    let canonical_cwd = cwd.canonicalize().unwrap_or(cwd);
    let frecency = build_frecency_context(&canonical_cwd);
    sort_paths_with_context(paths, &canonical_cwd, frecency.as_ref())
}

pub(crate) fn sort_paths_with_context(
    paths: &[PathBuf],
    cwd: &Path,
    frecency: Option<&FrecencyContext>,
) -> Vec<PathBuf> {
    let mut entries: Vec<PathOrderEntry> = paths
        .iter()
        .enumerate()
        .map(|(idx, path)| {
            let rank = frecency.and_then(|ctx| {
                relative_path_in_repo(path, cwd, &ctx.repo_root)
                    .and_then(|rel| ctx.rank_for(&rel))
            });
            let modified = path_modified_timestamp(path);
            PathOrderEntry {
                idx,
                path: path.clone(),
                rank,
                modified,
            }
        })
        .collect();

    entries.sort_by(compare_path_order);
    if std::env::var_os("HEADSON_FRECEN_TRACE").is_some() {
        eprintln!(
            "frecen-sort input={:?} ranks={:?}",
            paths,
            frecency.map(|ctx| {
                ctx.ranks
                    .iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect::<Vec<_>>()
            })
        );
        eprintln!(
            "frecen-sort sorted={:?}",
            entries
                .iter()
                .map(|e| (e.path.clone(), e.rank, e.modified))
                .collect::<Vec<_>>()
        );
    }
    entries.into_iter().map(|entry| entry.path).collect()
}

fn compare_path_order(a: &PathOrderEntry, b: &PathOrderEntry) -> Ordering {
    match (a.rank, b.rank) {
        (Some(ra), Some(rb)) => {
            let ord = ra.cmp(&rb);
            if ord != Ordering::Equal {
                return ord;
            }
        }
        (Some(_), None) => return Ordering::Less,
        (None, Some(_)) => return Ordering::Greater,
        (None, None) => {}
    }
    match (a.modified, b.modified) {
        (Some(ma), Some(mb)) => {
            let ord = mb.cmp(&ma);
            if ord != Ordering::Equal {
                return ord;
            }
        }
        (Some(_), None) => return Ordering::Less,
        (None, Some(_)) => return Ordering::Greater,
        (None, None) => {}
    }
    a.idx.cmp(&b.idx)
}

pub(crate) fn build_frecency_context(cwd: &Path) -> Option<FrecencyContext> {
    if std::env::var_os("HEADSON_SKIP_FRECEN").is_some() {
        return None;
    }
    let repo = Repository::discover(cwd).ok()?;
    let workdir = repo.workdir()?;
    let canonical_root = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let ranks = load_frecenfile_ranks(&canonical_root)?;
    Some(FrecencyContext {
        repo_root: canonical_root,
        ranks,
    })
}

fn load_frecenfile_ranks(
    repo_root: &Path,
) -> Option<HashMap<OsString, usize>> {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        frecenfile::analyze_repo(repo_root, None, None)
    }));
    panic::set_hook(default_hook);
    let mut scores = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            if std::env::var_os("HEADSON_FRECEN_TRACE").is_some() {
                eprintln!("frecenfile error: {e}");
            }
            return None;
        }
        Err(_) => {
            if std::env::var_os("HEADSON_FRECEN_TRACE").is_some() {
                eprintln!("frecenfile panicked; suppressing");
            }
            return None;
        }
    };
    scores.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    scores.truncate(FRECENFILE_TOP_N);
    let mut ranks = HashMap::with_capacity(scores.len());
    for (idx, (path, _score)) in scores.into_iter().enumerate() {
        ranks.insert(path.into_os_string(), idx);
    }
    Some(ranks)
}

fn relative_path_in_repo(
    path: &Path,
    cwd: &Path,
    repo_root: &Path,
) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let canonical = absolute.canonicalize().ok()?;
    canonical.strip_prefix(repo_root).ok().map(PathBuf::from)
}

fn path_modified_timestamp(path: &Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(duration.as_secs())
}

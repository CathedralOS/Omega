//! Manage the `.agents/worktree/worker-N` checkout pool (`cargo xtask worktree`).
//!
//! Why a fixed-path `worker-1`, `worker-2`, ... pool instead of task-named directories:
//! Cargo embeds absolute source paths inside `target/debug/.fingerprint/*` and `*.d`
//! dependency files. Moving or renaming a worktree directory invalidates those path
//! fingerprints and forces a full workspace rebuild, whereas keeping a fixed slot path
//! (`.agents/worktree/worker-1`) and resetting `HEAD` in place (`git checkout --detach
//! origin/main && git clean -fd`) preserves 100% of Cargo's incremental cache.
//!
//! Each slot's current `<lane>-<topic>` label is stored in its Git administration
//! directory, so moving a checkout preserves ownership and keeps its files clean.
//!
//! Subcommands:
//! - `cargo xtask worktree status` — list all worktrees, their active topic, state, and cache.
//! - `cargo xtask worktree acquire [<lane>-<topic>]` — claim an idle `worker-N` slot (or
//!   recycle one into `worker-N`), reset it to `origin/main` in place, and print its path.
//!   In a cloud session the slot starts at the commit its restored build was published
//!   for instead; see [`acquire_base`].
//! - `cargo xtask worktree release [worker-N]` — reset a finished slot to `origin/main`
//!   and clear its topic while keeping its warm `target/` cache.
//! - `cargo xtask worktree clean` — reset idle `worker-N` slots and remove legacy
//!   non-pool worktrees, orphaned directories, and stray log files in parallel. It
//!   also prunes registrations whose checkout directory is gone; the other
//!   subcommands skip those with a notice instead of failing.
//!
//! Several sessions share this pool, and a reset (`checkout --detach origin/main` plus
//! `clean -fd`) or a removal destroys whatever a worktree holds. So nothing is reset,
//! moved or removed unless [`reset_refusal`] finds it safe: not locked, no tracked or
//! untracked changes, and no commit that exists only in that worktree. A slot with a
//! topic belongs to the session that acquired it until `release` clears the topic, so
//! `acquire` and `clean` never take a claimed slot; only its own topic reuses it.
//! Those commands run one at a time under [`PoolLock`], so two sessions can never
//! both pick the same idle slot before either has written its topic.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::repo_root;

#[derive(Debug, Clone)]
struct WorktreeInfo {
    path: PathBuf,
    git_dir: PathBuf,
    name: String,
    slot_index: Option<usize>,
    topic: Option<String>,
    head: String,
    locked: bool,
    at_main: bool,
    open_pr: bool,
    has_warm_target: bool,
}

fn worker_slot_index(name: &str) -> Option<usize> {
    let suffix = name.strip_prefix("worker-")?;
    let idx: usize = suffix.parse().ok()?;
    (idx >= 1 && suffix == idx.to_string()).then_some(idx)
}

fn primary_root(current_root: &Path) -> PathBuf {
    let out = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(current_root)
        .stdin(Stdio::null())
        .output();
    if let Ok(out) = out
        && out.status.success()
    {
        let raw = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let git_dir = Path::new(&raw);
        let resolved = if git_dir.is_absolute() {
            git_dir.to_path_buf()
        } else {
            current_root.join(git_dir)
        };
        if let Ok(canon) = fs::canonicalize(&resolved) {
            let clean = canon
                .to_string_lossy()
                .strip_prefix(r"\\?\")
                .unwrap_or(&canon.to_string_lossy())
                .to_owned();
            let clean_path = PathBuf::from(clean);
            if clean_path.file_name().and_then(|n| n.to_str()) == Some(".git")
                && let Some(parent) = clean_path.parent()
            {
                return parent.to_path_buf();
            }
        }
    }
    current_root.to_path_buf()
}

fn worktree_git_dir(worktree_path: &Path) -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .current_dir(worktree_path)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            format!(
                "cannot locate Git directory for {}: {error}",
                worktree_path.display()
            )
        })?;
    if !out.status.success() {
        return Err(format!(
            "cannot locate Git directory for {}: {}",
            worktree_path.display(),
            String::from_utf8_lossy(&out.stderr).trim(),
        ));
    }
    let directory = String::from_utf8(out.stdout).map_err(|error| {
        format!(
            "Git directory for {} is not UTF-8: {error}",
            worktree_path.display()
        )
    })?;
    let directory = PathBuf::from(directory.trim());
    if !directory.is_absolute() {
        return Err(format!(
            "Git returned a non-absolute directory for {}",
            worktree_path.display()
        ));
    }
    Ok(directory)
}

fn read_topic(git_dir: &Path) -> Result<Option<String>, String> {
    let path = git_dir.join("topic");
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot read worktree topic {}: {error}",
                path.display()
            ));
        }
    };
    let trimmed = raw.trim();
    Ok((!trimmed.is_empty()).then(|| trimmed.to_owned()))
}

fn write_topic(git_dir: &Path, topic: Option<&str>) -> Result<(), String> {
    let path = git_dir.join("topic");
    let result = match topic {
        Some(topic) if !topic.is_empty() => fs::write(&path, format!("{topic}\n")),
        _ => match fs::remove_file(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        },
    };
    result.map_err(|error| format!("cannot update worktree topic {}: {error}", path.display()))
}

/// Serializes `acquire`, `release` and `clean` across sessions.
///
/// Choosing a slot and claiming it (writing its topic) must happen as one step.
/// Without this lock, two sessions acquiring at once could both see the same
/// slot as idle, for instance one freshly promoted from a legacy worktree that
/// had no topic yet, and both be handed `worker-N`. A clean or dirty check cannot
/// catch that, because a freshly acquired slot is clean.
struct PoolLock(PathBuf);

impl Drop for PoolLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

const POOL_LOCK_WAIT: Duration = Duration::from_secs(120);
// Pool operations finish in seconds once `origin/main` is fetched (fetch runs
// before the lock is taken), so an older lock was left by a killed process.
const POOL_LOCK_STALE: Duration = Duration::from_secs(300);

fn lock_pool(main_root: &Path) -> Result<PoolLock, String> {
    lock_pool_at(
        &main_root.join(".git/worktree-pool.lock"),
        POOL_LOCK_WAIT,
        POOL_LOCK_STALE,
    )
}

fn lock_pool_at(path: &Path, wait: Duration, stale: Duration) -> Result<PoolLock, String> {
    let deadline = Instant::now() + wait;
    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", std::process::id());
                return Ok(PoolLock(path.to_path_buf()));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                // ponytail: two waiters can both break the same stale lock; the
                // window is one remove/create pair after a crashed holder.
                let age = fs::metadata(path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok());
                if age.is_some_and(|age| age > stale) {
                    let _ = fs::remove_file(path);
                    continue;
                }
                if Instant::now() >= deadline {
                    return Err(format!(
                        "another session holds the worktree pool lock {}; retry, or delete it if no `xtask worktree` command is running",
                        path.display()
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                return Err(format!(
                    "cannot create worktree pool lock {}: {error}",
                    path.display()
                ));
            }
        }
    }
}

fn report_error(message: String) -> u8 {
    eprintln!("worktree: {message}");
    1
}

fn rev_parse(root: &Path, rev: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", rev])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn open_pr_commits(root: &Path) -> BTreeSet<String> {
    let mut oids = BTreeSet::new();
    let Ok(out) = Command::new("gh")
        .args(["pr", "list", "--state", "open", "--json", "headRefOid"])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
    else {
        return oids;
    };
    if !out.status.success() {
        return oids;
    }
    if let Ok(items) = serde_json::from_slice::<Vec<Value>>(&out.stdout) {
        for item in items {
            if let Some(oid) = item["headRefOid"].as_str() {
                oids.insert(oid.to_owned());
            }
        }
    }
    oids
}

/// Tracked or untracked changes. Untracked files count because a reset ends with
/// `git clean -fd`, which deletes them; ignored files such as `target/` survive it.
/// A worktree whose status cannot be read counts as dirty, so it is left alone.
fn is_dirty(worktree_path: &Path) -> bool {
    let Ok(out) = Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--ignore-submodules=dirty",
        ])
        .current_dir(worktree_path)
        .stdin(Stdio::null())
        .output()
    else {
        return true;
    };
    !out.status.success() || !String::from_utf8_lossy(&out.stdout).trim().is_empty()
}

/// Whether `head` holds commits that exist only in this worktree: not on
/// `origin/main` and not on any remote branch. Resetting such a worktree would
/// leave those commits unreachable. An unreadable answer counts as unsaved.
fn has_unsaved_commits(main_root: &Path, head: &str) -> bool {
    if head.is_empty() {
        return false;
    }
    let on_main = Command::new("git")
        .args(["merge-base", "--is-ancestor", head, "origin/main"])
        .current_dir(main_root)
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if on_main {
        return false;
    }
    let Ok(out) = Command::new("git")
        .args([
            "for-each-ref",
            "--count=1",
            "--contains",
            head,
            "--format=%(refname)",
            "refs/remotes",
        ])
        .current_dir(main_root)
        .stdin(Stdio::null())
        .output()
    else {
        return true;
    };
    !out.status.success() || String::from_utf8_lossy(&out.stdout).trim().is_empty()
}

/// Restored checkouts may have fetched only main even though their detached
/// checkpoint was pushed. Retain its actual origin branch locally before the
/// normal reset guard runs; an advertised object ID alone is not a saved ref.
fn fetch_published_head(main_root: &Path, head: &str) -> Result<(), String> {
    let advertised = Command::new("git")
        .args(["ls-remote", "--heads", "origin"])
        .current_dir(main_root)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "cannot inspect published worktree checkpoints".to_owned())?;
    if !advertised.status.success() {
        return Err("cannot inspect published worktree checkpoints".to_owned());
    }
    let refs = String::from_utf8_lossy(&advertised.stdout);
    for line in refs.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() != Some(head) {
            continue;
        }
        let Some(branch) = fields
            .next()
            .and_then(|name| name.strip_prefix("refs/heads/"))
        else {
            continue;
        };
        let refspec = format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
        let fetched = Command::new("git")
            .args(["fetch", "--no-tags", "origin", &refspec])
            .current_dir(main_root)
            .stdin(Stdio::null())
            .output()
            .map_err(|_| format!("cannot retain published worktree branch `{branch}`"))?;
        return if fetched.status.success() {
            Ok(())
        } else {
            Err(format!(
                "cannot retain published worktree branch `{branch}`"
            ))
        };
    }
    Ok(())
}

/// Why a worktree must not be reset, moved or removed, or `None` when doing so
/// loses nothing.
fn reset_refusal(locked: bool, dirty: bool, unsaved: bool) -> Option<&'static str> {
    if locked {
        Some("it is locked")
    } else if dirty {
        Some("it has uncommitted or untracked changes")
    } else if unsaved {
        Some("its HEAD has commits that are on no remote branch")
    } else {
        None
    }
}

fn refusal_for(main_root: &Path, wt: &WorktreeInfo) -> Option<&'static str> {
    reset_refusal(
        wt.locked,
        is_dirty(&wt.path),
        has_unsaved_commits(main_root, &wt.head),
    )
}

fn reset_slot_to_main(worktree_path: &Path) -> bool {
    reset_slot_to(worktree_path, "origin/main")
}

fn reset_slot_to(worktree_path: &Path, commit: &str) -> bool {
    let checkout_ok = Command::new("git")
        .args(["checkout", "--detach", commit])
        .current_dir(worktree_path)
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    let clean_ok = Command::new("git")
        .args(["clean", "-fd"])
        .current_dir(worktree_path)
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    checkout_ok && clean_ok
}

/// The commit a restored build was published for, read from the manifest
/// `tools/warm_build.sh restore` leaves at `target/.warm-build.txt`.
fn published_build_commit(manifest: &str) -> Option<&str> {
    let commit = manifest.lines().find_map(|l| l.strip_prefix("commit="))?;
    let commit = commit.trim();
    (commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit())).then_some(commit)
}

fn is_ancestor(main_root: &Path, commit: &str, of: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", commit, of])
        .current_dir(main_root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn cloud_session(claude_remote: Option<&str>, codex_environment: Option<&str>) -> bool {
    // A local Codex process also has session markers. Only its managed
    // environment marker opts into preserving a restored cloud build's base.
    claude_remote == Some("true") || codex_environment.is_some_and(|id| !id.is_empty())
}

/// The commit `acquire` starts a slot at, announced on stderr when it is not
/// `origin/main`.
///
/// A cloud session is a fresh container whose only built compiler is the one
/// the SessionStart hook restored, and that build is reused only at the commit
/// it was published for: a few commits later, with a foundation crate changed,
/// the first build takes as long as a cold one (about 11 minutes on 4 CPUs
/// against under a minute). So in a cloud session a slot holding a restored
/// build starts at that build's commit, and the worker rebases onto main only
/// when the task needs something newer; the merge queue integrates the rest.
///
/// The SessionStart hook creates `worker-1` at that commit. Once the slot has
/// moved (released to main, or checked out elsewhere), its `target/` holds a
/// later build and no longer matches the stamp, so it resets to `origin/main`.
/// Everywhere else, and for a slot with no restored build, the base stays
/// `origin/main`: a local slot's `target/` was built by the previous task there
/// and is already close to main. A published commit that is not an ancestor of
/// `origin/main` is never used, so work cannot start from a commit main dropped.
fn acquire_base(main_root: &Path, slot_path: &Path, head: &str, cloud_session: bool) -> String {
    let main = "origin/main".to_owned();
    if !cloud_session {
        return main;
    }
    // mbx may have turned `target` into a link; the manifest is read through it.
    let Ok(manifest) = fs::read_to_string(slot_path.join("target/.warm-build.txt")) else {
        return main;
    };
    let Some(commit) = published_build_commit(&manifest) else {
        return main;
    };
    if commit != head {
        return main;
    }
    let main_oid = rev_parse(main_root, "origin/main").unwrap_or_default();
    if commit == main_oid {
        return main;
    }
    let slot = slot_path.file_name().unwrap_or_default().to_string_lossy();
    if !is_ancestor(main_root, commit, "origin/main") {
        eprintln!(
            "worktree acquire: `{slot}` holds a build restored for {}, which is not an ancestor of origin/main in this clone; starting at origin/main, so the first build recompiles what differs",
            &commit[..12]
        );
        return main;
    }
    let behind = Command::new("git")
        .args(["rev-list", "--count", &format!("{commit}..origin/main")])
        .current_dir(main_root)
        .stdin(Stdio::null())
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_default();
    eprintln!(
        "worktree acquire: `{slot}` starts at {}, the commit its restored build was published for; origin/main is {}, {behind} commit(s) ahead. Rebase onto origin/main only if the task needs something newer",
        &commit[..12],
        &main_oid[..main_oid.len().min(12)]
    );
    commit.to_owned()
}

/// One record of `git worktree list --porcelain`.
#[derive(Debug, PartialEq)]
struct ListedWorktree {
    path: PathBuf,
    head: String,
    locked: bool,
    /// Git reports the checkout directory as missing, so `git worktree prune`
    /// would drop this registration.
    prunable: bool,
}

/// Split porcelain output into its records. Each record starts at a `worktree`
/// line; `locked` and `prunable` may carry a reason after the keyword.
fn parse_worktree_list(text: &str) -> Vec<ListedWorktree> {
    let mut records: Vec<ListedWorktree> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("worktree ") {
            records.push(ListedWorktree {
                path: PathBuf::from(rest.trim()),
                head: String::new(),
                locked: false,
                prunable: false,
            });
        } else if let Some(current) = records.last_mut() {
            if let Some(rest) = line.strip_prefix("HEAD ") {
                current.head = rest.trim().to_owned();
            } else if line.starts_with("locked") {
                current.locked = true;
            } else if line.starts_with("prunable") {
                current.prunable = true;
            }
        }
    }
    records
}

fn list_worktrees(main_root: &Path) -> Result<Vec<WorktreeInfo>, String> {
    let open_prs = open_pr_commits(main_root);
    let main_oid = rev_parse(main_root, "origin/main").unwrap_or_default();
    let out = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(main_root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot list worktrees: {error}"))?;
    if !out.status.success() {
        return Err(format!(
            "cannot list worktrees: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut result = Vec::new();
    for listed in parse_worktree_list(&text) {
        if listed.path == main_root {
            continue;
        }
        // A session's scratch directory can vanish with the session, leaving Git a
        // registration whose checkout is gone. It holds no work and has no Git
        // directory to read a topic from, so it is not a pool member; failing here
        // would stop every session from acquiring a slot until someone pruned it.
        if listed.prunable {
            eprintln!(
                "worktree: ignoring `{}`: its directory is gone (`cargo xtask worktree clean` prunes it)",
                listed.path.display()
            );
            continue;
        }
        let name = listed
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_owned();
        let slot_index = worker_slot_index(&name);
        // Git keeps this directory's name when the checkout is moved.
        let git_dir = worktree_git_dir(&listed.path)?;
        let topic = read_topic(&git_dir)?;
        let at_main = !listed.head.is_empty() && listed.head == main_oid;
        let open_pr = open_prs.contains(&listed.head);
        let has_warm_target = listed.path.join("target/debug").is_dir();
        result.push(WorktreeInfo {
            path: listed.path,
            git_dir,
            name,
            slot_index,
            topic,
            head: listed.head,
            locked: listed.locked,
            at_main,
            open_pr,
            has_warm_target,
        });
    }
    result.sort_by_key(|w| {
        (
            w.slot_index.is_none(),
            w.slot_index.unwrap_or(usize::MAX),
            w.name.clone(),
        )
    });
    Ok(result)
}

fn is_valid_topic_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

fn remove_dir_all_force(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    if fs::remove_dir_all(path).is_ok() {
        return true;
    }
    if cfg!(windows) {
        let _ = Command::new("cmd")
            .args(["/C", "rmdir", "/S", "/Q", &path.to_string_lossy()])
            .stdin(Stdio::null())
            .status();
    }
    !path.exists()
}

fn next_free_slot_index(items: &[WorktreeInfo], wt_base: &Path) -> usize {
    let used: BTreeSet<usize> = items.iter().filter_map(|w| w.slot_index).collect();
    let mut idx = 1usize;
    while used.contains(&idx) || wt_base.join(format!("worker-{idx}")).exists() {
        idx += 1;
    }
    idx
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    let current = repo_root();
    let main_root = primary_root(&current);
    let wt_base = main_root.join(".agents/worktree");
    let current_name = current
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_owned();

    match sub {
        "-h" | "--help" | "help" => {
            println!(
                "usage:\n  cargo xtask worktree status\n  cargo xtask worktree acquire [<lane>-<topic>]\n  cargo xtask worktree release [worker-N]\n  cargo xtask worktree clean [--dry-run] [--keep <name>...]"
            );
            Ok(())
        }
        "status" | "list" => {
            let items = list_worktrees(&main_root).map_err(report_error)?;
            println!(
                "{:<16} {:<24} {:<12} {:<10} {:<6} PATH",
                "SLOT", "TOPIC", "HEAD", "STATE", "CACHE"
            );
            for item in &items {
                let state = if item.name == current_name {
                    "current"
                } else if item.open_pr {
                    "open-pr"
                } else if item.slot_index.is_some() && item.at_main {
                    "idle"
                } else if item.slot_index.is_some() {
                    "assigned"
                } else {
                    "legacy"
                };
                let topic = item.topic.as_deref().unwrap_or("-");
                let cache = if item.has_warm_target { "warm" } else { "-" };
                let short_head = &item.head[..item.head.len().min(11)];
                println!(
                    "{:<16} {:<24} {:<12} {:<10} {:<6} {}",
                    item.name,
                    topic,
                    short_head,
                    state,
                    cache,
                    item.path.display()
                );
            }
            println!("worktree: {} registered worktree(s)", items.len());
            Ok(())
        }
        "acquire" => {
            let topic = args.get(1).map(String::as_str);
            if let Some(t) = topic
                && !is_valid_topic_name(t)
            {
                eprintln!(
                    "worktree acquire: invalid topic `{t}`; use lowercase letters, digits, and hyphens (e.g. `cli-arguments`)"
                );
                return Err(2);
            }
            let _ = fs::create_dir_all(&wt_base);
            let _ = Command::new("git")
                .args(["fetch", "origin", "main"])
                .current_dir(&main_root)
                .stdin(Stdio::null())
                .status();

            let _pool = lock_pool(&main_root).map_err(report_error)?;
            let items = list_worktrees(&main_root).map_err(report_error)?;
            let cloud_session = cloud_session(
                std::env::var("CLAUDE_CODE_REMOTE").ok().as_deref(),
                std::env::var("CODEX_ENVIRONMENT_ID").ok().as_deref(),
            );
            let base = |slot: &WorktreeInfo, path: &Path| {
                acquire_base(&main_root, path, &slot.head, cloud_session)
            };

            // 1. If a worker-N slot already carries this topic and is clean, reuse it in place.
            if let Some(t) = topic
                && let Some(existing) = items
                    .iter()
                    .find(|w| w.slot_index.is_some() && w.topic.as_deref() == Some(t))
            {
                if let Some(reason) = refusal_for(&main_root, existing) {
                    eprintln!(
                        "worktree acquire: `{}` carries topic `{}` but {reason}; push or discard that work first",
                        existing.path.display(),
                        topic.unwrap_or("-")
                    );
                    return Err(1);
                }
                if !reset_slot_to(&existing.path, &base(existing, &existing.path)) {
                    return Err(1);
                }
                println!("{}", existing.path.display());
                return Ok(());
            }

            // 2. Reuse a released `worker-N` slot in place (preserving its fixed path and warm
            // `target/`). A slot with a topic is still claimed by another session.
            let mut idle_slots: Vec<&WorktreeInfo> = items
                .iter()
                .filter(|w| w.slot_index.is_some() && w.topic.is_none() && !w.open_pr)
                .collect();
            idle_slots.sort_by_key(|w| (!w.has_warm_target, w.slot_index.unwrap_or(usize::MAX)));

            for slot in idle_slots {
                if refusal_for(&main_root, slot).is_some() {
                    continue;
                }
                if reset_slot_to(&slot.path, &base(slot, &slot.path)) {
                    write_topic(&slot.git_dir, topic).map_err(report_error)?;
                    eprintln!(
                        "worktree acquire: assigned `{}` (topic: {})",
                        slot.name,
                        topic.unwrap_or("-")
                    );
                    println!("{}", slot.path.display());
                    return Ok(());
                }
            }

            // 3. Allocate the next `worker-N` slot, migrating a warm legacy worktree if one exists.
            let next_idx = next_free_slot_index(&items, &wt_base);
            let slot_name = format!("worker-{next_idx}");
            let slot_path = wt_base.join(&slot_name);

            let mut legacy_donors: Vec<&WorktreeInfo> = items
                .iter()
                .filter(|w| {
                    w.slot_index.is_none()
                        && w.path.starts_with(&wt_base)
                        && !w.open_pr
                        && w.name != current_name
                })
                .collect();
            legacy_donors.sort_by_key(|w| (!w.has_warm_target, w.name.clone()));

            for donor in legacy_donors {
                if refusal_for(&main_root, donor).is_some() {
                    continue;
                }
                let move_ok = Command::new("git")
                    .args([
                        "worktree",
                        "move",
                        &donor.path.to_string_lossy(),
                        &slot_path.to_string_lossy(),
                    ])
                    .current_dir(&main_root)
                    .stdin(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if move_ok && reset_slot_to(&slot_path, &base(donor, &slot_path)) {
                    write_topic(&donor.git_dir, topic).map_err(report_error)?;
                    eprintln!(
                        "worktree acquire: promoted `{}` -> `{}` (topic: {})",
                        donor.name,
                        slot_name,
                        topic.unwrap_or("-")
                    );
                    println!("{}", slot_path.display());
                    return Ok(());
                }
            }

            let status = Command::new("git")
                .args([
                    "worktree",
                    "add",
                    "--detach",
                    &slot_path.to_string_lossy(),
                    "origin/main",
                ])
                .current_dir(&main_root)
                .stdin(Stdio::null())
                .status()
                .map_err(|_| 1u8)?;
            if !status.success() {
                return Err(1);
            }
            let git_dir = worktree_git_dir(&slot_path).map_err(report_error)?;
            write_topic(&git_dir, topic).map_err(report_error)?;
            eprintln!(
                "worktree acquire: created `{}` (topic: {})",
                slot_name,
                topic.unwrap_or("-")
            );
            println!("{}", slot_path.display());
            Ok(())
        }
        "release" => {
            let target_name = args.get(1).cloned().unwrap_or_else(|| current_name.clone());
            let _ = Command::new("git")
                .args(["fetch", "origin", "main"])
                .current_dir(&main_root)
                .stdin(Stdio::null())
                .status();
            // Network access stays outside the pool lock. The locked check below
            // rereads the slot and still refuses any dirty, locked or unsaved work.
            let before = list_worktrees(&main_root).map_err(report_error)?;
            if let Some(wt) = before.iter().find(|w| w.name == target_name)
                && !wt.locked
                && !is_dirty(&wt.path)
                && has_unsaved_commits(&main_root, &wt.head)
            {
                fetch_published_head(&main_root, &wt.head).map_err(report_error)?;
            }
            let _pool = lock_pool(&main_root).map_err(report_error)?;
            let items = list_worktrees(&main_root).map_err(report_error)?;
            let Some(wt) = items.iter().find(|w| w.name == target_name) else {
                eprintln!("worktree release: unknown worktree `{target_name}`");
                return Err(1);
            };
            if let Some(reason) = refusal_for(&main_root, wt) {
                eprintln!(
                    "worktree release: `{}` not released because {reason}; push or discard that work first",
                    wt.path.display()
                );
                return Err(1);
            }
            if !reset_slot_to_main(&wt.path) {
                return Err(1);
            }
            write_topic(&wt.git_dir, None).map_err(report_error)?;
            println!("released `{}` back to idle pool at origin/main", wt.name);
            Ok(())
        }
        "clean" => {
            let mut dry_run = false;
            let mut keep = BTreeSet::new();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--dry-run" => dry_run = true,
                    "--keep" => {
                        i += 1;
                        if let Some(k) = args.get(i) {
                            keep.insert(k.clone());
                        }
                    }
                    other => {
                        eprintln!("worktree clean: unknown argument `{other}`");
                        return Err(2);
                    }
                }
                i += 1;
            }
            if !current_name.is_empty() {
                keep.insert(current_name.clone());
            }
            let _pool = lock_pool(&main_root).map_err(report_error)?;

            // Git only prunes a registration whose checkout directory is missing and
            // that is not locked, so this can never discard work.
            if !dry_run {
                let _ = Command::new("git")
                    .args(["worktree", "prune"])
                    .current_dir(&main_root)
                    .stdin(Stdio::null())
                    .status();
            }

            let items = list_worktrees(&main_root).map_err(report_error)?;
            let mut to_delete_dirs: Vec<(String, PathBuf)> = Vec::new();
            let mut reset_slots = 0usize;
            // Legacy worktrees refused for removal; their directories stay out of the
            // orphan sweep below too.
            let mut kept: BTreeSet<String> = BTreeSet::new();

            // Seed `worker-1` from a warm legacy worktree if no `worker-1` exists yet.
            let has_worker_1 = items.iter().any(|w| w.slot_index == Some(1));
            let mut promoted_name: Option<String> = None;
            if !has_worker_1
                && !dry_run
                && let Some(donor) = items.iter().find(|w| {
                    w.slot_index.is_none()
                        && w.path.starts_with(&wt_base)
                        && !w.open_pr
                        && !keep.contains(&w.name)
                        && w.has_warm_target
                        && refusal_for(&main_root, w).is_none()
                })
            {
                let worker1_path = wt_base.join("worker-1");
                let move_ok = Command::new("git")
                    .args([
                        "worktree",
                        "move",
                        &donor.path.to_string_lossy(),
                        &worker1_path.to_string_lossy(),
                    ])
                    .current_dir(&main_root)
                    .stdin(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if move_ok {
                    let _ = reset_slot_to_main(&worker1_path);
                    write_topic(&donor.git_dir, None).map_err(report_error)?;
                    println!("promoted warm worktree `{}` -> `worker-1`", donor.name);
                    promoted_name = Some(donor.name.clone());
                }
            }

            for wt in &items {
                if keep.contains(&wt.name)
                    || wt.open_pr
                    || promoted_name.as_deref() == Some(wt.name.as_str())
                {
                    continue;
                }
                if wt.slot_index.is_some() {
                    // Pool slots stay in the pool; reset released slots back to origin/main.
                    // A claimed slot (topic set) is another session's until it releases it.
                    if !wt.at_main && wt.topic.is_none() && refusal_for(&main_root, wt).is_none() {
                        if dry_run {
                            println!("would reset pool slot `{}` to origin/main", wt.name);
                        } else if reset_slot_to_main(&wt.path) {
                            write_topic(&wt.git_dir, None).map_err(report_error)?;
                            println!("reset pool slot `{}` to origin/main", wt.name);
                        }
                        reset_slots += 1;
                    }
                    continue;
                }
                if let Some(reason) = refusal_for(&main_root, wt) {
                    println!("kept legacy worktree `{}` because {reason}", wt.name);
                    kept.insert(wt.name.clone());
                    continue;
                }
                if dry_run {
                    println!(
                        "would remove legacy worktree `{}` ({})",
                        wt.name,
                        wt.path.display()
                    );
                } else {
                    // Remove git worktree admin metadata first so directory deletion can run in parallel.
                    fs::remove_dir_all(&wt.git_dir).map_err(|error| {
                        report_error(format!(
                            "cannot remove Git directory {}: {error}",
                            wt.git_dir.display(),
                        ))
                    })?;
                    to_delete_dirs.push((wt.name.clone(), wt.path.clone()));
                }
            }

            let active_names: BTreeSet<String> = items
                .iter()
                .filter(|w| {
                    keep.contains(&w.name)
                        || kept.contains(&w.name)
                        || w.open_pr
                        || w.slot_index.is_some()
                        || promoted_name.as_deref() == Some(w.name.as_str())
                })
                .map(|w| w.name.clone())
                .chain(promoted_name.iter().map(|_| "worker-1".to_owned()))
                .collect();

            if let Ok(entries) = fs::read_dir(&wt_base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_owned();
                    if active_names.contains(&name) {
                        continue;
                    }
                    if path.is_file() {
                        if dry_run {
                            println!("would remove stray file `{}`", path.display());
                        } else {
                            let _ = fs::remove_file(&path);
                            println!("removed stray file `{name}`");
                        }
                    } else if path.is_dir() && !to_delete_dirs.iter().any(|(_, p)| p == &path) {
                        if dry_run {
                            println!("would remove orphaned directory `{}`", path.display());
                        } else {
                            to_delete_dirs.push((name, path));
                        }
                    }
                }
            }

            let deleted_count = to_delete_dirs.len();
            if !dry_run && !to_delete_dirs.is_empty() {
                let workers = std::thread::available_parallelism()
                    .map(|n| n.get().min(16))
                    .unwrap_or(8);
                let chunk_size = to_delete_dirs.len().div_ceil(workers);
                std::thread::scope(|scope| {
                    for chunk in to_delete_dirs.chunks(chunk_size) {
                        scope.spawn(move || {
                            for (name, path) in chunk {
                                remove_dir_all_force(path);
                                println!("removed `{name}`");
                            }
                        });
                    }
                });
                let _ = Command::new("git")
                    .args(["worktree", "prune"])
                    .current_dir(&main_root)
                    .stdin(Stdio::null())
                    .status();
            }

            println!(
                "worktree clean: {} legacy directory(s) {}, {} pool slot(s) reset",
                deleted_count,
                if dry_run { "selected" } else { "removed" },
                reset_slots
            );
            Ok(())
        }
        other => {
            eprintln!("worktree: unknown subcommand `{other}`");
            Err(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ListedWorktree, acquire_base, cloud_session, fetch_published_head, has_unsaved_commits,
        lock_pool_at, parse_worktree_list, published_build_commit, read_topic, reset_refusal,
        worktree_git_dir, write_topic,
    };
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    #[test]
    fn managed_cloud_markers_enable_warm_builds() {
        for (claude, codex, expected) in [
            (None, None, false),
            (Some("false"), None, false),
            (None, Some(""), false),
            (Some("true"), None, true),
            (None, Some("environment-id"), true),
            (Some("false"), Some("environment-id"), true),
        ] {
            assert_eq!(cloud_session(claude, codex), expected);
        }
    }

    // Run the real startup script with local manifests and already-built slots:
    // exercise host detection without fetching archives or changing a user's cache.
    #[cfg(unix)]
    #[test]
    fn warm_startup_recognizes_managed_codex_and_keeps_local_sessions_idle() {
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = Scratch(
            std::env::temp_dir().join(format!("omega-warm-start-{}-{unique}", std::process::id(),)),
        );
        let root = &scratch.0;
        for slot in ["target", ".agents/worktree/worker-1/target"] {
            fs::create_dir_all(root.join(slot)).unwrap();
            fs::write(root.join(slot).join("keep"), "existing build").unwrap();
        }
        fs::create_dir(root.join("tools")).unwrap();
        fs::write(
            root.join("tools/warm_build.sh"),
            include_str!("../../warm_build.sh"),
        )
        .unwrap();
        let init = Command::new("git").arg("init").arg(root).output().unwrap();
        assert!(init.status.success());
        let manifests = root.join("manifests");
        fs::create_dir(&manifests).unwrap();
        for slot in ["primary", "worker"] {
            fs::write(
                manifests.join(format!("warm-build-{slot}.txt")),
                "schema=omega-warm-build-v2\ncommit=0000000000000000000000000000000000000000\n",
            )
            .unwrap();
        }
        // Report an existing floor without accessing the user's mbx settings.
        fs::create_dir(root.join("bin")).unwrap();
        let mbx = root.join("bin/mbx");
        fs::write(
            &mbx,
            "#!/bin/sh\n[ \"$*\" = 'settings get gc.min_free_size' ] || exit 1\necho 5GiB\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(mbx, fs::Permissions::from_mode(0o755)).unwrap();
        let paths = std::env::join_paths(
            std::iter::once(root.join("bin"))
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        for (claude, codex, expected) in [
            ("", "", false),
            ("false", "", false),
            ("true", "", true),
            ("", "managed-environment", true),
        ] {
            let output = Command::new("bash")
                .arg(root.join("tools/warm_build.sh"))
                .arg("session-start")
                .env("PATH", &paths)
                .env("CLAUDE_CODE_REMOTE", claude)
                .env("CODEX_ENVIRONMENT_ID", codex)
                .env("CODEX_SESSION_ID", "also-present-locally")
                .env(
                    "OMEGA_WARM_BUILD_SOURCE",
                    format!("file://{}", manifests.display()),
                )
                .output()
                .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert_eq!(
                stdout.contains("warm build: primary kept"),
                expected,
                "{stdout}"
            );
            assert_eq!(
                stdout.contains("warm build: worker kept"),
                expected,
                "{stdout}"
            );
        }
    }

    #[test]
    fn ownership_follows_a_moved_linked_worktree() {
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fn git(root: &Path, arguments: &[&str]) {
            let output = Command::new("git")
                .args(arguments)
                .current_dir(root)
                .stdin(Stdio::null())
                .output()
                .expect("run temporary repository command");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "omega-worktree-topic-{}-{unique}",
            std::process::id(),
        )));
        fs::create_dir(&scratch.0).unwrap();
        let repository = scratch.0.join("repository");
        fs::create_dir(&repository).unwrap();
        let hooks = scratch.0.join("empty-hooks");
        fs::create_dir(&hooks).unwrap();
        git(&repository, &["init"]);
        git(
            &repository,
            &[
                "-c",
                "user.name=Worktree test",
                "-c",
                "user.email=worktree@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                &format!("core.hooksPath={}", hooks.display()),
                "commit",
                "--allow-empty",
                "-m",
                "initial",
            ],
        );
        let original = scratch.0.join("original-worktree");
        let moved = scratch.0.join("worker-1");
        git(
            &repository,
            &[
                "worktree",
                "add",
                "--detach",
                original.to_str().unwrap(),
                "HEAD",
            ],
        );
        let git_dir = worktree_git_dir(&original).unwrap();
        write_topic(&git_dir, Some("tools-before-move")).unwrap();
        git(
            &repository,
            &[
                "worktree",
                "move",
                original.to_str().unwrap(),
                moved.to_str().unwrap(),
            ],
        );

        let moved_git_dir = worktree_git_dir(&moved).unwrap();
        assert_eq!(moved_git_dir, git_dir);
        assert_eq!(
            read_topic(&moved_git_dir).unwrap().as_deref(),
            Some("tools-before-move")
        );
        write_topic(&moved_git_dir, Some("tools-after-move")).unwrap();
        assert_eq!(
            fs::read_to_string(git_dir.join("topic")).unwrap(),
            "tools-after-move\n"
        );
        write_topic(&moved_git_dir, None).unwrap();
        assert_eq!(read_topic(&moved_git_dir).unwrap(), None);
        write_topic(&moved_git_dir, None).unwrap();

        // A filesystem failure must remain an error, never an unowned slot or a
        // successful assignment. A directory blocks both reading and writing.
        fs::create_dir(git_dir.join("topic")).unwrap();
        assert!(read_topic(&moved_git_dir).is_err());
        let error = write_topic(&moved_git_dir, Some("tools-refused")).unwrap_err();
        assert!(error.contains("cannot update worktree topic"));
    }

    // A cloud slot still at the commit its restored build was published for
    // stays there, because that is the only commit the build is reused at.
    // Every other case keeps `origin/main`: a local session, a slot with no
    // restored build, a slot that has moved since, and a published commit that
    // main does not contain.
    #[test]
    fn a_cloud_slot_starts_at_its_restored_builds_commit() {
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fn git(root: &Path, arguments: &[&str]) -> String {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Worktree test",
                    "-c",
                    "user.email=worktree@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/nonexistent",
                ])
                .args(arguments)
                .current_dir(root)
                .stdin(Stdio::null())
                .output()
                .expect("run temporary repository command");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "omega-worktree-base-{}-{unique}",
            std::process::id(),
        )));
        let repository = scratch.0.join("repository");
        fs::create_dir_all(&repository).unwrap();
        git(&repository, &["init"]);
        git(&repository, &["commit", "--allow-empty", "-m", "published"]);
        let published = git(&repository, &["rev-parse", "HEAD"]);
        git(
            &repository,
            &["commit", "--allow-empty", "-m", "newer main"],
        );
        git(
            &repository,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        git(&repository, &["checkout", "--orphan", "elsewhere"]);
        git(
            &repository,
            &["commit", "--allow-empty", "-m", "not on main"],
        );
        let off_main = git(&repository, &["rev-parse", "HEAD"]);

        let slot = scratch.0.join("worker-1");
        fs::create_dir_all(slot.join("target")).unwrap();
        let manifest = slot.join("target/.warm-build.txt");
        let restore = |commit: &str| {
            fs::write(
                &manifest,
                format!("schema=omega-warm-build-v2\ncommit={commit}\n"),
            )
            .unwrap();
        };

        let base = |head: &str, cloud| acquire_base(&repository, &slot, head, cloud);
        assert_eq!(base(&published, true), "origin/main");
        restore(&published);
        assert_eq!(base(&published, true), published);
        assert_eq!(base(&published, false), "origin/main");
        // The slot moved after the restore, so its build is no longer the stamp's.
        assert_eq!(base(&off_main, true), "origin/main");
        restore(&off_main);
        assert_eq!(base(&off_main, true), "origin/main");
        restore("not-a-commit");
        assert_eq!(base("not-a-commit", true), "origin/main");

        assert_eq!(published_build_commit("slot=worker\n"), None);
        assert_eq!(published_build_commit("commit=abc123\n"), None);
    }

    // Only one pool command holds the lock at a time; the next one gets it once
    // the holder finishes, and a lock left by a killed process is broken.
    #[test]
    fn pool_lock_admits_one_holder_and_breaks_stale_locks() {
        let path = std::env::temp_dir().join(format!(
            "omega-worktree-pool-lock-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let hour = Duration::from_secs(3600);
        let held = lock_pool_at(&path, Duration::ZERO, hour).unwrap();
        let error = lock_pool_at(&path, Duration::from_millis(250), hour)
            .err()
            .expect("a second holder is refused");
        assert!(error.contains("another session holds"));
        drop(held);
        assert!(!path.exists());
        let held = lock_pool_at(&path, Duration::ZERO, hour).unwrap();
        std::mem::forget(held); // A killed holder never removes its lock.
        std::thread::sleep(Duration::from_millis(50));
        let broken = lock_pool_at(&path, Duration::ZERO, Duration::from_millis(10)).unwrap();
        drop(broken);
        assert!(!path.exists());
    }

    // A registration whose checkout directory is gone is reported as prunable, so
    // listing can skip it instead of failing on its missing Git directory.
    #[test]
    fn listing_marks_a_registration_whose_directory_is_gone() {
        let text = concat!(
            "worktree C:/repo\n",
            "HEAD aaa\n",
            "branch refs/heads/main\n",
            "\n",
            "worktree C:/repo/.agents/worktree/worker-1\n",
            "HEAD bbb\n",
            "detached\n",
            "locked in use\n",
            "\n",
            "worktree C:/Temp/session/scratchpad/wt-self\n",
            "HEAD ccc\n",
            "detached\n",
            "prunable gitdir file points to non-existent location\n",
        );
        let listed = |path: &str, head: &str, locked, prunable| ListedWorktree {
            path: path.into(),
            head: head.to_owned(),
            locked,
            prunable,
        };
        assert_eq!(
            parse_worktree_list(text),
            [
                listed("C:/repo", "aaa", false, false),
                listed("C:/repo/.agents/worktree/worker-1", "bbb", true, false),
                listed("C:/Temp/session/scratchpad/wt-self", "ccc", false, true),
            ]
        );
    }

    #[test]
    fn published_checkpoint_without_tracking_ref_is_retained() {
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fn git(root: &Path, arguments: &[&str]) -> String {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Worktree test",
                    "-c",
                    "user.email=worktree@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/nonexistent",
                ])
                .args(arguments)
                .current_dir(root)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "omega-worktree-publication-{}-{unique}",
            std::process::id(),
        )));
        fs::create_dir_all(&scratch.0).unwrap();
        let root = scratch.0.join("checkout");
        let origin = scratch.0.join("origin.git");
        git(&scratch.0, &["init", "--bare", origin.to_str().unwrap()]);
        git(&scratch.0, &["init", "-b", "main", root.to_str().unwrap()]);
        git(
            &root,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&root, &["commit", "--allow-empty", "-m", "main"]);
        git(&root, &["push", "origin", "main"]);
        git(&root, &["checkout", "--detach"]);
        git(&root, &["commit", "--allow-empty", "-m", "published"]);
        let published = git(&root, &["rev-parse", "HEAD"]);
        git(
            &root,
            &["push", "origin", "HEAD:refs/heads/work/checkpoint"],
        );
        // Reproduce a restored checkout that only retained origin/main.
        git(
            &root,
            &["update-ref", "-d", "refs/remotes/origin/work/checkpoint"],
        );
        assert!(has_unsaved_commits(&root, &published));
        fetch_published_head(&root, &published).unwrap();
        assert!(!has_unsaved_commits(&root, &published));
        assert_eq!(
            git(&root, &["rev-parse", "refs/remotes/origin/work/checkpoint"]),
            published
        );

        // A real unpublished descendant remains protected, and no checkout or
        // working file is changed by consulting the remote.
        git(&root, &["commit", "--allow-empty", "-m", "unpublished"]);
        let unpublished = git(&root, &["rev-parse", "HEAD"]);
        fs::write(root.join("keep"), "untracked work").unwrap();
        fetch_published_head(&root, &unpublished).unwrap();
        assert!(has_unsaved_commits(&root, &unpublished));
        assert_eq!(git(&root, &["rev-parse", "HEAD"]), unpublished);
        assert_eq!(
            fs::read_to_string(root.join("keep")).unwrap(),
            "untracked work"
        );

        git(
            &root,
            &[
                "remote",
                "set-url",
                "origin",
                origin.join("missing").to_str().unwrap(),
            ],
        );
        assert!(fetch_published_head(&root, &unpublished).is_err());
        assert!(has_unsaved_commits(&root, &unpublished));
    }

    // Every way a worktree can hold work blocks a reset; only an empty one passes.
    #[test]
    fn reset_is_refused_whenever_work_would_be_lost() {
        assert_eq!(reset_refusal(false, false, false), None);
        for (locked, dirty, unsaved) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            assert!(reset_refusal(locked, dirty, unsaved).is_some());
        }
    }
}

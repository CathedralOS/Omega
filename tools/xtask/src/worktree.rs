//! Manage the `.agents/worktree/worker-N` checkout pool (`cargo xtask worktree`).
//!
//! Why a fixed-path `worker-1`, `worker-2`, ... pool instead of task-named directories:
//! Cargo embeds absolute source paths inside `target/debug/.fingerprint/*` and `*.d`
//! dependency files. Moving or renaming a worktree directory invalidates those path
//! fingerprints and forces a full workspace rebuild, whereas keeping a fixed slot path
//! (`.agents/worktree/worker-1`) and resetting `HEAD` in place (`git checkout --detach
//! origin/main && git clean -fd`) preserves 100% of Cargo's incremental cache.
//!
//! Each slot's current `<lane>-<topic>` label is stored in `.git/worktrees/<slot>/topic`
//! so the checkout itself remains completely clean.
//!
//! Subcommands:
//! - `cargo xtask worktree status` — list all worktrees, their active topic, state, and cache.
//! - `cargo xtask worktree acquire [<lane>-<topic>]` — claim an idle `worker-N` slot (or
//!   recycle one into `worker-N`), reset it to `origin/main` in place, and print its path.
//! - `cargo xtask worktree release [worker-N]` — reset a finished slot to `origin/main`
//!   and clear its topic while keeping its warm `target/` cache.
//! - `cargo xtask worktree clean` — reset idle `worker-N` slots and remove legacy
//!   non-pool worktrees, orphaned directories, and stray log files in parallel.
//!
//! Several sessions share this pool, and a reset (`checkout --detach origin/main` plus
//! `clean -fd`) or a removal destroys whatever a worktree holds. So nothing is reset,
//! moved or removed unless [`reset_refusal`] finds it safe: not locked, no tracked or
//! untracked changes, and no commit that exists only in that worktree. A slot with a
//! topic belongs to the session that acquired it until `release` clears the topic, so
//! `acquire` and `clean` never take a claimed slot; only its own topic reuses it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::repo_root;

#[derive(Debug, Clone)]
struct WorktreeInfo {
    path: PathBuf,
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

fn topic_file(main_root: &Path, name: &str) -> PathBuf {
    main_root.join(".git/worktrees").join(name).join("topic")
}

fn read_topic(main_root: &Path, name: &str) -> Option<String> {
    let raw = fs::read_to_string(topic_file(main_root, name)).ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn write_topic(main_root: &Path, name: &str, topic: Option<&str>) {
    let path = topic_file(main_root, name);
    match topic {
        Some(t) if !t.is_empty() => {
            let _ = fs::write(path, format!("{t}\n"));
        }
        _ => {
            let _ = fs::remove_file(path);
        }
    }
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
    let checkout_ok = Command::new("git")
        .args(["checkout", "--detach", "origin/main"])
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

fn list_worktrees(main_root: &Path) -> Vec<WorktreeInfo> {
    let open_prs = open_pr_commits(main_root);
    let main_oid = rev_parse(main_root, "origin/main").unwrap_or_default();
    let Ok(out) = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(main_root)
        .stdin(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut result = Vec::new();
    let mut cur_path: Option<PathBuf> = None;
    let mut cur_head = String::new();
    let mut cur_locked = false;

    let flush =
        |path: Option<PathBuf>, head: &str, locked: bool, out_vec: &mut Vec<WorktreeInfo>| {
            let Some(p) = path else {
                return;
            };
            if p == main_root {
                return;
            }
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_owned();
            let slot_index = worker_slot_index(&name);
            let topic = read_topic(main_root, &name);
            let at_main = !head.is_empty() && head == main_oid;
            let open_pr = open_prs.contains(head);
            let has_warm_target = p.join("target/debug").is_dir();
            out_vec.push(WorktreeInfo {
                path: p,
                name,
                slot_index,
                topic,
                head: head.to_owned(),
                locked,
                at_main,
                open_pr,
                has_warm_target,
            });
        };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("worktree ") {
            flush(cur_path.take(), &cur_head, cur_locked, &mut result);
            cur_path = Some(PathBuf::from(rest.trim()));
            cur_head.clear();
            cur_locked = false;
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            cur_head = rest.trim().to_owned();
        } else if line.starts_with("locked") {
            cur_locked = true;
        }
    }
    flush(cur_path.take(), &cur_head, cur_locked, &mut result);
    result.sort_by_key(|w| {
        (
            w.slot_index.is_none(),
            w.slot_index.unwrap_or(usize::MAX),
            w.name.clone(),
        )
    });
    result
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
            let items = list_worktrees(&main_root);
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

            let items = list_worktrees(&main_root);

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
                if !reset_slot_to_main(&existing.path) {
                    return Err(1);
                }
                println!("{}", existing.path.display());
                return Ok(());
            }

            // 2. Reuse a released `worker-N` slot in place (preserving its fixed path and warm
            // `target/`). A slot with a topic is still claimed by another session.
            let mut idle_slots: Vec<&WorktreeInfo> = items
                .iter()
                .filter(|w| {
                    w.slot_index.is_some()
                        && w.topic.is_none()
                        && !w.open_pr
                        && w.name != current_name
                })
                .collect();
            idle_slots.sort_by_key(|w| (!w.has_warm_target, w.slot_index.unwrap_or(usize::MAX)));

            for slot in idle_slots {
                if refusal_for(&main_root, slot).is_some() {
                    continue;
                }
                if reset_slot_to_main(&slot.path) {
                    write_topic(&main_root, &slot.name, topic);
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
                if move_ok && reset_slot_to_main(&slot_path) {
                    write_topic(&main_root, &slot_name, topic);
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
            write_topic(&main_root, &slot_name, topic);
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
            let items = list_worktrees(&main_root);
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
            let _ = Command::new("git")
                .args(["fetch", "origin", "main"])
                .current_dir(&main_root)
                .stdin(Stdio::null())
                .status();
            if !reset_slot_to_main(&wt.path) {
                return Err(1);
            }
            write_topic(&main_root, &wt.name, None);
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

            let items = list_worktrees(&main_root);
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
                    write_topic(&main_root, "worker-1", None);
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
                            write_topic(&main_root, &wt.name, None);
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
                    let admin_dir = main_root.join(".git/worktrees").join(&wt.name);
                    let _ = fs::remove_dir_all(&admin_dir);
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
    use super::reset_refusal;

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

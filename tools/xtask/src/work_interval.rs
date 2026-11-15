//! A checkout owns the start of its current work, independently of remote refs.
//!
//! Acquisition records it automatically. Legacy active slots adopt a provable
//! local source transition once; an explicit warm handoff starts the next one.
//! Only source identity and time are retained here, never reported model counts.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const FILE: &str = "omega-work-interval.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkInterval {
    version: u32,
    source: String,
    started_epoch: i64,
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot inspect work interval source: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot inspect work interval source: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|error| format!("work interval source is not UTF-8: {error}"))
}

fn directory(root: &Path) -> Result<PathBuf, String> {
    let directory = PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"])?);
    if !directory.is_absolute() {
        return Err("work interval Git directory is not absolute".to_owned());
    }
    Ok(directory)
}

fn validate(root: &Path, interval: &WorkInterval) -> Result<(), String> {
    if interval.version != 1
        || interval.started_epoch <= 0
        || !matches!(interval.source.len(), 40 | 64)
        || !interval.source.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("invalid work interval source identity or version".to_owned());
    }
    let resolved = git(
        root,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", interval.source),
        ],
    )?;
    if resolved != interval.source {
        return Err("work interval source does not name its exact commit".to_owned());
    }
    Ok(())
}

fn write(root: &Path, interval: &WorkInterval) -> Result<(), String> {
    validate(root, interval)?;
    let path = directory(root)?.join(FILE);
    let bytes = serde_json::to_vec(interval)
        .map_err(|error| format!("cannot encode work interval: {error}"))?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("cannot read work interval clock: {error}"))?
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{unique}", std::process::id()));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot record work interval {}: {error}", path.display()))?;
    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "cannot publish work interval {}: {error}",
            path.display()
        ));
    }
    Ok(())
}

/// Start a new attribution interval without changing HEAD, files or pool lease.
pub(crate) fn begin(root: &Path) -> Result<(), String> {
    let source = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let started_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("cannot read work interval clock: {error}"))?
        .as_secs();
    let started_epoch = i64::try_from(started_epoch)
        .map_err(|_| "work interval clock is out of range".to_owned())?;
    write(
        root,
        &WorkInterval {
            version: 1,
            source,
            started_epoch,
        },
    )
}

pub(crate) fn clear(root: &Path) -> Result<(), String> {
    let path = directory(root)?.join(FILE);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot clear work interval {}: {error}",
            path.display()
        )),
    }
}

struct SourceTransition {
    old: String,
    source: String,
    epoch: i64,
    message: String,
}

fn transitions(directory: &Path) -> Result<Vec<SourceTransition>, String> {
    let text = fs::read_to_string(directory.join("logs/HEAD"))
        .map_err(|error| format!("cannot read checkout-local work history: {error}"))?;
    text.lines()
        .rev()
        .map(|line| {
            // Git initializes a linked worktree with a zero-old-ID record
            // whose message can be empty (and have no tab separator).
            let (identity, message) = line.split_once('\t').unwrap_or((line, ""));
            let fields = identity.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 5 {
                return Err("invalid checkout-local source transition".to_owned());
            }
            let epoch = fields[fields.len() - 2]
                .parse::<i64>()
                .map_err(|_| "invalid checkout-local source time")?;
            Ok(SourceTransition {
                old: fields[0].to_owned(),
                source: fields[1].to_owned(),
                epoch,
                message: message.to_owned(),
            })
        })
        .collect()
}

fn ancestor(root: &Path, source: &str, tip: &str) -> Result<bool, String> {
    let status = Command::new("git")
        .args(["merge-base", "--is-ancestor", source, tip])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot compare work interval sources: {error}"))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err("cannot compare work interval sources".to_owned()),
    }
}

fn direct_parent(root: &Path, transition: &SourceTransition) -> Result<bool, String> {
    let commit = git(
        root,
        &[
            "--no-replace-objects",
            "cat-file",
            "commit",
            &transition.source,
        ],
    )?;
    let mut parents = commit
        .lines()
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.strip_prefix("parent "));
    Ok(parents.next() == Some(transition.old.as_str()) && parents.next().is_none())
}

fn adopt(root: &Path, directory: &Path) -> Result<WorkInterval, String> {
    let history = transitions(directory)?;
    let head = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    if history.first().is_none_or(|entry| entry.source != head) {
        return Err("checkout-local history does not identify current HEAD".to_owned());
    }
    let mut position = 0;
    while let Some(entry) = history.get(position) {
        if entry.old.bytes().all(|byte| byte == b'0') {
            return Ok(WorkInterval {
                version: 1,
                source: entry.source.clone(),
                started_epoch: entry.epoch,
            });
        }
        if entry.message.starts_with("checkout:") {
            // A temporary baseline checkout followed by restoration is one
            // ongoing change, not two new outcomes.
            if let Some(previous) = history.get(position + 1)
                && previous.message.starts_with("checkout:")
                && previous.old == entry.source
                && previous.source == entry.old
            {
                position += 2;
                continue;
            }
            if entry.old == entry.source {
                return Err(
                    "ambiguous unchanged checkout; start the next outcome with `worktree begin`"
                        .to_owned(),
                );
            }
            // An unpaired related checkout may be either a source probe or a
            // warm new outcome. Git alone cannot establish that distinction.
            if ancestor(root, &entry.old, &entry.source)?
                || ancestor(root, &entry.source, &entry.old)?
            {
                return Err(
                    "ambiguous related checkout; restore the owned source or use `worktree begin`"
                        .to_owned(),
                );
            }
            return Ok(WorkInterval {
                version: 1,
                source: entry.source.clone(),
                started_epoch: entry.epoch,
            });
        } else if !entry.message.starts_with("commit")
            && !entry.message.starts_with("rebase")
            // A cherry-pick continues this work only when the new commit's
            // sole parent is the exact HEAD this checkout previously held.
            && !(entry.message.starts_with("cherry-pick:") && direct_parent(root, entry)?)
            && !(entry.message.starts_with("reset:") && entry.source == entry.old)
        {
            return Err(format!(
                "ambiguous source transition `{}`; use `worktree begin`",
                entry.message
            ));
        }
        position += 1;
    }
    Err("no provable checkout-local interval; use `worktree begin`".to_owned())
}

/// Reuse the persisted boundary. Neither remote refs nor rewritten topic commits
/// can move it. A missing legacy record is adopted once, never from remote main.
pub(crate) fn since_epoch(root: &Path) -> Result<i64, String> {
    let directory = directory(root)?;
    let path = directory.join(FILE);
    let interval = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<WorkInterval>(&bytes)
            .map_err(|error| format!("invalid work interval {}: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let interval = adopt(root, &directory)?;
            write(root, &interval)?;
            interval
        }
        Err(error) => {
            return Err(format!(
                "cannot read work interval {}: {error}",
                path.display()
            ));
        }
    };
    validate(root, &interval)?;
    Ok(interval.started_epoch)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct History {
        root: PathBuf,
        worker: PathBuf,
        picked: String,
        local: String,
    }

    impl History {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "omega-work-interval-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            git(&root, &["init", "--quiet"]).unwrap();
            for (key, value) in [
                ("user.name", "Work interval test"),
                ("user.email", "work-interval@example.invalid"),
                ("commit.gpgSign", "false"),
                ("core.hooksPath", ".git/empty-hooks"),
            ] {
                git(&root, &["config", key, value]).unwrap();
            }
            git(&root, &["commit", "--quiet", "--allow-empty", "-m", "base"]).unwrap();
            let base = git(&root, &["rev-parse", "HEAD"]).unwrap();
            fs::write(root.join("picked.txt"), "picked source\n").unwrap();
            git(&root, &["add", "picked.txt"]).unwrap();
            git(&root, &["commit", "--quiet", "-m", "picked change"]).unwrap();
            let picked = git(&root, &["rev-parse", "HEAD"]).unwrap();
            let worker = root.join("worker");
            git(
                &root,
                &[
                    "worktree",
                    "add",
                    "--quiet",
                    "--detach",
                    worker.to_str().unwrap(),
                    &base,
                ],
            )
            .unwrap();
            fs::write(worker.join("local.txt"), "local source\n").unwrap();
            git(&worker, &["add", "local.txt"]).unwrap();
            git(&worker, &["commit", "--quiet", "-m", "local change"]).unwrap();
            let local = git(&worker, &["rev-parse", "HEAD"]).unwrap();
            Self {
                root,
                worker,
                picked,
                local,
            }
        }
    }

    impl Drop for History {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn direct_parent_cherry_pick_preserves_the_acquired_interval() {
        let history = History::new();
        let directory = directory(&history.worker).unwrap();
        let before = adopt(&history.worker, &directory).unwrap();
        git(&history.worker, &["cherry-pick", &history.picked]).unwrap();
        let after = adopt(&history.worker, &directory).unwrap();
        assert_eq!(after.source, before.source);
        assert_eq!(after.started_epoch, before.started_epoch);
        assert_eq!(since_epoch(&history.worker).unwrap(), before.started_epoch);
        let persisted: WorkInterval =
            serde_json::from_slice(&fs::read(directory.join(FILE)).unwrap()).unwrap();
        assert_eq!(persisted.source, before.source);
        assert_eq!(persisted.started_epoch, before.started_epoch);
    }

    #[test]
    fn cherry_pick_label_does_not_authorize_a_nonparent_transition() {
        let history = History::new();
        // Let Git record a real source transition with a misleading action.
        // The picked commit's parent is the base, not this checkout's HEAD.
        git(
            &history.worker,
            &[
                "update-ref",
                "-m",
                "cherry-pick: substituted history",
                "HEAD",
                &history.picked,
                &history.local,
            ],
        )
        .unwrap();
        let Err(error) = adopt(&history.worker, &directory(&history.worker).unwrap()) else {
            panic!("the source transition must reject");
        };
        assert!(
            error.contains("ambiguous source transition `cherry-pick:"),
            "{error}"
        );
    }

    #[test]
    fn unrelated_checkout_retains_its_existing_source_boundary() {
        let history = History::new();
        git(
            &history.worker,
            &["checkout", "--quiet", "--detach", &history.picked],
        )
        .unwrap();
        let interval = adopt(&history.worker, &directory(&history.worker).unwrap()).unwrap();
        assert_eq!(interval.source, history.picked);
    }
}

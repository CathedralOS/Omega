//! Select the checkout being operated on, not the checkout that built xtask.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn current() -> Result<PathBuf, String> {
    let directory = std::env::current_dir()
        .map_err(|error| format!("cannot read invocation directory: {error}"))?;
    enclosing(&directory)
}

pub(crate) fn common_git_dir(repository: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(repository)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot locate shared Git metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot locate shared Git metadata for {}: {}",
            repository.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let directory = String::from_utf8(output.stdout)
        .map_err(|error| format!("shared Git directory is not UTF-8: {error}"))?;
    // Remove the command's line ending, preserving spaces in the real path.
    let directory = directory.strip_suffix('\n').unwrap_or(&directory);
    #[cfg(windows)]
    let directory = directory.strip_suffix('\r').unwrap_or(directory);
    let directory = Path::new(directory);
    if !directory.is_absolute() {
        return Err("Git did not return an absolute shared metadata directory".to_owned());
    }
    fs::canonicalize(directory)
        .map_err(|error| format!("cannot resolve shared Git metadata: {error}"))
}

fn open_fetch_lock(repository: &Path) -> Result<File, String> {
    let path = common_git_dir(repository)?.join("omega-git-fetch.lock");
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| {
            format!(
                "cannot open shared Git fetch lock {}: {error}",
                path.display()
            )
        })
}

/// Fetches update shared metadata even when invoked from different worktrees:
/// two depth-one readers can otherwise race Git's common shallow file. Keep
/// the native lock across the Git child only, outside the worktree pool lock.
/// The file stays in place; the OS releases ownership on drop or process exit.
pub(crate) fn with_git_fetch_lock<T>(
    repository: &Path,
    fetch: impl FnOnce() -> io::Result<T>,
) -> Result<T, String> {
    let lock = open_fetch_lock(repository)?;
    lock.lock().map_err(|error| {
        format!(
            "cannot lock shared Git fetch metadata for {}: {error}",
            repository.display()
        )
    })?;
    fetch().map_err(|error| format!("git fetch failed: {error}"))
}

fn enclosing(directory: &Path) -> Result<PathBuf, String> {
    // Worktrees nest under the primary checkout, so the nearest complete
    // workspace wins. Repository contents identify source archives too; neither
    // a .git directory nor the binary's installation location is required.
    for candidate in directory.ancestors() {
        let mut workspace = true;
        for marker in [
            "Cargo.toml",
            "tools/xtask/Cargo.toml",
            "omega-rust/pipeline.md",
        ] {
            let marker = candidate.join(marker);
            match std::fs::metadata(&marker) {
                Ok(metadata) if metadata.is_file() => {}
                Ok(_) => {
                    workspace = false;
                    break;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    workspace = false;
                    break;
                }
                Err(error) => {
                    return Err(format!("cannot inspect {}: {error}", marker.display()));
                }
            }
        }
        if workspace {
            return Ok(candidate.to_owned());
        }
        // An incomplete or unrelated nested checkout must not redirect the
        // operation into an enclosing primary checkout.
        let git = candidate.join(".git");
        match std::fs::metadata(&git) {
            Ok(_) => {
                return Err(format!(
                    "Git checkout {} is not an Omega workspace",
                    candidate.display()
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect {}: {error}", git.display())),
        }
    }
    Err(format!(
        "no Omega workspace contains {}; run from the intended checkout or use the command's explicit repository option",
        directory.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Checkouts(PathBuf);

    impl Checkouts {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir()
                .join(format!("omega-xtask-roots-{}-{unique}", std::process::id()));
            fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }

        fn checkout(&self, relative: &str) -> PathBuf {
            let root = self.0.join(relative);
            fs::create_dir_all(root.join("tools/xtask/src")).unwrap();
            fs::create_dir_all(root.join("omega-rust")).unwrap();
            for marker in [
                "Cargo.toml",
                "tools/xtask/Cargo.toml",
                "omega-rust/pipeline.md",
            ] {
                fs::write(root.join(marker), "").unwrap();
            }
            root
        }
    }

    impl Drop for Checkouts {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn root_follows_the_invoked_checkout_instead_of_another_checkout() {
        let checkouts = Checkouts::new();
        let first = checkouts.checkout("first");
        let second = checkouts.checkout("second");
        assert_eq!(enclosing(&first).unwrap(), first);
        assert_eq!(enclosing(&second.join("tools/xtask/src")).unwrap(), second);
    }

    #[test]
    fn nested_worktree_wins_over_its_primary_checkout() {
        let checkouts = Checkouts::new();
        checkouts.checkout("primary");
        let worker = checkouts.checkout("primary/.agents/worktree/worker-1");
        assert_eq!(enclosing(&worker.join("omega-rust")).unwrap(), worker);
    }

    #[test]
    fn outside_or_incomplete_workspace_does_not_fall_back_to_the_build_checkout() {
        let checkouts = Checkouts::new();
        checkouts.checkout("unrelated");
        fs::write(checkouts.0.join("Cargo.toml"), "[workspace]").unwrap();
        let error = enclosing(&checkouts.0).unwrap_err();
        assert!(error.contains("no Omega workspace contains"), "{error}");
        assert!(
            error.contains(&checkouts.0.display().to_string()),
            "{error}"
        );
    }

    #[test]
    fn incomplete_nested_checkout_does_not_escape_to_the_primary() {
        let checkouts = Checkouts::new();
        let primary = checkouts.checkout("primary");
        let nested = primary.join(".agents/worktree/incomplete");
        fs::create_dir_all(nested.join("src")).unwrap();
        fs::write(nested.join(".git"), "gitdir: unused by this resolver").unwrap();
        let error = enclosing(&nested.join("src")).unwrap_err();
        assert!(error.contains("is not an Omega workspace"), "{error}");
        assert!(error.contains(&nested.display().to_string()), "{error}");
    }
}

#[cfg(test)]
mod fetch_tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Repositories(PathBuf);

    impl Repositories {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "omega-fetch-{}-{nonce}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn repository(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).unwrap();
            git(&path, &["init", "-q"]);
            git(
                &path,
                &[
                    "-c",
                    "user.name=test",
                    "-c",
                    "user.email=test@invalid",
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    "seed",
                ],
            );
            path
        }

        fn linked(&self, source: &Path, name: &str) -> PathBuf {
            let path = self.0.join(name);
            git(
                source,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    path.to_str().unwrap(),
                    "HEAD",
                ],
            );
            path
        }
    }

    impl Drop for Repositories {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn git(repository: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(repository)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn probe_lock(repository: &Path, blocked: bool) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "repository::fetch_tests::fetch_lock_probe_child",
                "--nocapture",
            ])
            .env("OMEGA_XTASK_FETCH_LOCK_TEST_REPOSITORY", repository)
            .env(
                "OMEGA_XTASK_FETCH_LOCK_TEST_BLOCKED",
                if blocked { "yes" } else { "no" },
            )
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fetch-lock child: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[ignore = "subprocess entry point driven by the ownership test"]
    fn fetch_lock_probe_child() {
        let repository =
            PathBuf::from(std::env::var_os("OMEGA_XTASK_FETCH_LOCK_TEST_REPOSITORY").unwrap());
        let lock = open_fetch_lock(&repository).unwrap();
        let result = lock.try_lock();
        if std::env::var("OMEGA_XTASK_FETCH_LOCK_TEST_BLOCKED").unwrap() == "yes" {
            assert!(
                matches!(result, Err(std::fs::TryLockError::WouldBlock)),
                "{result:?}"
            );
        } else {
            result.unwrap();
        }
    }

    #[test]
    fn fetch_ownership_is_shared_by_worktrees_and_released_on_error() {
        let repos = Repositories::new();
        let main = repos.repository("shared repository");
        let linked = repos.linked(&main, "linked worker");
        let independent = repos.repository("independent repository");
        assert_eq!(
            common_git_dir(&main).unwrap(),
            common_git_dir(&linked).unwrap()
        );
        let error = with_git_fetch_lock(&main, || {
            probe_lock(&linked, true);
            probe_lock(&independent, false);
            Err::<(), _>(io::Error::other("planned fetch failure"))
        })
        .unwrap_err();
        assert!(error.contains("planned fetch failure"), "{error}");
        probe_lock(&linked, false);
        assert!(
            common_git_dir(&main)
                .unwrap()
                .join("omega-git-fetch.lock")
                .is_file()
        );
    }

    fn fetch_exact(repository: &Path, version: &str) {
        let output = with_git_fetch_lock(repository, || {
            Command::new("git")
                .args(["fetch", "-q", "--depth=1", "origin", version])
                .current_dir(repository)
                .stdin(Stdio::null())
                .output()
        })
        .unwrap();
        assert!(
            output.status.success(),
            "snapshot {version}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn next_commit(repository: &Path, tree: &str, parent: &str, label: &str) -> String {
        git(
            repository,
            &[
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@invalid",
                "commit-tree",
                tree,
                "-p",
                parent,
                "-m",
                label,
            ],
        )
    }

    #[test]
    fn linked_worktrees_fetch_distinct_shallow_snapshots_concurrently() {
        let repos = Repositories::new();
        let seed = repos.repository("seed");
        let origin = repos.0.join("origin.git");
        git(
            &repos.0,
            &[
                "clone",
                "--bare",
                "-q",
                seed.to_str().unwrap(),
                origin.to_str().unwrap(),
            ],
        );
        let main = repos.0.join("reader");
        fs::create_dir(&main).unwrap();
        git(&main, &["init", "-q"]);
        git(
            &main,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        let mut left_version = git(&origin, &["rev-parse", "HEAD"]);
        let mut right_version = left_version.clone();
        fetch_exact(&main, &left_version);
        git(&main, &["checkout", "--detach", "-q", "FETCH_HEAD"]);
        let left = repos.linked(&main, "left worker");
        let right = repos.linked(&main, "right worker");
        let tree = git(&origin, &["rev-parse", "HEAD^{tree}"]);
        for round in 0..4 {
            let left_label = format!("left-{round}");
            let right_label = format!("right-{round}");
            left_version = next_commit(&origin, &tree, &left_version, &left_label);
            right_version = next_commit(&origin, &tree, &right_version, &right_label);
            git(&origin, &["update-ref", "refs/heads/left", &left_version]);
            git(&origin, &["update-ref", "refs/heads/right", &right_version]);
            std::thread::scope(|scope| {
                let start = Arc::new(Barrier::new(3));
                fn run(repository: &Path, version: &str, label: &str, start: Arc<Barrier>) {
                    start.wait();
                    fetch_exact(repository, version);
                    assert_eq!(
                        git(repository, &["show", "-s", "--format=%s", version]),
                        label
                    );
                }
                let left_start = Arc::clone(&start);
                let right_start = Arc::clone(&start);
                scope.spawn(|| run(&left, &left_version, &left_label, left_start));
                scope.spawn(|| run(&right, &right_version, &right_label, right_start));
                start.wait();
            });
        }
        assert!(common_git_dir(&main).unwrap().join("shallow").is_file());
    }
}

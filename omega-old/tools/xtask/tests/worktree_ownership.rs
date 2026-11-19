//! Exercise automatic reuse against scratch repositories, never live workers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const ORIGINAL: &str = "source owned by the assigned session\n";
const CURRENT: &str = "later published main\n";
const LEGACY_TOPIC: &str = "psi-existing-work";
const POOL_TOPIC: &str = "psi-pool-work";

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Worktree ownership fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/nonexistent",
        ])
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .expect("run scratch repository command");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

struct Repository {
    scratch: PathBuf,
    root: PathBuf,
    legacy: PathBuf,
    legacy_admin: PathBuf,
    pool: Option<PathBuf>,
    original: String,
}

impl Repository {
    fn new(with_pool: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = std::env::temp_dir().join(format!(
            "omega-legacy-ownership-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = scratch.join("repository");
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        let hooks = scratch.join("empty-hooks");
        fs::create_dir(&hooks).unwrap();
        git(
            &root,
            &["config", "core.hooksPath", hooks.to_str().unwrap()],
        );
        for marker in [
            "Cargo.toml",
            "tools/xtask/Cargo.toml",
            "omega-rust/pipeline.md",
        ] {
            let path = root.join(marker);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        fs::write(root.join(".gitignore"), "/.agents/\n/target/\n").unwrap();
        fs::write(root.join("payload"), ORIGINAL).unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-qm", "original source"]);
        let original = git(&root, &["rev-parse", "HEAD"]);
        let remote = scratch.join("remote.git");
        git(&root, &["init", "--bare", "-q", remote.to_str().unwrap()]);
        git(
            &root,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&root, &["push", "-q", "-u", "origin", "main"]);

        let worktrees = root.join(".agents/worktree");
        let legacy = worktrees.join("older-topic");
        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                legacy.to_str().unwrap(),
                &original,
            ],
        );
        let legacy_admin = PathBuf::from(git(&legacy, &["rev-parse", "--absolute-git-dir"]));
        fs::write(legacy_admin.join("topic"), format!("{LEGACY_TOPIC}\n")).unwrap();
        fs::create_dir_all(legacy.join("target/debug")).unwrap();
        fs::write(legacy.join("target/debug/keep"), "warm cache").unwrap();

        let pool = with_pool.then(|| {
            let pool = worktrees.join("worker-1");
            git(
                &root,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "--detach",
                    pool.to_str().unwrap(),
                    &original,
                ],
            );
            let admin = PathBuf::from(git(&pool, &["rev-parse", "--absolute-git-dir"]));
            fs::write(admin.join("topic"), format!("{POOL_TOPIC}\n")).unwrap();
            pool
        });

        fs::write(root.join("payload"), CURRENT).unwrap();
        git(&root, &["add", "payload"]);
        git(&root, &["commit", "-qm", "later main"]);
        git(&root, &["push", "-q", "origin", "main"]);
        assert!(git(&legacy, &["status", "--porcelain"]).is_empty());
        Self {
            scratch,
            root,
            legacy,
            legacy_admin,
            pool,
            original,
        }
    }

    fn run(&self, current: &Path, arguments: &[&str]) -> Output {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("worktree")
            .args(arguments)
            .current_dir(current)
            .env_remove("GH_REPO")
            .env("GH_PROMPT_DISABLED", "1")
            .env("CLAUDE_CODE_REMOTE", "")
            .env("CODEX_ENVIRONMENT_ID", "")
            .stdin(Stdio::null())
            .output()
            .expect("run worktree command in scratch repository");
        assert!(
            output.status.success(),
            "worktree {arguments:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn assert_assigned_legacy_untouched(&self) {
        assert_eq!(
            fs::read_to_string(self.legacy.join("payload")).unwrap(),
            ORIGINAL
        );
        assert_eq!(git(&self.legacy, &["rev-parse", "HEAD"]), self.original);
        assert_eq!(
            fs::read_to_string(self.legacy_admin.join("topic")).unwrap(),
            format!("{LEGACY_TOPIC}\n")
        );
        assert_eq!(
            fs::read_to_string(self.legacy.join("target/debug/keep")).unwrap(),
            "warm cache"
        );
    }

    fn assert_other_pool_untouched(&self) {
        if let Some(pool) = &self.pool {
            assert_eq!(fs::read_to_string(pool.join("payload")).unwrap(), ORIGINAL);
            assert_eq!(git(pool, &["rev-parse", "HEAD"]), self.original);
            let admin = PathBuf::from(git(pool, &["rev-parse", "--absolute-git-dir"]));
            assert_eq!(
                fs::read_to_string(admin.join("topic")).unwrap(),
                format!("{POOL_TOPIC}\n")
            );
        }
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.scratch);
    }
}

#[test]
fn assigned_legacy_checkout_survives_acquisition_and_cleanup() {
    for (arguments, with_pool) in [
        (vec!["acquire", "repo-next"], true),
        (vec!["clean"], true),
        (vec!["clean"], false),
    ] {
        let repository = Repository::new(with_pool);
        repository.run(&repository.root, &arguments);
        repository.assert_assigned_legacy_untouched();
        repository.assert_other_pool_untouched();
        if !with_pool {
            assert!(!repository.root.join(".agents/worktree/worker-1").exists());
        }
    }
}

#[test]
fn released_legacy_checkout_still_migrates_or_is_removed() {
    for acquire in [true, false] {
        let repository = Repository::new(true);
        fs::remove_file(repository.legacy_admin.join("topic")).unwrap();
        if acquire {
            let output = repository.run(&repository.root, &["acquire", "repo-next"]);
            let text = String::from_utf8(output.stdout).unwrap();
            let selected = PathBuf::from(text.lines().last().unwrap());
            assert_eq!(selected, repository.root.join(".agents/worktree/worker-2"));
            assert_eq!(
                fs::read_to_string(selected.join("payload")).unwrap(),
                CURRENT
            );
            assert_eq!(
                fs::read_to_string(selected.join("target/debug/keep")).unwrap(),
                "warm cache"
            );
        } else {
            repository.run(&repository.root, &["clean"]);
        }
        assert!(!repository.legacy.exists());
        repository.assert_other_pool_untouched();
    }
}

#[test]
fn resuming_the_same_pool_topic_still_resets_its_published_source() {
    let repository = Repository::new(true);
    let pool = repository.pool.as_ref().unwrap();
    repository.run(pool, &["acquire", POOL_TOPIC]);
    assert_eq!(fs::read_to_string(pool.join("payload")).unwrap(), CURRENT);
    repository.assert_assigned_legacy_untouched();
}

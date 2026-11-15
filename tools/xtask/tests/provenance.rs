//! An active thread's audit must not change when another session writes a transcript.

use std::fs::{self, File, FileTimes};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ACTIVE_THREAD: &str = "11111111-1111-4111-8111-111111111111";
const OTHER_THREAD: &str = "22222222-2222-4222-8222-222222222222";

struct Transcripts(PathBuf);

impl Transcripts {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home =
            std::env::temp_dir().join(format!("omega-provenance-{}-{unique}", std::process::id()));
        fs::create_dir_all(home.join(".codex/sessions/2026/11/14")).unwrap();
        Self(home)
    }

    fn write(
        &self,
        label: &str,
        file_thread: &str,
        recorded_thread: &str,
        model: &str,
        epoch: u64,
    ) -> PathBuf {
        let path = self.0.join(format!(
            ".codex/sessions/2026/11/14/rollout-{label}-{file_thread}.jsonl"
        ));
        let header = serde_json::json!({
            "type": "session_meta", "payload": { "id": recorded_thread }
        });
        let context = serde_json::json!({
            "type": "turn_context", "payload": { "model": model, "effort": "high" }
        });
        let usage = serde_json::json!({
            "type": "event_msg", "payload": {
                "type": "token_count", "info": { "last_token_usage": {
                    "input_tokens": 10, "cached_input_tokens": 2, "output_tokens": 1
                }}
            }
        });
        fs::write(&path, format!("{header}\n{context}\n{usage}\n")).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(epoch)))
            .unwrap();
        path
    }

    fn audit(&self, thread: Option<&str>, explicit: Option<&Path>) -> Output {
        self.audit_options(thread, explicit, false)
    }

    fn audit_options(&self, thread: Option<&str>, explicit: Option<&Path>, allow: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(["provenance", "--full-session"])
            .env("USERPROFILE", &self.0)
            .env("HOME", &self.0)
            .env("CODEX_CI", "1")
            .stdin(Stdio::null());
        for name in [
            "ANTIGRAVITY_CONVERSATION_ID",
            "ANTIGRAVITY_AGENT",
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CODEX_THREAD_ID",
        ] {
            command.env_remove(name);
        }
        if let Some(thread) = thread {
            command.env("CODEX_THREAD_ID", thread);
        }
        if let Some(path) = explicit {
            command.arg("--transcript").arg(path);
        }
        if allow {
            command.arg("--allow-unavailable");
        }
        command.output().unwrap()
    }
}

impl Drop for Transcripts {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn active_thread_ignores_newer_foreign_and_mislabeled_transcripts() {
    let transcripts = Transcripts::new();
    transcripts.write("active", ACTIVE_THREAD, ACTIVE_THREAD, "active-model", 1);
    transcripts.write("other", OTHER_THREAD, OTHER_THREAD, "other-model", 2);
    transcripts.write(
        "mislabeled",
        ACTIVE_THREAD,
        OTHER_THREAD,
        "mislabeled-model",
        3,
    );
    let output = transcripts.audit(Some(ACTIVE_THREAD), None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let audit = String::from_utf8(output.stdout).unwrap();
    assert!(
        audit.contains("Agent: Codex | Model: active-model |"),
        "{audit}"
    );
    assert!(audit.contains("Tokens: 10 in (2 cached), 1 out"), "{audit}");
}

#[test]
fn missing_active_thread_refuses_instead_of_auditing_another_session() {
    let transcripts = Transcripts::new();
    transcripts.write("other", OTHER_THREAD, OTHER_THREAD, "other-model", 2);
    let output = transcripts.audit(Some(ACTIVE_THREAD), None);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains(ACTIVE_THREAD) && error.contains("--transcript"),
        "{error}"
    );
}

#[test]
fn codex_without_thread_identity_keeps_newest_transcript_fallback() {
    let transcripts = Transcripts::new();
    transcripts.write("active", ACTIVE_THREAD, ACTIVE_THREAD, "active-model", 1);
    transcripts.write("other", OTHER_THREAD, OTHER_THREAD, "newest-model", 2);
    let output = transcripts.audit(None, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let audit = String::from_utf8(output.stdout).unwrap();
    assert!(audit.contains("Model: newest-model |"), "{audit}");
}

#[test]
fn explicit_transcript_overrides_active_thread_selection() {
    let transcripts = Transcripts::new();
    let explicit = transcripts.write("other", OTHER_THREAD, OTHER_THREAD, "explicit-model", 2);
    let output = transcripts.audit(Some(ACTIVE_THREAD), Some(&explicit));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let audit = String::from_utf8(output.stdout).unwrap();
    assert!(audit.contains("Model: explicit-model |"), "{audit}");
}

#[test]
fn unavailable_opt_in_never_uses_foreign_metrics() {
    let transcripts = Transcripts::new();
    transcripts.write("other", OTHER_THREAD, OTHER_THREAD, "other-model", 2);
    let output = transcripts.audit_options(Some(ACTIVE_THREAD), None, true);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "Agent: Codex | Model: unavailable | Effort: unavailable | Tokens: unavailable in (unavailable cached), unavailable out | Steps: unavailable | Time: unavailable | Steers: unavailable"
    );
}

#[test]
fn unavailable_opt_in_preserves_real_matching_metrics() {
    let transcripts = Transcripts::new();
    transcripts.write("active", ACTIVE_THREAD, ACTIVE_THREAD, "active-model", 1);
    transcripts.write("other", OTHER_THREAD, OTHER_THREAD, "other-model", 2);
    let output = transcripts.audit_options(Some(ACTIVE_THREAD), None, true);
    assert!(output.status.success());
    let audit = String::from_utf8(output.stdout).unwrap();
    assert!(
        audit.contains("Agent: Codex | Model: active-model |"),
        "{audit}"
    );
    assert!(audit.contains("Tokens: 10 in (2 cached), 1 out"), "{audit}");
    assert!(!audit.contains("unavailable"), "{audit}");
}

#[test]
fn explicit_invalid_transcript_refuses_even_with_unavailable_opt_in() {
    let transcripts = Transcripts::new();
    let invalid = transcripts.0.join("missing.jsonl");
    for allow in [false, true] {
        let output = transcripts.audit_options(Some(ACTIVE_THREAD), Some(&invalid), allow);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("cannot read explicit transcript")
        );
    }
}

#[test]
fn unreadable_active_transcript_requires_unavailable_opt_in() {
    let transcripts = Transcripts::new();
    let path = transcripts.write("active", ACTIVE_THREAD, ACTIVE_THREAD, "active-model", 1);
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(0xff);
    fs::write(path, bytes).unwrap();
    let strict = transcripts.audit(Some(ACTIVE_THREAD), None);
    assert_eq!(strict.status.code(), Some(2));
    assert!(strict.stdout.is_empty());
    assert!(
        String::from_utf8(strict.stderr)
            .unwrap()
            .contains("cannot read active Codex transcript")
    );
    let allowed = transcripts.audit_options(Some(ACTIVE_THREAD), None, true);
    assert!(allowed.status.success());
    assert!(
        String::from_utf8(allowed.stdout)
            .unwrap()
            .contains("Model: unavailable")
    );
}

struct WorktreeInterval {
    transcripts: Transcripts,
    repository: PathBuf,
    worker: PathBuf,
    git_dir: PathBuf,
    base: String,
}

impl WorktreeInterval {
    fn git(root: &Path, arguments: &[&str], date: &str) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Interval test",
                "-c",
                "user.email=interval@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=",
                "-c",
                "core.autocrlf=false",
            ])
            .args(arguments)
            .current_dir(root)
            .stdin(Stdio::null())
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .unwrap_or_else(|error| panic!("cannot run Git in {}: {error}", root.display()));
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn new() -> Self {
        let transcripts = Transcripts::new();
        let repository = transcripts.0.join("repository");
        fs::create_dir_all(repository.join("tools/xtask")).unwrap();
        fs::create_dir_all(repository.join("omega-rust")).unwrap();
        for marker in [
            "Cargo.toml",
            "tools/xtask/Cargo.toml",
            "omega-rust/pipeline.md",
        ] {
            fs::write(repository.join(marker), "interval fixture\n").unwrap();
        }
        fs::write(repository.join(".gitignore"), "target/\n").unwrap();
        let early = "2026-01-01T00:00:00Z";
        Self::git(&repository, &["init", "-b", "main"], early);
        Self::git(&repository, &["add", "."], early);
        Self::git(&repository, &["commit", "-m", "initial"], early);
        let initial = Self::git(&repository, &["rev-parse", "HEAD"], early);
        Self::git(
            &repository,
            &["commit", "--allow-empty", "-m", "previous outcome"],
            early,
        );
        let previous = Self::git(&repository, &["rev-parse", "HEAD"], early);
        Self::git(&repository, &["checkout", "--detach", &initial], early);
        Self::git(
            &repository,
            &["commit", "--allow-empty", "-m", "peer main"],
            early,
        );
        let base = Self::git(&repository, &["rev-parse", "HEAD"], early);
        Self::git(
            &repository,
            &["update-ref", "refs/remotes/origin/main", &base],
            early,
        );
        let worker = repository.join(".agents/worktree/worker-22");
        Self::git(
            &repository,
            &[
                "worktree",
                "add",
                "--detach",
                worker.to_str().unwrap(),
                &previous,
            ],
            early,
        );
        Self::git(
            &worker,
            &["checkout", "--detach", &base],
            "2026-01-02T00:00:00Z",
        );
        let git_dir = PathBuf::from(Self::git(
            &worker,
            &["rev-parse", "--absolute-git-dir"],
            early,
        ));
        fs::write(git_dir.join("topic"), "tools-owned-lease\n").unwrap();
        Self {
            transcripts,
            repository,
            worker,
            git_dir,
            base,
        }
    }

    fn transcript(&self, current: Option<&str>) {
        let path =
            self.transcripts
                .write("active", ACTIVE_THREAD, ACTIVE_THREAD, "interval-model", 1);
        let header = serde_json::json!({"type":"session_meta", "payload":{"id":ACTIVE_THREAD}});
        let context = serde_json::json!({"type":"turn_context", "timestamp":"2026-01-01T00:00:00Z", "payload":{"model":"interval-model", "effort":"high"}});
        let usage = |timestamp, tokens| serde_json::json!({"type":"event_msg", "timestamp":timestamp, "payload":{"type":"token_count", "info":{"last_token_usage":{"input_tokens":tokens,"cached_input_tokens":2,"output_tokens":1}}}});
        let mut records = vec![header, context, usage("2026-01-01T00:00:00Z", 100)];
        if let Some(current) = current {
            records.push(usage(current, 10));
        }
        let bytes = records
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, bytes).unwrap();
    }

    fn append_usage(&self, timestamp: &str, tokens: u64) {
        let path = self.transcripts.0.join(format!(
            ".codex/sessions/2026/11/14/rollout-active-{ACTIVE_THREAD}.jsonl"
        ));
        let usage = serde_json::json!({"type":"event_msg", "timestamp":timestamp, "payload":{"type":"token_count", "info":{"last_token_usage":{"input_tokens":tokens,"cached_input_tokens":2,"output_tokens":1}}}});
        writeln!(
            File::options().append(true).open(path).unwrap(),
            "\n{usage}"
        )
        .unwrap();
    }

    fn command(&self, root: &Path, arguments: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .args(arguments)
            .current_dir(root)
            .stdin(Stdio::null())
            .env("USERPROFILE", &self.transcripts.0)
            .env("HOME", &self.transcripts.0)
            .env("CODEX_THREAD_ID", ACTIVE_THREAD);
        for name in [
            "ANTIGRAVITY_CONVERSATION_ID",
            "ANTIGRAVITY_AGENT",
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
        ] {
            command.env_remove(name);
        }
        command.output().unwrap()
    }

    fn audit(&self) -> String {
        let output = self.command(&self.worker, &["provenance"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

#[test]
fn legacy_interval_survives_peer_main_checkpoints_and_rebase_without_prior_outcomes() {
    let fixture = WorktreeInterval::new();
    fixture.transcript(Some("2026-01-03T00:00:00Z"));
    let ancestor = WorktreeInterval::git(
        &fixture.worker,
        &["rev-parse", "HEAD^"],
        "2026-01-02T01:00:00Z",
    );
    WorktreeInterval::git(
        &fixture.worker,
        &["checkout", "--detach", &ancestor],
        "2026-01-02T01:00:00Z",
    );
    WorktreeInterval::git(
        &fixture.worker,
        &["checkout", "--detach", &fixture.base],
        "2026-01-02T02:00:00Z",
    );
    fs::write(fixture.worker.join("owned-wip"), "preserved").unwrap();
    let audit = fixture.audit();
    assert!(audit.contains("Tokens: 10 in (2 cached), 1 out"), "{audit}");
    let interval = fs::read(fixture.git_dir.join("omega-work-interval.json")).unwrap();
    assert_eq!(
        fs::read_to_string(fixture.worker.join("owned-wip")).unwrap(),
        "preserved"
    );
    WorktreeInterval::git(
        &fixture.repository,
        &["commit", "--allow-empty", "-m", "advance only remote main"],
        "2026-01-04T00:00:00Z",
    );
    let peer = WorktreeInterval::git(
        &fixture.repository,
        &["rev-parse", "HEAD"],
        "2026-01-04T00:00:00Z",
    );
    WorktreeInterval::git(
        &fixture.repository,
        &["update-ref", "refs/remotes/origin/main", &peer],
        "2026-01-04T00:00:00Z",
    );
    assert_eq!(
        fixture.audit(),
        audit,
        "a peer fetch must not change one interval's counts"
    );
    WorktreeInterval::git(
        &fixture.worker,
        &["add", "owned-wip"],
        "2026-01-04T00:00:00Z",
    );
    WorktreeInterval::git(
        &fixture.worker,
        &["commit", "-m", "checkpoint"],
        "2026-01-04T00:00:00Z",
    );
    assert_eq!(fixture.audit(), audit);
    WorktreeInterval::git(&fixture.worker, &["rebase", &peer], "2026-01-05T00:00:00Z");
    assert_eq!(fixture.audit(), audit);
    assert_eq!(
        fs::read(fixture.git_dir.join("omega-work-interval.json")).unwrap(),
        interval
    );
    fs::remove_file(fixture.git_dir.join("omega-work-interval.json")).unwrap();
    assert_eq!(
        fixture.audit(),
        audit,
        "legacy migration after rebase keeps the original source boundary"
    );
    let full = fixture.command(&fixture.worker, &["provenance", "--full-session"]);
    assert!(
        String::from_utf8(full.stdout)
            .unwrap()
            .contains("Tokens: 110 in (4 cached), 2 out")
    );
}

#[test]
fn warm_next_outcome_and_fresh_acquisition_record_without_resetting_source_cache_or_lease() {
    let fixture = WorktreeInterval::new();
    fixture.transcript(Some("2026-01-03T00:00:00Z"));
    fixture.audit();
    let old = fs::read(fixture.git_dir.join("omega-work-interval.json")).unwrap();
    fs::create_dir_all(fixture.worker.join("target")).unwrap();
    fs::write(fixture.worker.join("target/warm"), "cache").unwrap();
    fs::write(fixture.worker.join("dirty"), "do not reset").unwrap();
    let refused = fixture.command(&fixture.worker, &["worktree", "begin"]);
    assert!(!refused.status.success());
    assert_eq!(
        fs::read(fixture.git_dir.join("omega-work-interval.json")).unwrap(),
        old
    );
    assert_eq!(
        fs::read_to_string(fixture.worker.join("dirty")).unwrap(),
        "do not reset"
    );
    fs::remove_file(fixture.worker.join("dirty")).unwrap();
    let begun = fixture.command(&fixture.worker, &["worktree", "begin"]);
    assert!(
        begun.status.success(),
        "{}",
        String::from_utf8_lossy(&begun.stderr)
    );
    fixture.append_usage("2099-01-03T00:00:00Z", 20);
    assert!(fixture.audit().contains("Tokens: 20 in (2 cached), 1 out"));
    assert_eq!(
        WorktreeInterval::git(
            &fixture.worker,
            &["rev-parse", "HEAD"],
            "2026-01-01T00:00:00Z"
        ),
        fixture.base
    );
    assert_eq!(
        fs::read_to_string(fixture.worker.join("target/warm")).unwrap(),
        "cache"
    );
    assert_eq!(
        fs::read_to_string(fixture.git_dir.join("topic")).unwrap(),
        "tools-owned-lease\n"
    );
    let acquired = fixture.command(
        &fixture.repository,
        &["worktree", "acquire", "tools-fresh-audit"],
    );
    assert!(
        acquired.status.success(),
        "{}",
        String::from_utf8_lossy(&acquired.stderr)
    );
    let acquired = String::from_utf8(acquired.stdout).unwrap();
    let acquired = PathBuf::from(acquired.lines().last().expect("assigned slot path"));
    let git_dir = PathBuf::from(WorktreeInterval::git(
        &acquired,
        &["rev-parse", "--absolute-git-dir"],
        "2026-01-01T00:00:00Z",
    ));
    assert!(git_dir.join("omega-work-interval.json").exists());
    let audit = fixture.command(&acquired, &["provenance"]);
    assert!(
        audit.status.success(),
        "{}",
        String::from_utf8_lossy(&audit.stderr)
    );
    assert!(
        String::from_utf8(audit.stdout)
            .unwrap()
            .contains("Tokens: 20 in (2 cached), 1 out")
    );
}

#[test]
fn empty_or_unidentified_interval_never_silently_audits_the_full_session() {
    let fixture = WorktreeInterval::new();
    fixture.transcript(None);
    let empty = fixture.command(&fixture.worker, &["provenance"]);
    assert!(!empty.status.success());
    assert!(empty.stdout.is_empty());
    assert!(
        String::from_utf8(empty.stderr)
            .unwrap()
            .contains("cannot read active Codex transcript")
    );
    assert!(fixture.git_dir.join("omega-work-interval.json").exists());
    for record in [
        serde_json::json!({"type":"PLANNER_RESPONSE","created_at":"2026-01-01T00:00:00Z","input_tokens":100,"output_tokens":1}),
        serde_json::json!({"type":"assistant","parentUuid":"previous","timestamp":"2026-01-01T00:00:00Z","message":{"model":"fixture","usage":{"input_tokens":100,"output_tokens":1}}}),
        serde_json::json!({"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"output_tokens":1}}}}),
    ] {
        let path = fixture.transcripts.0.join("window-control.jsonl");
        let header = serde_json::json!({"type":"session_meta","payload":{"id":ACTIVE_THREAD}});
        fs::write(&path, format!("{header}\n{record}\n")).unwrap();
        let path = path.to_str().unwrap();
        let empty = fixture.command(&fixture.worker, &["provenance", "--transcript", path]);
        assert!(!empty.status.success());
        assert!(empty.stdout.is_empty());
        assert!(
            String::from_utf8(empty.stderr)
                .unwrap()
                .contains("cannot read explicit transcript")
        );
        let explicit = fixture.command(
            &fixture.worker,
            &["provenance", "--transcript", path, "--full-session"],
        );
        assert!(
            explicit.status.success(),
            "{}",
            String::from_utf8_lossy(&explicit.stderr)
        );
        assert!(
            String::from_utf8(explicit.stdout)
                .unwrap()
                .contains("Tokens: 100 in")
        );
    }
    let ambiguous = WorktreeInterval::new();
    ambiguous.transcript(Some("2026-01-03T00:00:00Z"));
    WorktreeInterval::git(
        &ambiguous.worker,
        &["checkout", "--detach", "HEAD^"],
        "2026-01-02T01:00:00Z",
    );
    let output = ambiguous.command(&ambiguous.worker, &["provenance"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("ambiguous related checkout")
    );
    assert!(!ambiguous.git_dir.join("omega-work-interval.json").exists());
    for source in ["not-a-commit".to_owned(), "f".repeat(40)] {
        fs::write(
            fixture.git_dir.join("omega-work-interval.json"),
            serde_json::json!({"version":1,"source":source,"started_epoch":1}).to_string(),
        )
        .unwrap();
        let invalid = fixture.command(&fixture.worker, &["provenance", "--allow-unavailable"]);
        assert!(!invalid.status.success());
        assert!(invalid.stdout.is_empty());
    }
}

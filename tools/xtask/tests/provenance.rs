//! An active thread's audit must not change when another session writes a transcript.

use std::fs::{self, File, FileTimes};
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

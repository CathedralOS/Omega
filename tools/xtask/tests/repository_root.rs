//! A shared executable must audit the invocation checkout, not its build tree.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Invocation(PathBuf);

impl Invocation {
    fn new(workspace: bool) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "omega-xtask-invocation-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(directory.join("nested")).unwrap();
        if workspace {
            fs::create_dir_all(directory.join("tools/xtask")).unwrap();
            fs::create_dir_all(directory.join("omega-rust")).unwrap();
            for marker in [
                "Cargo.toml",
                "tools/xtask/Cargo.toml",
                "omega-rust/pipeline.md",
            ] {
                fs::write(directory.join(marker), "").unwrap();
            }
        }
        fs::write(directory.join("members"), "").unwrap();
        Self(directory)
    }

    fn audit(&self, explicit: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .current_dir(self.0.join("nested"))
            .arg("rust-producer-omission")
            .arg("--manifest")
            .arg(self.0.join("members"))
            .arg("--record")
            .arg(self.0.join("audit.json"));
        if explicit {
            command.arg("--root").arg(&self.0);
        }
        command.output().unwrap()
    }

    fn recorded_root(&self) -> PathBuf {
        let record: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(self.0.join("audit.json")).unwrap()).unwrap();
        PathBuf::from(record["subject"]["root"].as_str().unwrap())
    }
}

impl Drop for Invocation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn shared_binary_audits_another_checkout_from_a_nested_directory() {
    let invocation = Invocation::new(true);
    let output = invocation.audit(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        invocation.recorded_root(),
        invocation.0.canonicalize().unwrap()
    );
}

#[test]
fn outside_checkout_invocation_refuses_without_creating_an_audit() {
    let invocation = Invocation::new(false);
    let output = invocation.audit(false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no Omega workspace contains"));
    assert!(!invocation.0.join("audit.json").exists());
}

#[test]
fn explicit_audit_root_still_works_outside_a_checkout() {
    let invocation = Invocation::new(false);
    let output = invocation.audit(true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        invocation.recorded_root(),
        invocation.0.canonicalize().unwrap()
    );
}

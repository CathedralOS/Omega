//! A shared executable must audit the invocation checkout, not its build tree.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

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
            for marker in ["Cargo.toml", "tools/xtask/Cargo.toml", "build.omg"] {
                fs::write(directory.join(marker), "").unwrap();
            }
        }
        fs::write(directory.join("members"), "").unwrap();
        Self(directory)
    }

    fn git(&self, arguments: &[&str]) {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn changed_sweep(&self, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .current_dir(self.0.join("nested"))
            .args(["sweep", "--list", "--changed", "HEAD^", "--omega"])
            .arg(env!("CARGO_BIN_EXE_xtask")) // List selection never executes the compiler.
            .args(extra)
            .output()
            .unwrap()
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
fn unrelated_checkout_invocation_refuses_without_creating_an_audit() {
    let invocation = Invocation::new(false);
    invocation.git(&["init", "-q"]);
    let output = invocation.audit(false);
    assert_eq!(output.status.code(), Some(2));
    let expected = format!(
        "Git checkout {} is not an Omega workspace",
        invocation.0.canonicalize().unwrap().display()
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains(&expected), "{diagnostic}");
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

/// CLI errors and help are local even when a workspace has no Git registry.
/// A consumed help-looking value must still form the ordinary valid request.
#[test]
fn claims_preflight_does_not_read_git_metadata() {
    let invocation = Invocation::new(true);
    let cases: &[(&[&str], i32, bool)] = &[
        (&["release", "--help"], 0, false),
        (&["claim", "--help"], 0, false),
        (&["note", "-h"], 0, false),
        (&["release"], 2, false),
        (&["note", "--ticket", "t"], 2, false),
        (&["claim"], 2, false),
        (&["unknown"], 2, false),
        (&["status", "--unknown"], 2, false),
        (&["claim", "--item", "t", "--owner"], 2, false),
        (&["note", "--ticket", "t", "--text", "--help"], 1, true),
    ];
    for (index, (arguments, expected, reads_git)) in cases.iter().enumerate() {
        let trace = invocation.0.join(format!("claims-{index}.jsonl"));
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("claims")
            .args(*arguments)
            .current_dir(invocation.0.join("nested"))
            .env("GIT_TRACE2_EVENT", &trace)
            .env("GIT_CEILING_DIRECTORIES", &invocation.0)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(*expected),
            "claims {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            trace.exists(),
            *reads_git,
            "claims {arguments:?} read Git metadata"
        );
    }
}

#[test]
fn status_commands_accept_a_closed_output_reader() {
    use std::io;

    let invocation = Invocation::new(true);
    for arguments in [
        &["init"][..],
        &["init", "--bare", "registry.git"],
        &["remote", "add", "origin", "registry.git"],
    ] {
        invocation.git(arguments);
    }
    for arguments in [
        ["worktree", "status"],
        ["worktree", "list"],
        ["claims", "status"],
    ] {
        let (reader, writer) = io::pipe().unwrap();
        drop(reader);
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(arguments)
            .current_dir(&invocation.0)
            .stdin(Stdio::null())
            .stdout(writer)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn worktree_status_shows_a_claim_before_the_first_commit() {
    let invocation = Invocation::new(true);
    invocation.git(&["init", "-b", "main"]);
    invocation.git(&["add", "Cargo.toml", "tools", "build.omg"]);
    invocation.git(&[
        "-c",
        "user.name=Worktree status test",
        "-c",
        "user.email=worktree@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=empty-hooks",
        "commit",
        "-m",
        "initial fixture",
    ]);
    invocation.git(&["init", "--bare", "registry.git"]);
    invocation.git(&["remote", "add", "origin", "registry.git"]);
    invocation.git(&["push", "origin", "main"]);

    let worktree = |arguments: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("worktree")
            .args(arguments)
            .current_dir(&invocation.0)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "worktree {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let row = || {
        worktree(&["status"])
            .lines()
            .find(|line| line.starts_with("worker-1 "))
            .expect("registered worker-1 status")
            .split_whitespace()
            .take(4)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    worktree(&["acquire", "tools-status-control"]);
    let claimed = row();
    assert_eq!(claimed[1], "tools-status-control");
    assert_eq!(claimed[3], "assigned");

    worktree(&["release", "worker-1"]);
    let released = row();
    assert_eq!(released[1], "-");
    assert_eq!(released[2], claimed[2]);
    assert_eq!(released[3], "idle");
}

#[test]
fn changed_compiler_sweep_guards_programs_from_the_invocation_checkout() {
    let invocation = Invocation::new(true);
    invocation.git(&["init", "-q"]);
    invocation.git(&["config", "user.name", "Sweep test"]);
    invocation.git(&["config", "user.email", "sweep@example.invalid"]);
    let mut previous = String::new();
    for (name, outcome, detail) in [
        ("guard", "Checked", ""),
        ("target", "Blocked", "specific compiler diagnostic"),
        ("other", "Blocked", "unrelated reason"),
    ] {
        let program = format!("samples/{name}/main.omg");
        let path = invocation.0.join(&program);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
        previous.push_str(&format!("{program}\t0\t1\t{outcome}\tOther\t\t{detail}\n"));
    }
    invocation.git(&["add", "."]);
    invocation.git(&["commit", "-qm", "programs"]);
    let source = invocation
        .0
        .join("source-rust/subsystems/compiler/check.rs");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(
        source,
        r#"const ERROR: &str = "specific compiler diagnostic";"#,
    )
    .unwrap();
    invocation.git(&["add", "source-rust"]);
    invocation.git(&["commit", "-qm", "compiler change"]);
    fs::write(invocation.0.join("previous.tsv"), previous).unwrap();

    let missing = invocation.changed_sweep(&[]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--previous"));
    let arguments = ["--previous", "../previous.tsv"];
    let selected = invocation.changed_sweep(&arguments);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    assert_eq!(
        String::from_utf8(selected.stdout)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["samples/guard/main.omg", "samples/target/main.omg"]
    );
    invocation.git(&[
        "config",
        "diff.external",
        "missing-omega-sweep-diff-for-test",
    ]);
    let failed = invocation.changed_sweep(&arguments);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("git diff"));

    let fixture = "source-rust/subsystems/compiler/tests/fixtures/valid/main.omg";
    let path = invocation.0.join(fixture);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
    invocation.git(&["add", fixture]);
    invocation.git(&["commit", "-qm", "fixture change"]);
    let fixture_only = invocation.changed_sweep(&[]);
    assert!(fixture_only.status.success());
    assert_eq!(
        String::from_utf8(fixture_only.stdout).unwrap().trim(),
        fixture
    );
}

#[test]
fn changed_sweep_selects_the_consumer_of_a_child_packages_dependency() {
    let invocation = Invocation::new(true);
    invocation.git(&["init", "-q"]);
    invocation.git(&["config", "user.name", "Sweep test"]);
    invocation.git(&["config", "user.email", "sweep@example.invalid"]);
    for folder in ["samples/app", "samples/independent"] {
        fs::create_dir_all(invocation.0.join(folder)).unwrap();
        fs::write(
            invocation.0.join(folder).join("main.omg"),
            "machine main() {}",
        )
        .unwrap();
    }
    for (package, location) in [
        ("samples/app", "../../packages/lib"),
        ("packages/lib", "vendor"),
        ("packages/lib/vendor", "../../shared"),
    ] {
        let folder = invocation.0.join(package);
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("build.omg"),
            format!(r#"use {{omega}}/build/dependency.omg;
machine build(builder: @mutable Build) {{
    Build::package(builder, "example");
    let registered: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "{location}" }}));
}}"#),
        ).unwrap();
    }
    invocation.git(&["add", "."]);
    invocation.git(&["commit", "-qm", "package dependencies"]);
    // Changing a file inside lib would select app even if the sweep missed the vendor edge.
    let shared = invocation.0.join("packages/shared");
    fs::create_dir_all(&shared).unwrap();
    fs::write(
        shared.join("math.omg"),
        "public machine one() -> u64 { 1u64 }",
    )
    .unwrap();
    invocation.git(&["add", "packages/shared/math.omg"]);
    invocation.git(&["commit", "-qm", "change shared math"]);

    let selected = invocation.changed_sweep(&[]);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    assert_eq!(
        String::from_utf8(selected.stdout)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["samples/app/main.omg"]
    );
}

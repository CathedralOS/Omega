//! Resolve the literal commit in a Build and repeat it after its transport disappears.
#![cfg(unix)]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn build(project: &Path, remote: &Path, output: &Path, offline: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.arg("build").arg("--build-dir").arg(output);
    if offline {
        command.arg("--offline");
    }
    command
        .arg("build.omg")
        .current_dir(project)
        .env("OMEGA_GIT_TEST_REMOTE", remote)
        .env("GIT_SSH_VARIANT", "simple")
        .env(
            "GIT_SSH_COMMAND",
            "sh -c 'exec git upload-pack \"$OMEGA_GIT_TEST_REMOTE\"' ssh",
        )
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn an_authored_git_pin_rebuilds_offline_without_the_tracking_ref_or_a_lock() {
    let Some(target) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let dir = TemporaryDirectory::create("authored-git-pin", false).unwrap();
    let remote = dir.path().join("remote");
    let project = dir.path().join("app");
    fs::create_dir(&remote).unwrap();
    fs::create_dir(&project).unwrap();
    fs::write(
        remote.join("build.omg"),
        r#"machine build(build: @mutable BuildPlan) { build.product = BuildProduct::Package { name: "answers" }; }"#,
    )
    .unwrap();
    fs::write(
        remote.join("answer.omg"),
        "public machine value() -> u64 { 37u64 }",
    )
    .unwrap();
    git(&remote, &["init", "--quiet"]);
    git(&remote, &["add", "."]);
    git(&remote, &["commit", "--quiet", "-m", "publish answer"]);
    let pin = git(&remote, &["rev-parse", "HEAD"]);
    fs::write(project.join("build.omg"), format!(r#"use main.omg as app;
use {{omega}}/build/dependencies/providers/git.omg as upstream;
machine build(build: @mutable BuildPlan) {{
    build.product = BuildProduct::Application {{ name: "pinned" }};
    let answers: DependencyDescription = |upstream|::dependency(
        "pinned_answers", "ssh://git@pin.invalid/answers.git", "refs/heads/no-such-tracking-ref", "{pin}");
    _ = Build::dependency(build, answers);
    Build::bind(build, {}::ProgramEntry, |app|::main);
}}"#, target.target_name())).unwrap();
    fs::write(
        project.join("main.omg"),
        r#"use {pinned_answers}/answer.omg as answer;
machine main() crashes Trap {
    transition |answer|::value() { 37u64 -> passed(), _ -> failed() }
    machine passed() {}
    machine failed() { crash Trap; }
}"#,
    )
    .unwrap();

    for offline in [false, true] {
        if offline {
            fs::rename(&remote, dir.path().join("disconnected")).unwrap();
        }
        let destination = dir.path().join(if offline { "rebuilt" } else { "first" });
        let output = build(&project, &remote, &destination, offline);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{report}");
        let executable = destination.join("omega-program");
        let status = Command::new(executable)
            .current_dir(&project)
            .status()
            .unwrap();
        assert!(status.success(), "{status}");
    }
}

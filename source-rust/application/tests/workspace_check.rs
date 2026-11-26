//! An unnamed check validates the members returned by the workspace Build.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn check(root: &Path, paths: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "--offline"])
        .args(paths)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn member(root: &Path, path: &str, name: &str, source: &str) -> PathBuf {
    let member = root.join(path);
    fs::create_dir_all(&member).unwrap();
    fs::write(
        member.join("build.omg"),
        format!("machine build(b: @mutable Build) {{ Build::package(b, \"{name}\"); }}"),
    )
    .unwrap();
    fs::write(member.join("unused.omg"), source).unwrap();
    member
}

#[test]
fn workspace_checks_its_returned_members_and_named_checks_select_only_one() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("local_workspace_check");
    if root.is_dir() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("build.omg"),
        r#"machine build(b: @mutable Build) { Build::member(b, "alpha"); roster(b, true); }
machine roster(b: @mutable Build, enabled: bool) {
    transition enabled { true -> listed(b), false -> decoy(b), }
    machine listed(b: @mutable Build) { Build::member(b, "nested/beta"); }
    machine decoy(b: @mutable Build) { Build::member(b, "undeclared"); }
}"#,
    )
    .unwrap();
    let good = "public machine value() -> u64 { 1u64 }";
    let broken = "public machine unused_bad(count: u64) -> u64 { count + 1u64 }";
    member(&root, "alpha", "first", good);
    let beta = member(&root, "nested/beta", "second", good);
    member(&root, "undeclared", "outsider", broken);
    for paths in [&[][..], &["build.omg"][..]] {
        let output = check(&root, paths);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = String::from_utf8(output.stdout).unwrap();
        assert!(
            report.contains("alpha") && report.contains("beta"),
            "{report}"
        );
        assert!(!report.contains("undeclared"), "{report}");
    }
    // The two enclosing conventional entries are ambiguous without running either.
    let ambiguous = check(&beta, &[]);
    assert!(!ambiguous.status.success());
    assert!(
        String::from_utf8_lossy(&ambiguous.stderr).contains("multiple enclosing Build entries")
    );
    let scoped_ok = check(
        &root,
        &[
            "--build-input",
            "build.omg",
            "--build-input",
            "alpha",
            "--build-input",
            "nested/beta",
            "build.omg",
        ],
    );
    assert!(
        scoped_ok.status.success(),
        "{}",
        String::from_utf8_lossy(&scoped_ok.stderr)
    );
    let scoped = check(&root, &["--build-input", "build.omg", "build.omg"]);
    assert!(!scoped.status.success());
    assert!(String::from_utf8_lossy(&scoped.stderr).contains("complete directory"));
    fs::write(
        beta.join("build.omg"),
        "machine build(b: @mutable Build) { Build::package(b, \"first\"); }",
    )
    .unwrap();
    let duplicate = check(&root, &["build.omg"]);
    assert!(
        !duplicate.status.success()
            && String::from_utf8_lossy(&duplicate.stderr).contains("duplicate declared name")
    );
    fs::write(
        beta.join("build.omg"),
        "machine build(b: @mutable Build) { Build::package(b, \"second\"); }",
    )
    .unwrap();
    fs::write(beta.join("unused.omg"), broken).unwrap();
    let named = check(&root, &["alpha/build.omg"]);
    assert!(
        named.status.success(),
        "{}",
        String::from_utf8_lossy(&named.stderr)
    );
    let output = check(&root, &[]);
    let refused = String::from_utf8(output.stderr).unwrap();
    assert!(
        !output.status.success()
            && refused.contains("unused_bad")
            && refused.contains("may overflow"),
        "{refused}"
    );
}

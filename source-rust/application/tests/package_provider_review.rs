//! Package review retains the checked provider and the public types in its contract.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn review_retains_the_provider_already_checked_for_each_target() {
    let project = TemporaryDirectory::create("review-selected-provider", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"machine build(builder: @mutable Build) {
    Build::package(builder, "reading");
    Build::select_provider::<Reading, LinuxReading>(builder, linux_x86_64);
    Build::select_provider::<Reading, MacReading>(builder, macos_arm64);
}"#,
    )
    .unwrap();
    fs::write(
        project.path().join("reading.omg"),
        "public boundary trait Reading { machine read() -> u64 reaches none crashes none; }
public LinuxReading satisfies Reading { machine read() -> u64 { 1 } }
public MacReading satisfies Reading { machine read() -> u64 { 2 } }",
    )
    .unwrap();
    for (target, provider, inactive) in [
        ("linux_x86_64", "LinuxReading", "MacReading"),
        ("macos_arm64", "MacReading", "LinuxReading"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(project.path())
            .args([
                "packages",
                "audit",
                "--offline",
                "--target",
                target,
                "--details",
            ])
            .output()
            .unwrap();
        assert!(
            matches!(output.status.code(), Some(0 | 3)),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = String::from_utf8(output.stdout).unwrap();
        let selected: Vec<_> = report
            .lines()
            .filter(|line| line.trim_start().starts_with("provider-type "))
            .collect();
        assert!(
            selected.iter().any(|line| line.contains(provider)),
            "{report}"
        );
        assert!(
            !selected.iter().any(|line| line.contains(inactive)),
            "{report}"
        );
    }
}

#[test]
fn review_retains_owned_boundary_results_disclosed_by_public_data() {
    let project = TemporaryDirectory::create("review-owned-result", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"machine build(builder: @mutable Build) {
    Build::package(builder, "returned_status");
    Build::select_provider::<Reader, LocalReader>(builder, linux_x86_64);
}"#,
    )
    .unwrap();
    let source = r#"public data Status { case Read(count: u64); }
public data Receipt [linear] { case Read(count: u64); }
public boundary trait Reader {
    machine status() -> Status reaches none crashes none;
    machine receipt() -> Receipt reaches none crashes none;
}
public LocalReader satisfies Reader {
    machine status() -> Status { Status::Read { count: 7 } }
    machine receipt() -> Receipt { Receipt::Read { count: 7 } }
}"#;
    let (status, report) = audit_reader(&project, source);
    assert_eq!(status, Some(0), "{report}");
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(report.contains("public declarations: data 2"), "{report}");

    let (status, report) = audit_reader(
        &project,
        &source.replace("public data Status", "data Status"),
    );
    assert_eq!(status, Some(1), "{report}");
    assert!(
        report.contains("result.multiplicity") && report.contains("Reader::status"),
        "{report}"
    );
}

#[test]
fn review_retains_checked_domain_casts_at_their_declared_exposure() {
    let project = TemporaryDirectory::create("review-domain-cast", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"machine build(builder: @mutable Build) {
    Build::package(builder, "cast_contract");
    Build::select_provider::<Reader, LocalReader>(builder, linux_x86_64);
}"#,
    )
    .unwrap();
    let source = r#"public domain u64::Handle;
public domain u64::Other;
public boundary trait Reader {
    machine take(handle: u64 in Handle) -> u64
    requires (handle as -Handle) != 0 reaches none crashes none;
}
public LocalReader satisfies Reader {
    machine take(handle: u64 in Handle) -> u64
    requires (handle as -Handle) != 0 { 17 }
}"#;
    let (status, report) = audit_reader(&project, source);
    assert_eq!(status, Some(0), "{report}");
    assert!(report.contains("fresh-analysis complete"), "{report}");

    let (status, report) = audit_reader(&project, &source.replace("-Handle", "-Other"));
    assert_eq!(status, Some(1), "{report}");
    assert!(report.contains("does not carry `Other`"), "{report}");

    let private_domain = source.replace("public domain u64::Handle", "domain u64::Handle");
    let (status, report) = audit_reader(&project, &private_domain);
    assert_eq!(status, Some(1), "{report}");
    assert!(report.contains("private domain"), "{report}");

    let private_schema = private_domain
        .replace("public boundary trait Reader", "boundary trait Reader")
        .replace("public LocalReader satisfies", "LocalReader satisfies");
    let (status, report) = audit_reader(&project, &private_schema);
    assert_eq!(status, Some(0), "{report}");
    assert!(report.contains("fresh-analysis complete"), "{report}");
}

fn audit_reader(project: &TemporaryDirectory, source: &str) -> (Option<i32>, String) {
    fs::write(project.path().join("reader.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(project.path())
        .args(["packages", "audit", "--offline", "--target", "linux_x86_64"])
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

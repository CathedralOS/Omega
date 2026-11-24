//! Package review describes the service actually selected for each target.
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

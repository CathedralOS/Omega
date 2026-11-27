//! Each native import has one trust row in the lock for its supplying Build.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn updating_a_consumer_keeps_each_native_binding_in_its_supplying_package() {
    check_lock("reader", Some(r#"bytes "getppid""#));
}

#[test]
fn updating_a_consumer_keeps_an_unused_native_binding_in_its_supplying_package() {
    check_lock("empty", None);
}

fn check_lock(consumer_name: &str, local_locator: Option<&str>) {
    let scratch = TemporaryDirectory::create("native-binding-ownership", false).unwrap();
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/packages/pass/native_binding_ownership");
    for package in [consumer_name, "reading"] {
        let directory = scratch.path().join(package);
        fs::create_dir(&directory).unwrap();
        for file in ["build.omg", "main.omg"] {
            fs::copy(fixture.join(package).join(file), directory.join(file)).unwrap();
        }
    }

    let consumer = scratch.path().join(consumer_name);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(&consumer)
        .env(
            "OMEGA_LIBRARY_ROOT",
            support::repository_root().join("source/library"),
        )
        .args([
            "packages",
            "update",
            "--offline",
            "--target",
            "linux_x86_64",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "--dangerously-accept-all",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = fs::read_to_string(consumer.join("omega.lock")).unwrap();
    let packages: Vec<_> = text
        .split("\npackage\nsource\nname \"")
        .skip(1)
        .map(|row| row.split_once('"').unwrap().0)
        .collect();
    let occurrences: Vec<_> = text.split("\noccurrence ").skip(1).collect();
    assert_eq!(occurrences.len(), 2);
    for occurrence in occurrences {
        let index: usize = occurrence
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let locator = match packages[index] {
            "reading" => Some(r#"bytes "getpid""#),
            name if name == consumer_name => local_locator,
            other => panic!("unexpected package {other}"),
        };
        let policy = occurrence.split_once("\nend_acceptance\n").unwrap().0;
        if let Some(locator) = locator {
            assert!(policy.lines().any(|line| line.trim() == locator));
        }
        let expected_count = usize::from(locator.is_some());
        assert_eq!(
            policy
                .lines()
                .filter(|line| *line == "row native_binding")
                .count(),
            expected_count
        );
        assert_eq!(
            occurrence
                .lines()
                .filter(|line| line.starts_with("native_bindings_v1 "))
                .count(),
            expected_count
        );
    }
}

#[test]
fn review_keeps_each_closed_native_call_with_its_borrowed_array() {
    let scratch = TemporaryDirectory::create("review-borrowed-native-array", false).unwrap();
    let project = scratch.path().join("reader");
    fs::create_dir(&project).unwrap();
    let library = scratch.path().join("probe");
    fs::create_dir(&library).unwrap();
    fs::write(
        project.join("build.omg"),
        r#"use {omega}/build/dependency.omg;
use main.omg as app;
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Application { name: "reader" };
    _ = Build::dependency(plan, |dependency|::from_source(Source::Path { location: "../probe" }));
    Build::bind(plan, windows_x86_64::ProgramEntry, |app|::main);
}"#,
    )
    .unwrap();
    fs::write(project.join("main.omg"), "public machine main() {}").unwrap();
    fs::write(
        library.join("main.omg"),
        r#"boundary trait Probe {
    machine read<const Size: u64>(bytes: @[u8; Size]) -> u32 reaches none crashes none;
}"#,
    )
    .unwrap();
    fs::write(
        library.join("build.omg"),
        r#"use main.omg as api;
use {omega}/language/core/external_binding.omg as native;
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: "probe" };
    Build::bind_native::<|api|::Probe::read<4>>(plan, windows_x86_64,
        |native|::ForeignBinding::DllImport { import: |native|::DllImport::PeByName {
            library: "probe.dll", export: "ReadFour"
        } });
    Build::bind_native::<|api|::Probe::read<8>>(plan, windows_x86_64,
        |native|::ForeignBinding::DllImport { import: |native|::DllImport::PeByName {
            library: "probe.dll", export: "ReadEight"
        } });
}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(project)
        .env(
            "OMEGA_LIBRARY_ROOT",
            support::repository_root().join("source/library"),
        )
        .args([
            "packages",
            "audit",
            "--offline",
            "--target",
            "windows_x86_64",
            "--details",
        ])
        .output()
        .unwrap();
    let report = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(3), "{report}");
    assert!(report.contains("fresh-analysis complete"), "{report}");
    for (export, extent) in [("ReadFour", 4), ("ReadEight", 8)] {
        let (before_export, _) = report.split_once(&format!("bytes \"{export}\"")).unwrap();
        let (_, binding) = before_export
            .rsplit_once("\n          field application {")
            .unwrap();
        assert!(binding.contains(&format!("literal({extent})")), "{binding}");
    }
}

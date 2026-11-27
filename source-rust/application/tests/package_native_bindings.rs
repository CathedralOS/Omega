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
    let bindings: Vec<_> = report
        .lines()
        .filter(|line| line.starts_with("  native binding "))
        .collect();
    assert_eq!(bindings.len(), 2, "{report}");
    for (export, extent) in [("ReadFour", 4), ("ReadEight", 8)] {
        let line = bindings
            .iter()
            .find(|line| line.ends_with(&format!(r#" -> DLL "probe.dll" export "{export}""#)))
            .unwrap();
        assert!(
            line.contains(r#""probe"::"#)
                && line.contains(&format!("<index-expression(integer({extent}))>")),
            "{line}"
        );
        let (before_export, _) = report.split_once(&format!("bytes \"{export}\"")).unwrap();
        let (_, binding) = before_export
            .rsplit_once("\n          field application {")
            .unwrap();
        assert!(binding.contains(&format!("literal({extent})")), "{binding}");
    }
}

#[test]
fn package_review_discloses_the_windows_utf8_import_without_any_converter_callers() {
    let scratch = TemporaryDirectory::create("unused-generic-windows-converter", false).unwrap();
    let consumer = scratch.path().join("consumer");
    let library = scratch.path().join("windows");
    fs::create_dir(&consumer).unwrap();
    fs::create_dir(&library).unwrap();
    fs::write(
        consumer.join("build.omg"),
        r#"use {omega}/build/dependency.omg;
use main.omg as app;
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Application { name: "consumer" };
    _ = Build::dependency(plan, |dependency|::from_source(Source::Path { location: "../windows" }));
    Build::bind(plan, windows_x86_64::ProgramEntry, |app|::main);
}"#,
    )
    .unwrap();
    fs::write(consumer.join("main.omg"), "public machine main() {}").unwrap();
    let library_root = support::repository_root().join("source/library");
    // Use the system package's own boundary with both capacity binders and its
    // predicates. None of the path, reparse or directory clients are in this product.
    fs::copy(
        library_root.join("system/targets/windows_x86_64/windows_filesystem_path_query_native.omg"),
        library.join("main.omg"),
    )
    .unwrap();
    fs::write(
        library.join("build.omg"),
        r#"use main.omg as api;
use {omega}/language/core/external_binding.omg as native;
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: "windows" };
    Build::bind_native::<|api|::WindowsFilesystemUtf8Io::utf8>(plan, windows_x86_64,
        |native|::ForeignBinding::DllImport { import: |native|::DllImport::PeByName {
            library: "kernel32.dll", export: "WideCharToMultiByte"
        } });
}"#,
    )
    .unwrap();
    let audit = |exit| {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&consumer)
            .env("OMEGA_LIBRARY_ROOT", &library_root)
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
        assert_eq!(
            output.status.code(),
            Some(exit),
            "{report}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        report
    };
    let report = audit(3);
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(
        report
            .lines()
            .any(|line| line.contains("unused generic native import")
                && line.contains("utf8")
                && line.contains("<Units, Bytes>")
                && line.contains(r#"export "WideCharToMultiByte""#)),
        "{report}"
    );
    assert!(
        report.contains(r#"bytes "WideCharToMultiByte""#),
        "{report}"
    );
    assert!(report.contains("tag unused_generic"), "{report}");

    let client = r#"
public machine encode_one(source: @[u16; 1], output: @mutable [u8; 4]) -> i32 {
    let count: i32 = WindowsFilesystemUtf8Io::utf8::<1, 4>(65001, 128, source, 1, output, 4, 0, 0);
    transition { _ -> (count) }
}
public machine encode_two(source: @[u16; 2], output: @mutable [u8; 8]) -> i32 {
    let count: i32 = WindowsFilesystemUtf8Io::utf8::<2, 8>(65001, 128, source, 2, output, 8, 0, 0);
    transition { _ -> (count) }
}
"#;
    let main = library.join("main.omg");
    let original = fs::read_to_string(&main).unwrap();
    fs::write(&main, format!("{original}{client}")).unwrap();
    let used = audit(3);
    assert!(used.contains("fresh-analysis complete"), "{used}");
    assert!(!used.contains("tag unused_generic"), "{used}");
    let applications: Vec<_> = used.split(r#"bytes "WideCharToMultiByte""#).collect();
    assert_eq!(applications.len(), 3, "{used}");
    for buffer in [4, 8] {
        assert!(
            applications[..2].iter().any(|prefix| {
                prefix
                    .rsplit_once("\n          field application {")
                    .is_some_and(|(_, row)| {
                        row.contains(&format!("literal({buffer})"))
                            && row.contains("field calling {")
                    })
            }),
            "{used}"
        );
    }

    fs::write(&main, original).unwrap();
    let update = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(&consumer)
        .env("OMEGA_LIBRARY_ROOT", &library_root)
        .args([
            "packages",
            "update",
            "--offline",
            "--target",
            "windows_x86_64",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "--dangerously-accept-all",
        ])
        .output()
        .unwrap();
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    let lock = fs::read_to_string(consumer.join("omega.lock")).unwrap();
    assert!(lock.contains("tag unused_generic"), "{lock}");
    let accepted = audit(0);
    assert!(
        accepted.contains("accepted-policy equal-to-fresh"),
        "{accepted}"
    );
}

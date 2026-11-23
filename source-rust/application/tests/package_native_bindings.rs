//! Each native import has one trust row in the lock for its supplying Build.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn updating_a_consumer_keeps_each_native_binding_in_its_supplying_package() {
    let scratch = TemporaryDirectory::create("native-binding-ownership", false).unwrap();
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/packages/pass/native_binding_ownership");
    for package in ["reader", "reading"] {
        let directory = scratch.path().join(package);
        fs::create_dir(&directory).unwrap();
        for file in ["build.omg", "main.omg"] {
            fs::copy(fixture.join(package).join(file), directory.join(file)).unwrap();
        }
    }

    let consumer = scratch.path().join("reader");
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
            "reading" => r#"bytes "getpid""#,
            "reader" => r#"bytes "getppid""#,
            other => panic!("unexpected package {other}"),
        };
        let policy = occurrence.split_once("\nend_acceptance\n").unwrap().0;
        assert!(policy.lines().any(|line| line.trim() == locator));
        assert_eq!(
            policy
                .lines()
                .filter(|line| *line == "row native_binding")
                .count(),
            1
        );
        assert_eq!(
            occurrence
                .lines()
                .filter(|line| line.starts_with("native_bindings_v1 "))
                .count(),
            1
        );
    }
}

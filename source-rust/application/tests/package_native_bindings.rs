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

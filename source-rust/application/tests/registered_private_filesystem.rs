//! Registered tests reopen their own files on both engines under an ordinary source boundary.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

const BUILD: &str = include_str!("fixtures/build/run/private_filesystem_group/build.omg");
const PROGRAM: &str = include_str!("fixtures/build/run/private_filesystem_group/main.omg");

fn run(build: &str, program: &str, target: &str) -> Output {
    let directory = TemporaryDirectory::create("omega-private-files", false).unwrap();
    let root = directory.path();
    std::fs::write(root.join("build.omg"), build).unwrap();
    std::fs::write(root.join("main.omg"), program).unwrap();
    std::fs::write(root.join("note.txt"), b"host sentinel").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["build", "--compare-tests", "--target", target, "build.omg"])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(
        std::fs::read(root.join("note.txt")).unwrap(),
        b"host sentinel"
    );
    result
}

#[test]
fn private_filesystems_are_fresh_per_test_and_engine() {
    let target = if cfg!(target_arch = "x86_64") {
        "linux_x86_64"
    } else {
        "linux_arm64"
    };
    let result = run(BUILD, PROGRAM, target);
    let output = String::from_utf8_lossy(&result.stderr);
    for test in ["first_file", "second_file"] {
        let row = output
            .lines()
            .find(|row| row.contains(&format!("test `{test}` in group `Checks::file`")))
            .unwrap_or_else(|| panic!("{output}"));
        assert!(row.contains("passed on Psi interpreter"), "{row}");
        if result.status.success() {
            assert!(row.contains("native/Psi comparison agrees"), "{row}");
        } else {
            // Restricted Linux hosts can refuse user or mount namespace setup before exec.
            assert!(
                row.contains("native: cannot start the test")
                    && (row.contains("Operation not permitted")
                        || row.contains("Read-only file system")
                        || row.contains("Permission denied"))
                    && row.contains("native/Psi comparison unavailable"),
                "{row}"
            );
        }
    }
}

#[test]
fn private_backing_does_not_grant_a_sibling_group_the_same_operation() {
    let build = BUILD.replace(
        "group.inputs.private_filesystem = true;",
        "group.inputs.private_filesystem = true;\n    _ = builder.tests.group::<Checks::plain>();",
    );
    let program = PROGRAM.replace("trait Checks {", "trait Checks {\n    machine plain() reaches FilesystemContentRead + FilesystemContentWrite + FilesystemMetadataMutation + FilesystemNamespaceMutation crashes Trap;") + "\nmachine sibling_file() satisfies Checks::plain { round_trip(67u8); }\n";
    let target = if cfg!(target_arch = "x86_64") {
        "linux_arm64"
    } else {
        "linux_x86_64"
    };
    let result = run(&build, &program, target);
    assert!(!result.status.success());
    let output = String::from_utf8_lossy(&result.stderr);
    let sibling = output
        .lines()
        .find(|row| row.contains("test `sibling_file` in group `Checks::plain`"))
        .unwrap_or_else(|| panic!("{output}"));
    assert!(
        sibling.contains("cannot be produced") && sibling.contains("isolated execution"),
        "{sibling}"
    );
    for test in ["first_file", "second_file"] {
        let row = output
            .lines()
            .find(|row| row.contains(&format!("test `{test}` in group `Checks::file`")))
            .unwrap_or_else(|| panic!("{output}"));
        assert!(
            row.contains("passed on Psi interpreter")
                && row.contains("its target does not execute on this host"),
            "{row}"
        );
    }
}

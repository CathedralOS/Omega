//! A successful check of an application covers the obligations of its selected entry.

mod support;

use std::path::Path;
use std::process::{Command, Output, Stdio};

fn check(entry: impl AsRef<Path>, target: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "--verbose", "--target", target])
        .arg(support::repository_root().join(entry))
        .stdin(Stdio::null())
        .output()
        .expect("omega checks the program")
}

#[test]
fn a_data_predicate_keeps_its_own_carrier_beside_the_target_contract() {
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "predicate-carrier",
        false,
    )
    .unwrap();
    let entry = scratch.path().join("main.omg");
    std::fs::write(
        &entry,
        "data NativePlace requires true { local: u64; }\npublic machine main() {}",
    )
    .unwrap();
    let output = check(entry, "linux_x86_64");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_layout_dependent_subtraction_must_fit_its_carrier() {
    let directory = "samples/cli/facts/size_align_layout_geometry";
    let positive = check(format!("{directory}/local/main.omg"), "linux_x86_64");
    assert!(positive.status.success(), "{positive:?}");
    let negative = check(
        format!("{directory}/invalid_local/main.omg"),
        "linux_x86_64",
    );
    assert_eq!(negative.status.code(), Some(1), "{negative:?}");
    let stderr = String::from_utf8_lossy(&negative.stderr);
    assert!(
        stderr.contains("bounds [-8, -8] lie outside [0, 18446744073709551615]"),
        "{stderr}"
    );
}

#[test]
fn an_executable_application_requires_a_bound_entry() {
    let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
        "check-unbound-entry",
        false,
    )
    .unwrap();
    std::fs::write(directory.path().join("main.omg"), "machine main() {}").unwrap();
    let build = directory.path().join("build.omg");
    std::fs::write(
        &build,
        r#"machine build(builder: @mutable Build) { Build::application(builder, "unbound"); }"#,
    )
    .unwrap();
    let output = check(build, "linux_x86_64");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no unique checked entry contract"),
        "{stderr}"
    );
}

#[test]
fn an_entryless_package_can_be_checked_on_its_own() {
    let package = "source-rust/subsystems/compiler/tests/fixtures/core/fail/content_projection_foreign_package/owner/build.omg";
    let output = check(package, "linux_x86_64");
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn a_checked_application_needs_its_selected_providers() {
    let directory = "source-rust/application/tests/fixtures/build/fail/target_selected_implementation_missing_row";
    for root in ["build.omg", "main.omg"] {
        let entry = format!("{directory}/{root}");
        let linux = check(&entry, "linux_x86_64");
        assert!(linux.status.success(), "{entry}: {linux:?}");
        let windows = check(&entry, "windows_x86_64");
        assert_eq!(windows.status.code(), Some(1), "{entry}: {windows:?}");
        let stderr = String::from_utf8_lossy(&windows.stderr);
        for member in ["record", "no_follow_flag"] {
            let expected = format!(
                "boundary requirement `TargetConstants::{member}` has no provider on target `windows_x86_64`"
            );
            assert!(stderr.contains(&expected), "{entry}: {stderr}");
        }
    }
}

//! Exercise provider selection and receiving authority through ordinary registered test builds.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn compare_registered_tests(fixture: &str) -> (TemporaryDirectory, Output) {
    let project = support::repository_root()
        .join("source-rust/application/tests/fixtures/providers")
        .join(fixture);
    let outputs = TemporaryDirectory::create("test-provider-selection", false)
        .expect("temporary build directory");
    let build = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["build", "--compare-tests", "--build-dir"])
        .arg(outputs.path())
        .arg("build.omg")
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs");
    (outputs, build)
}

#[test]
fn the_registered_test_uses_the_default_and_the_application_uses_its_override() {
    let (outputs, build) = compare_registered_tests("run/registered_test_provider_selection");
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(build.status.success(), "{stderr}");
    assert!(
        stderr.contains("test `test_uses_default` in group `Tests::reading`")
            && stderr.contains("native/Psi comparison agrees"),
        "{stderr}"
    );
    let application = Command::new(outputs.path().join("omega-program"))
        .stdin(Stdio::null())
        .status()
        .expect("the published application runs");
    assert!(
        application.success(),
        "application exited with {application}"
    );
}

#[test]
fn ungranted_host_bindings_are_refused_before_the_registered_tests_run() {
    let (outputs, build) = compare_registered_tests("fail/registered_test_host_binding");
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(!build.status.success(), "{stderr}");
    assert!(
        stderr.contains("test `test_calls_host` in group `Tests::host` cannot be produced")
            && stderr.contains("isolated execution cannot grant unclassified physical authority")
            && stderr.contains("HostProcessId"),
        "{stderr}"
    );
    assert!(
        stderr.contains("test `test_needs_file_write` in group `Tests::host` cannot be produced")
            && stderr.contains("isolated execution has no FilesystemContentWrite grant")
            && stderr.contains("HostFile"),
        "{stderr}"
    );
    assert!(!outputs.path().join("omega-program").exists());
}

//! Follow the Build entry chosen by each executed plan, then run its final product.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::process::{Command, ExitStatus, Stdio};

fn invoke(files: &[(&str, &str)], arguments: &[&str]) -> (ExitStatus, String) {
    let project = TemporaryDirectory::create("staged-build-entries", false).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.args(arguments);
    for (name, source) in files {
        let path = project.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
        command.args(["--build-input", name]);
    }
    let output = command
        .arg("build.omg")
        .current_dir(project.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status, report)
}

fn build(body: &str) -> String {
    format!("machine build(builder: @mutable Build) {{ {body} }}")
}

#[test]
fn successive_builds_select_the_final_native_and_psi_entry() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let first = build(r#"Build::build_entry(builder, "stages/configure.omg");"#);
    let middle = build(r#"Build::build_entry(builder, "release.omg");"#);
    let last = format!(
        r#"machine build(builder: @mutable Build) {{
            Build::application(builder, "staged");
            Build::bind(builder, {}\ProgramEntry, launch);
        }}
        machine launch() {{}}"#,
        host.target_name()
    );
    let (status, report) = invoke(
        &[
            ("build.omg", &first),
            ("stages/configure.omg", &middle),
            ("release.omg", &last),
        ],
        &["run", "--both"],
    );
    assert!(status.success(), "{report}");
    for observation in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == observation), "{report}");
    }
}

#[test]
fn a_successor_must_be_one_canonical_file_in_the_frozen_source_view() {
    for (name, reason) in [
        (
            "../outside.omg",
            "Build successor entry requires a canonical package-relative .omg filename",
        ),
        ("absent.omg", "cannot select source"),
    ] {
        let entry = build(&format!(r#"Build::build_entry(builder, "{name}");"#));
        let (status, report) = invoke(&[("build.omg", &entry)], &["build"]);
        assert!(!status.success(), "{report}");
        assert!(report.contains(reason), "{report}");
        if name == "absent.omg" {
            assert!(report.contains("/absent.omg:"), "{report}");
        }
    }
}

#[test]
fn a_build_may_choose_only_one_successor_or_final_product() {
    let successor = r#"Build::build_entry(builder, "configure.omg");"#;
    let application = r#"Build::application(builder, "later");"#;
    for (first, second, reason) in [
        (
            successor,
            successor,
            "Build selected a successor more than once",
        ),
        (successor, application, "Build returned mixed project roles"),
        (application, successor, "Build returned mixed project roles"),
    ] {
        let entry = build(&format!("{first} {second}"));
        let (status, report) = invoke(&[("build.omg", &entry)], &["build"]);
        assert!(!status.success(), "{report}");
        assert!(report.contains(reason), "{report}");
    }
}

#[test]
fn the_successor_must_be_admitted_before_its_first_instruction() {
    let first = build(r#"Build::build_entry(builder, "configure.omg");"#);
    let invalid = r#"machine build(builder: @mutable Build)
        requires 1u64 == 2u64 {
            Build::build_entry(builder, "../outside.omg");
        }"#;
    let (status, report) = invoke(
        &[("build.omg", &first), ("configure.omg", invalid)],
        &["build"],
    );
    assert!(!status.success(), "{report}");
    assert!(
        report.contains("the build entry's `requires` does not hold for this build evaluator"),
        "{report}"
    );
    assert!(
        !report.contains("requires a canonical package-relative .omg filename"),
        "{report}"
    );
}

#[test]
fn the_selected_build_must_accept_the_original_builder() {
    let first = build(r#"Build::build_entry(builder, "configure.omg");"#);
    for (invalid, reason) in [
        ("machine main() {}", "declares no free build machine"),
        (
            "machine build() {} machine main() {}",
            "one original Build parameter",
        ),
    ] {
        let (status, report) = invoke(
            &[("build.omg", &first), ("configure.omg", invalid)],
            &["build"],
        );
        assert!(!status.success(), "{report}");
        assert!(report.contains(reason), "{report}");
    }
}

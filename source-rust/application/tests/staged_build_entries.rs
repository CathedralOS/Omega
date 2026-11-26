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
    format!("machine build(builder: @mutable BuildPlan) {{ {body} }}")
}

#[test]
fn successive_builds_select_the_final_native_and_psi_entry() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let first = build(
        r#"builder.product = BuildProduct::InterpretedBuild { entry: "stages/configure.omg" };"#,
    );
    let middle =
        build(r#"builder.product = BuildProduct::InterpretedBuild { entry: "release.lock.omg" };"#);
    let last = format!(
        r#"machine build(builder: @mutable BuildPlan) {{
            builder.product = BuildProduct::Application {{ name: "staged" }};
            Build::bind(builder, {}::ProgramEntry, launch);
        }}
        machine launch() {{}}
        machine main() crashes Trap {{ crash Trap; }}"#,
        host.target_name()
    );
    for (engine, observation) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
    let (status, report) = invoke(
        &[
            ("build.omg", &first),
            ("stages/configure.omg", &middle),
            ("release.lock.omg", &last),
        ],
        &["run", "--engine", engine],
    );
        assert!(status.success(), "{report}");
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
        let entry = build(&format!(r#"builder.product = BuildProduct::InterpretedBuild {{ entry: "{name}" }};"#));
        let (status, report) = invoke(&[("build.omg", &entry)], &["build"]);
        assert!(!status.success(), "{report}");
        assert!(report.contains(reason), "{report}");
        if name == "absent.omg" {
            assert!(report.contains("absent.omg:"), "{report}");
        }
    }
}

#[test]
fn the_returned_product_is_the_last_value_in_the_plan() {
    let first = build(
        r#"builder.product = BuildProduct::InterpretedBuild { entry: "not_selected.omg" };
        builder.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };"#,
    );
    let last = build(r#"builder.product = BuildProduct::Package { name: "final_package" };"#);
    let (status, report) = invoke(
        &[("build.omg", &first), ("configure.omg", &last)],
        &["--check"],
    );
    assert!(status.success(), "{report}");
}

#[test]
fn the_successor_must_be_admitted_before_its_first_instruction() {
    let first =
        build(r#"builder.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };"#);
    let invalid = r#"machine build(builder: @mutable BuildPlan)
        requires 1u64 == 2u64 {
            builder.product = BuildProduct::InterpretedBuild { entry: "../outside.omg" };
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
    let first =
        build(r#"builder.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };"#);
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

#[test]
fn dependency_imports_respect_the_build_files_the_child_entered() {
    let consumer = r#"use {omega}/build/dependency.omg;
        machine build(builder: @mutable BuildPlan) {
            builder.product = BuildProduct::Application { name: "consumer" };
            let kit: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "kit" }));
            Build::bind(builder, linux_x86_64::ProgramEntry, main);
        }"#;
    let first =
        build(r#"builder.product = BuildProduct::InterpretedBuild { entry: "prepare.omg" };"#);
    let prepare =
        build(r#"builder.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };"#);
    let configure = build(r#"builder.product = BuildProduct::Package { name: "kit" };"#);
    let marker = "public machine marker() -> i32 { 3 }";
    let prepare = format!("{prepare}\n{marker}");
    let configure = format!("{configure}\n{marker}");
    for (path, permitted) in [
        ("library/build.omg", true),
        ("prepare.omg", false),
        ("configure.omg", false),
    ] {
        let source =
            format!("use {{kit}}/{path} as lib; machine main() {{ let value: i32 = |lib|::marker(); }}");
        let (status, report) = invoke(
            &[
                ("build.omg", consumer),
                ("main.omg", &source),
                ("kit/build.omg", &first),
                ("kit/prepare.omg", &prepare),
                ("kit/configure.omg", &configure),
                ("kit/library/build.omg", marker),
            ],
            &["--check", "--target", "linux_x86_64"],
        );
        assert_eq!(status.success(), permitted, "{path}: {report}");
        if !permitted {
            assert!(
                report.contains("may not load dependency build file") && report.contains(path),
                "{report}"
            );
        }
    }
}

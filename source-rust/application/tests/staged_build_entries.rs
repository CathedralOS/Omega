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
        let entry = build(&format!(
            r#"builder.product = BuildProduct::InterpretedBuild {{ entry: "{name}" }};"#
        ));
        let (status, report) = invoke(&[("build.omg", &entry)], &["build"]);
        assert!(!status.success(), "{report}");
        assert!(report.contains(reason), "{report}");
        if name == "absent.omg" {
            assert!(report.contains("absent.omg:"), "{report}");
        }
    }
}

#[test]
fn an_authored_build_must_fill_its_initially_unselected_product() {
    let (status, report) = invoke(&[("build.omg", &build(""))], &["--check"]);
    assert!(!status.success(), "{report}");
    assert!(
        report.contains("Build returned no project role"),
        "{report}"
    );
}

#[test]
fn the_returned_product_is_the_last_value_in_the_plan() {
    let first = build(
        r#"builder.product = BuildProduct::InterpretedBuild { entry: "not_selected.omg" };
        builder.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };"#,
    );
    let last = r#"machine build(plan: @mutable BuildPlan) {
    let name: @[u8] = choose_name(true);
    declare(plan, name);
}
machine choose_name(flag: bool) -> @[u8] {
    match flag { true -> "final_package", _ -> "invalid-name" }
}
machine declare<'s>(plan: @mutable BuildPlan<'s>, name: @'s [u8]) {
    plan.product = BuildProduct::Package { name: name };
}"#;
    let (status, report) = invoke(
        &[("build.omg", &first), ("configure.omg", last)],
        &["--check"],
    );
    assert!(status.success(), "{report}");
}

#[test]
fn the_plan_cannot_return_a_name_borrowed_from_its_own_stack_frame() {
    let entry = build(
        r#"let name: [u8; 3] = [107u8, 105u8, 116u8];
        builder.product = BuildProduct::Package { name: @name[..] };"#,
    );
    let (status, report) = invoke(&[("build.omg", &entry)], &["--check"]);
    assert!(!status.success(), "{report}");
    assert!(
        report.contains("local storage cannot establish that promise"),
        "{report}"
    );
}

#[test]
fn a_queried_declaration_keeps_its_product_identity() {
    let first = r#"use {omega}/build/dependency.omg;
use {omega}/language/core/reflect/reflect.omg as reflect;
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: "first" };
    let mutable description: DependencyDescription = |dependency|::from_source(Source::Path { location: "library" });
    |dependency|::with_alias(@mutable description, "api");
    let registration: DependencyRegistration = Build::dependency(plan, description);
    let info: |reflect|::FieldInfo = |reflect|::field(api::Table::invoke);
    transition info.name == "invoke" {
        true -> finish(plan),
        false -> refuse()
    }
    machine finish(plan: @mutable BuildPlan) {
        plan.product = BuildProduct::Package { name: "final_name" };
    }
    machine refuse() crashes Trap { crash Trap; }
}"#;
    let library = build(r#"builder.product = BuildProduct::Package { name: "descriptions" };"#);
    for (name, approved) in [("first", true), ("other", false)] {
        let caller = first.replace("final_name", name);
        let (status, report) = invoke(
            &[
                ("build.omg", &caller),
                ("library/build.omg", &library),
                ("library/main.omg", "public data Table { invoke: addr; }"),
            ],
            &["--check"],
        );
        assert_eq!(status.success(), approved, "{report}");
        if !approved {
            assert!(
                report.contains("returned Build product differs from the product that issued a declaration description"),
                "{report}"
            );
        }
    }
}

#[test]
fn only_the_returned_workspace_catalog_selects_members() {
    let workspace = r#"use {omega}/build/workspace.omg;
machine build(plan: @mutable BuildPlan) {
    let first: WorkspaceMembers = |workspace|::with_member(plan, WorkspaceMembers {}, "not_a_package");
    plan.product = BuildProduct::Workspace { members: first };
    select_workspace(plan);
}
machine select_workspace(plan: @mutable BuildPlan) {
    let path: [u8; 5] = [116u8, 111u8, 111u8, 108u8, 115u8];
    let second: WorkspaceMembers = |workspace|::with_member(plan, WorkspaceMembers {}, @path[..]);
    plan.product = BuildProduct::Workspace { members: second };
}"#;
    let tools = build(r#"builder.product = BuildProduct::Package { name: "tools" };"#);
    let (status, report) = invoke(
        &[
            ("build.omg", workspace),
            ("tools/build.omg", &tools),
            ("tools/main.omg", "public machine answer() -> u32 { 42u32 }"),
        ],
        &["--check"],
    );
    assert!(status.success(), "{report}");

    let duplicate = workspace.replace(
        "plan.product = BuildProduct::Workspace { members: second };",
        r#"let repeated: WorkspaceMembers = |workspace|::with_member(plan, second, "tools");
            plan.product = BuildProduct::Workspace { members: repeated };"#,
    );
    assert_ne!(duplicate, workspace);
    let (status, report) = invoke(&[("build.omg", &duplicate)], &["--check"]);
    assert!(!status.success(), "{report}");
    assert!(
        report.contains("repeated workspace member `tools`"),
        "{report}"
    );
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
            "one original BuildPlan parameter",
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
        let source = format!(
            "use {{kit}}/{path} as lib; machine main() {{ let value: i32 = |lib|::marker(); }}"
        );
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

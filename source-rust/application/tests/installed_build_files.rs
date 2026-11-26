//! Installed Build files use the same captured paths and file-local aliases as other sources.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use std::process::Command;

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn build_files_retain_their_paths_aliases_and_checked_scope() {
    let temp = TemporaryDirectory::create("installed-build-files", false).unwrap();
    let project = temp.path().join("app");
    let library = temp.path().join("library");
    copy_directory(&support::repository_root().join("source/library"), &library);
    fs::create_dir(&project).unwrap();
    let providers = library.join("build");
    for directory in ["left", "right"] {
        fs::create_dir_all(providers.join(directory)).unwrap();
    }
    fs::write(
        providers.join("inner.omg"),
        "public machine inner_value() -> u32 { 7 }",
    )
    .unwrap();
    fs::write(
        providers.join("other.omg"),
        "public machine inner_value() -> u32 { 11 }",
    )
    .unwrap();
    fs::write(
        providers.join("left/score.omg"),
        "public machine score() -> u32 { 3 }",
    )
    .unwrap();
    fs::write(
        providers.join("right/score.omg"),
        r#"use inner.omg as inside;
use other.omg as other;
public machine score() -> u32 { |inside|::inner_value() +% |other|::inner_value() }"#,
    )
    .unwrap();
    let build = r#"use {omega}/build/left/score.omg as left;
use {omega}/build/right/score.omg as right;
machine build(builder: @mutable Build) {
    Build::package(builder, "checks");
    transition |left|::score() == 3u32 && |right|::score() == 18u32 {
        true -> BuildLog::write_line(@mutable builder.log, "read both"),
        _ -> Build::package(builder, "duplicate")
    }
}"#;
    let main = "public machine answer() -> u32 { 42 }";
    let check = |build: &str, main: &str| {
        fs::write(project.join("build.omg"), build).unwrap();
        fs::write(project.join("main.omg"), main).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&project)
            .env("OMEGA_LIBRARY_ROOT", &library)
            .args(["--check", "build.omg"])
            .output()
            .unwrap();
        (
            result.status.success(),
            String::from_utf8_lossy(&result.stderr).into_owned(),
        )
    };
    let (ok, reason) = check(build, main);
    assert!(ok, "{reason}");
    let (ok, reason) = check(&build.replace("== 18u32", "== 17u32"), main);
    assert!(!ok && reason.contains("more than once"), "{reason}");
    let (ok, reason) = check(&build.replace("|right|::score()", "inner_value()"), main);
    assert!(
        !ok && reason.contains("no visible declaration in its source scope"),
        "{reason}"
    );
    let (ok, reason) = check(&build.replace("left/score.omg", "missing/score.omg"), main);
    assert!(!ok && reason.contains("missing/score.omg"), "{reason}");
    let (ok, reason) = check(
        build,
        "use {omega}/build/inner.omg; public machine value() -> u32 { |inner|::inner_value() }",
    );
    assert!(!ok && reason.contains("Build scope"), "{reason}");
}

#[test]
fn bundled_registration_selects_the_declared_name_and_preserves_the_edge_ceiling() {
    let temp = TemporaryDirectory::create("installed-bundled-package", false).unwrap();
    let project = temp.path().join("client");
    let library = temp.path().join("relocated-installation");
    copy_directory(&support::repository_root().join("source/library"), &library);
    fs::create_dir(&project).unwrap();
    let package = library.join("physical-dir-17");
    fs::create_dir(&package).unwrap();
    fs::write(library.join("packages.tsv"), "answers\tphysical-dir-17\n").unwrap();
    let declaration = |name: &str| {
        format!(
            "machine build(plan: @mutable BuildPlan) {{ plan.product = BuildProduct::Package {{ name: \"{name}\" }}; }}"
        )
    };
    fs::write(package.join("build.omg"), declaration("answers")).unwrap();
    fs::write(package.join("answer.omg"),
        "public boundary trait AuditEffect {}\npublic machine charged() reaches AuditEffect {}\npublic machine free() {}").unwrap();
    fs::write(project.join("build.omg"), r#"use {omega}/build/dependency.omg;
machine build(plan: @mutable BuildPlan) {
    let mutable dep: DependencyDescription = |dependency|::from_source(Source::Bundled { name: "answers" });
    |dependency|::with_alias(@mutable dep, "tool");
    _ = |dependency|::add(plan, dep);
    plan.product = BuildProduct::InterpretedBuild { entry: "select.omg" };
}"#).unwrap();
    let write_entry = |export: &str| {
        fs::write(project.join("select.omg"), format!(
        "use {{tool}}/answer.omg;\nmachine build(plan: @mutable BuildPlan) {{ |answer|::{export}(); plan.product = BuildProduct::Package {{ name: \"client\" }}; }}"
    )).unwrap()
    };
    let check = || {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["--check", "--offline", "build.omg"])
            .current_dir(&project)
            .env("OMEGA_LIBRARY_ROOT", &library)
            .output()
            .unwrap();
        (
            output.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        )
    };
    write_entry("free");
    let (ok, output) = check();
    assert!(ok, "{output}");
    write_entry("charged");
    let (ok, output) = check();
    assert!(!ok && output.contains("AuditEffect"), "{output}");

    write_entry("free");
    fs::write(package.join("build.omg"), declaration("counterfeit")).unwrap();
    let (ok, output) = check();
    assert!(
        !ok && output.contains("counterfeit") && output.contains("answers"),
        "{output}"
    );

    let decoy = project.join("answers");
    fs::create_dir(&decoy).unwrap();
    fs::write(decoy.join("build.omg"), declaration("answers")).unwrap();
    fs::copy(package.join("answer.omg"), decoy.join("answer.omg")).unwrap();
    fs::write(library.join("packages.tsv"), "").unwrap();
    let (ok, output) = check();
    assert!(
        !ok && output.contains("does not supply bundled package named answers"),
        "{output}"
    );
}

#[test]
fn zeroed_source_selects_a_path_with_an_empty_location() {
    let project = TemporaryDirectory::create("zeroed-source-path", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"machine build(plan: @mutable BuildPlan) crashes Trap {
    let mutable selection: Source;
    transition selection {
        Source::Path { location } -> chosen(plan, location),
        _ -> unexpected()
    }
    machine chosen(plan: @mutable BuildPlan, location: @[u8]) {
        transition location.length == 0 {
            true -> declare(plan),
            false -> unexpected()
        }
    }
    machine declare(plan: @mutable BuildPlan) {
        plan.product = BuildProduct::Package { name: "zero_path" };
    }
    machine unexpected() { crash Trap; }
}"#,
    )
    .unwrap();
    fs::write(
        project.path().join("main.omg"),
        "public machine witness() {}",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(project.path())
        .env(
            "OMEGA_LIBRARY_ROOT",
            support::repository_root().join("source/library"),
        )
        .args(["--check", "--offline", "build.omg"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

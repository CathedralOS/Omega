//! The invoker supplies the package, its reach ceiling and its live directory independently.
#[path = "support/project.rs"]
mod project;
mod support;

use build_declarations::{BuildDeclaration, BuildDependencySource, ProjectName};
use package_compilation::BuildSnapshotRequest;
use package_manager::project::{BuildLibraryRequest, ProjectInput, RootBuildInputs, open_products};
use platform_custody::directory_mappings::{DirectoryAccess, DirectoryMappings};
use semantic_vocabulary::dependency_reach::DependencyReach;
use std::fs;
use target::TargetProfile;

#[test]
fn the_initial_build_reads_and_creates_only_with_the_appropriate_directory_grant() {
    let temp = project::stage(
        "initial-build-directories",
        &[
            (
                "package/grammar.omg",
                "public machine expected() -> u8 { 82u8 }",
            ),
            ("host/grammar.omg", "this host file is data"),
        ],
    );
    let package = temp.path().join("package");
    let host = temp.path().join("host");
    let entry = package.join("build.omg");
    fs::write(&entry, r#"use grammar.omg;
use {system}/filesystem.omg as files;
use {system}/filesystem_host.omg as paths;
machine build(plan: @mutable BuildPlan) blocks {
    let mutable data: [u8; 1] = [0u8];
    let read: |files|::IoResult = block |files|::Filesystem::read_all(
        "/grammar/sql.txt" as + |paths|::Path, @mutable data[..], 1u64);
    transition read {
        |files|::IoResult::Ok { count } -> matched(plan, count, data[0]),
        |files|::IoResult::Error { kind } -> refused(plan, kind)
    }
    machine matched(plan: @mutable BuildPlan, count: u64, value: u8) {
        transition count == 1u64 && value == |grammar|::expected() {
            true -> selected(plan),
            false -> wrong(plan)
        }
    }
    machine selected(plan: @mutable BuildPlan) { plan.product = BuildProduct::Package { name: "read_grammar" }; }
    machine refused(plan: @mutable BuildPlan, kind: |files|::ErrorKind) {
        transition kind {
            |files|::ErrorKind::PermissionDenied -> denied(plan),
            _ -> wrong(plan)
        }
    }
    machine denied(plan: @mutable BuildPlan) { plan.product = BuildProduct::Package { name: "ungranted" }; }
    machine wrong(plan: @mutable BuildPlan) { plan.product = BuildProduct::Package { name: "wrong" }; }
}"#).unwrap();

    let sdk = std::env::var_os("OMEGA_LIBRARY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::repository_root().join("source/library"));
    package_compilation::toolchain_library::configure(&sdk).unwrap();
    let library = BuildLibraryRequest {
        alias: ProjectName::parse("system").unwrap(),
        source: BuildDependencySource::Path(sdk.join("system").to_str().unwrap().to_owned()),
        max_reach: DependencyReach::from_checked_services(
            [
                "FilesystemContentRead",
                "FilesystemContentWrite",
                "FilesystemMetadataMutation",
                "FilesystemNamespaceMutation",
            ]
            .map(|name| format!(r"trait-application(declaration(nominal\(toolchain-owner\,path\({name}\)\)),arguments(),lifetimes())")),
        ),
    };
    let target = TargetProfile::host_if_supported().unwrap();
    let run = |directories, expected| {
        let products = open_products(
            ProjectInput::WithBuildLibraries {
                file: &entry,
                libraries: std::slice::from_ref(&library),
            },
            &[target],
            true,
            false,
            &temp.path().join(expected),
            RootBuildInputs {
                snapshot: Some(
                    BuildSnapshotRequest::for_activation(false).with_directories(directories),
                ),
                ..Default::default()
            },
        )
        .unwrap();
        let config = &products.select(target).unwrap().evaluated().configuration;
        let Some(BuildDeclaration::Package(package)) = &config.project_declaration else {
            panic!("expected a package");
        };
        assert_eq!(package.name.as_str(), expected);
    };
    let mut granted = DirectoryMappings::default();
    granted
        .grant(&host, "/grammar", DirectoryAccess::ReadOnly)
        .unwrap();
    fs::write(host.join("sql.txt"), b"R").unwrap();
    run(granted.clone(), "read_grammar");
    run(DirectoryMappings::default(), "ungranted");

    fs::write(&entry, r#"use {system}/filesystem.omg as files;
use {system}/filesystem_host.omg as paths;
machine build(plan: @mutable BuildPlan) blocks {
    let made: |files|::UnitResult = block |files|::Filesystem::create_dir(
        "/grammar/generated" as + |paths|::Path);
    transition made {
        |files|::UnitResult::Ok -> { plan.product = BuildProduct::Package { name: "created" }; },
        |files|::UnitResult::Error { kind } -> refused(plan, kind)
    }
    machine refused(plan: @mutable BuildPlan, kind: |files|::ErrorKind) {
        transition kind {
            |files|::ErrorKind::PermissionDenied -> { plan.product = BuildProduct::Package { name: "denied" }; },
            _ -> { plan.product = BuildProduct::Package { name: "unexpected_error" }; }
        }
    }
}"#).unwrap();
    run(granted, "denied");
    assert!(!host.join("generated").exists());
    let mut writable = DirectoryMappings::default();
    writable
        .grant(&host, "/grammar", DirectoryAccess::ReadWrite)
        .unwrap();
    run(writable, "created");
    assert!(host.join("generated").is_dir());
}

#[test]
fn directory_grants_leave_the_drivers_product_publication_and_root_imports_alone() {
    let target = TargetProfile::host_if_supported().unwrap();
    let build = format!(
        r#"use helper.omg;
machine build(plan: @mutable BuildPlan) {{
    |helper|::select(plan);
    Build::bind(plan, {}::ProgramEntry, main);
}}"#,
        target.target_name()
    );
    let temp = project::stage(
        "build-mapping-publication",
        &[
            ("app/build.omg", &build),
            (
                "app/helper.omg",
                r#"public machine select(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Application { name: "mapped" };
}"#,
            ),
            ("app/main.omg", "machine main() {}"),
            ("host/helper.omg", "not source"),
        ],
    );
    let app = temp.path().join("app");
    let host = temp.path().join("host");
    let mapping = format!("{}::/out", host.display());
    let invoke = |arguments: &[&str]| {
        let output = project::omega(&app, arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let result = invoke(&[
        "build",
        "--build-directories-readonly",
        &mapping,
        "build.omg",
    ]);
    let published = result
        .lines()
        .find_map(|line| line.strip_prefix("published native output to "))
        .map(std::path::Path::new)
        .unwrap_or_else(|| panic!("{result}"));
    assert!(
        published.starts_with(app.join("build")) && published.is_file(),
        "{result}"
    );
    let _ = invoke(&[
        "run",
        "--engine",
        "psi",
        "--build-directories-readwrite",
        &mapping,
        "build.omg",
    ]);
    assert_eq!(fs::read_dir(&host).unwrap().count(), 1);
}

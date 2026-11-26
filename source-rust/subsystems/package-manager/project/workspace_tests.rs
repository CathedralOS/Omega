//! Check the source graph published for a workspace whose catalog executes another package.
use crate::PackageBuildSession;
use crate::operations::begin_local_project_preparation;
use crate::resolution::graph::{CanonicalRootSourceRequest, CanonicalSourceClosureSubject};
use build_declarations::DependencyPurpose;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use target::TargetProfile;

fn write_package(root: &Path, build: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("build.omg"), build).unwrap();
    fs::write(root.join("main.omg"), "machine main() {}").unwrap();
}

fn git(directory: &Path, args: &[&str]) {
    let result = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn check_catalog(directory: &Path, fixture: &Path) {
    use artifacts::compile_timings::counters::source_build_execution_count;
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../source/library");
    package_compilation::toolchain_library::configure(&library).unwrap();
    let host = TargetProfile::host_if_supported().unwrap();
    let resolve = || {
        let preparation =
            begin_local_project_preparation(&directory.join("client/build.omg"), &[host], false)
                .unwrap();
        let root = preparation.acquire_root().unwrap();
        let mut acquisitions = preparation.source_acquisitions().unwrap();
        let mut session = PackageBuildSession::create(&directory.join("out")).unwrap();
        let result = super::resolve_build_sources(&mut session, root, &mut acquisitions, None);
        session.dispose(result)
    };
    let before = source_build_execution_count();
    let graph = resolve().unwrap().sources;
    let review = CanonicalSourceClosureSubject::from_resolved(
        &graph.closure.for_exact_target(host),
        Default::default(),
    )
    .unwrap();
    let mut packages: Vec<_> = review
        .packages()
        .iter()
        .map(|p| p.key().name().as_str())
        .collect();
    packages.sort_unstable();
    assert_eq!(packages, ["client", "workspace_selected"]);
    let [dependency] = review.catalog_dependencies() else {
        panic!("expected only the provider used to name the catalog member")
    };
    assert_eq!(
        dependency.observer.key().name().as_str(),
        "workspace_unselected"
    );
    assert_eq!(
        (dependency.purpose, dependency.ordinal),
        (DependencyPurpose::Build, 1)
    );
    assert_eq!(
        dependency.request.explicit_alias().unwrap().as_str(),
        "naming"
    );
    assert_eq!(
        dependency.provider.root().selected().key().name().as_str(),
        "catalog_helper"
    );
    assert_eq!(dependency.provider.packages().len(), 1);
    let CanonicalRootSourceRequest::Git {
        requested_revision, ..
    } = dependency.provider.root().request()
    else {
        panic!("the naming helper was requested from Git");
    };
    assert_eq!(requested_revision, "executed-build-fixture-v1");
    assert_eq!(
        source_build_execution_count() - before,
        6,
        "client, catalog, both members, alias candidate and shared naming helper each execute once"
    );

    let product = super::open_product(
        &directory.join("client/build.omg"),
        host,
        false,
        &directory.join("out"),
        None,
    )
    .unwrap();
    let inputs = product.package_inputs().unwrap();
    let handoffs: Vec<_> = inputs
        .dependency_build_handoffs()
        .map(|handoff| (inputs.package_name(handoff.package()), handoff.purpose()))
        .collect();
    assert_eq!(
        handoffs,
        [(Some("workspace_selected"), DependencyPurpose::Product)]
    );

    // The other provider result makes the observer declare the selected member's name.
    let helper = directory.join("catalog-helper");
    fs::copy(
        fixture.join("catalog-helper/wrong_result.omg"),
        helper.join("main.omg"),
    )
    .unwrap();
    git(
        &helper,
        &[
            "commit",
            "--quiet",
            "-am",
            "Change the catalog provider result",
        ],
    );
    git(&helper, &["branch", "-f", "also-helper", "HEAD"]);
    let Err(error) = resolve() else {
        panic!("the workspace would have two members with the same name")
    };
    assert!(
        error
            .to_string()
            .contains("workspace repeats declared package name `workspace_selected`"),
        "{error}"
    );
}

fn write_catalog(directory: &Path, fixture: &Path) {
    let workspace = directory.join("workspace");
    fs::create_dir(&workspace).unwrap();
    fs::write(
        workspace.join("build.omg"),
        r#"machine build(builder: @mutable Build) {
        Build::member(builder, "member");
        Build::member(builder, "unselected");
    }"#,
    )
    .unwrap();
    write_package(
        &workspace.join("member"),
        r#"machine build(builder: @mutable Build) {
        Build::package(builder, "workspace_selected");
    }"#,
    );
    let observer = workspace.join("unselected");
    fs::create_dir(&observer).unwrap();
    for name in ["main.omg", "name.omg"] {
        fs::copy(fixture.join("unselected").join(name), observer.join(name)).unwrap();
    }
    let build = fs::read_to_string(fixture.join("unselected/build.omg")).unwrap();
    // Looking up the default name of this candidate settles its dependencies first.
    // The subsequent explicit "naming" request reaches its child through a different Git ref.
    let build = build.replace("    let mutable naming_dependency:", r#"    let candidate: DependencyDescription = |dependency|::from_source(Source::Git {
        repository: "ssh://git@candidate-fixture.invalid/candidate.git", revision: "executed-build-fixture-v1",
        selection: PackageSelection::Root
    });
    let candidate_registration: DependencyRegistration = Build::build_dependency(builder, candidate);
    let mutable naming_dependency:"#);
    fs::write(
        observer.join("build.omg"),
        build.replace(
            "https://omega-source.invalid/catalog-helper.git",
            "ssh://git@helper-fixture.invalid/catalog-helper.git",
        ),
    )
    .unwrap();
    let helper = directory.join("catalog-helper");
    fs::create_dir(&helper).unwrap();
    for file in ["main.omg", "build.omg"] {
        fs::copy(fixture.join("catalog-helper").join(file), helper.join(file)).unwrap();
    }
    let candidate = directory.join("candidate");
    write_package(
        &candidate,
        r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
        Build::package(builder, "candidate");
        let helper: DependencyDescription = |dependency|::from_source(Source::Git {
            repository: "ssh://git@helper-fixture.invalid/catalog-helper.git", revision: "also-helper",
            selection: PackageSelection::Root
        });
        let registration: DependencyRegistration = Build::dependency(builder, helper);
    }"#,
    );
    for repository in [&workspace, &candidate, &helper] {
        git(
            repository,
            &["init", "--quiet", "-b", "executed-build-fixture-v1"],
        );
        git(repository, &["add", "."]);
        git(
            repository,
            &["commit", "--quiet", "-m", "Publish the catalog fixture"],
        );
    }
    git(&helper, &["branch", "also-helper", "HEAD"]);
    write_package(
        &directory.join("client"),
        r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
        Build::application(builder, "client");
        let member: DependencyDescription = |dependency|::from_source(Source::Git {
            repository: "ssh://git@omega-fixture.invalid/workspace.git", revision: "executed-build-fixture-v1",
            selection: PackageSelection::Named { package: "workspace_selected" }
        });
        let registration: DependencyRegistration = Build::dependency(builder, member);
        Build::bind(builder, windows_x86_64::ProgramEntry, main);
        Build::bind(builder, linux_x86_64::ProgramEntry, main);
        Build::bind(builder, macos_arm64::ProgramEntry, main);
        Build::bind(builder, linux_arm64::ProgramEntry, main);
    }"#,
    );
}

#[test]
fn catalog_review_keeps_the_provider_used_to_name_a_workspace_member() {
    use std::process::Command;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../compiler/tests/fixtures/packages/executed-workspace");
    const DIRECTORY: &str = "OMEGA_PROJECT_TEST_DIRECTORY";
    if let Some(directory) = std::env::var_os(DIRECTORY) {
        check_catalog(Path::new(&directory), &fixture);
        return;
    }
    let temp = TemporaryDirectory::create("project-workspace", false).unwrap();
    write_catalog(temp.path(), &fixture);
    // Scope the local SSH route and the process-wide execution counter to this test.
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "project::workspace_tests::catalog_review_keeps_the_provider_used_to_name_a_workspace_member",
            "--nocapture",
        ])
        .env(DIRECTORY, temp.path())
        .env("GIT_SSH_VARIANT", "simple")
        .env(
            "GIT_SSH_COMMAND",
            r#"sh -c 'case "$1" in
            git@omega-fixture.invalid) repo=workspace ;;
            git@helper-fixture.invalid) repo=catalog-helper ;;
            git@candidate-fixture.invalid) repo=candidate ;;
            *) exit 1 ;;
        esac
        exec git upload-pack "$OMEGA_PROJECT_TEST_DIRECTORY/$repo"' ssh"#,
        )
        .output()
        .unwrap();
    assert!(
        result.status.success() && String::from_utf8_lossy(&result.stdout).contains("1 passed;"),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

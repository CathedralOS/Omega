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

fn resolve_catalog(
    directory: &Path,
    host: TargetProfile,
) -> Result<super::ResolvedPackageProject, crate::PackageCommandError> {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../source/library");
    package_compilation::toolchain_library::configure(&library).unwrap();
    let preparation =
        begin_local_project_preparation(&directory.join("client/build.omg"), &[host], false)?;
    let root = preparation.acquire_root()?;
    let mut acquisitions = preparation.source_acquisitions()?;
    let mut session = PackageBuildSession::create(&directory.join("out"))?;
    let result = super::resolve_build_sources(&mut session, root, &mut acquisitions, None);
    session.dispose(result)
}

fn check_catalog(directory: &Path, fixture: &Path) {
    use artifacts::compile_timings::counters::source_build_execution_count;
    let host = TargetProfile::host_if_supported().unwrap();
    let resolve = || resolve_catalog(directory, host);
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
    let [candidate, dependency] = review.catalog_dependencies() else {
        panic!("expected the two exact inputs installed for the catalog naming Build")
    };
    assert_eq!(
        candidate.provider.root().selected().key().name().as_str(),
        "candidate"
    );
    assert_eq!(candidate.ordinal, 0);
    let preparing = dependency
        .preparation
        .as_ref()
        .expect("the package was supplied before naming began");
    assert_eq!(
        (preparing.ordinal, preparing.successor.as_str()),
        (0, "name.omg")
    );
    assert_eq!(candidate.preparation.as_ref(), Some(preparing));
    let text = review.canonical_text(Default::default()).unwrap();
    let recovered = CanonicalSourceClosureSubject::recover_text(&text, Default::default()).unwrap();
    assert_eq!(review, recovered);
    assert_eq!(
        dependency.observer.member().unwrap().key().name().as_str(),
        "workspace_unselected"
    );
    assert_eq!(
        (dependency.purpose, dependency.ordinal),
        (DependencyPurpose::Product, 1)
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
        7,
        "the catalog member runs its two entries and the shared helper builds once"
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

    // The provider keeps returning 64, so the selected package and every Product
    // declaration stay the same. Its newly fetched Git commit must still change
    // which source the catalog trusted while choosing its name.
    let helper = directory.join("catalog-helper");
    git(
        &helper,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "Same naming result from a new Git commit",
        ],
    );
    git(&helper, &["branch", "-f", "also-helper", "HEAD"]);
    let next_graph = resolve().unwrap().sources;
    let next_review = CanonicalSourceClosureSubject::from_resolved(
        &next_graph.closure.for_exact_target(host),
        Default::default(),
    )
    .unwrap();
    assert_eq!(review.packages(), next_review.packages());
    assert_eq!(
        review.dependency_requests(),
        next_review.dependency_requests()
    );
    assert_ne!(
        review.catalog_dependencies()[1]
            .provider
            .root()
            .selected()
            .resolution(),
        next_review.catalog_dependencies()[1]
            .provider
            .root()
            .selected()
            .resolution()
    );
    assert!(!review.same_source_graph(&next_review));
    assert_ne!(review.fingerprint(), next_review.fingerprint());
    // The other provider result makes the observer declare the selected member's name.
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
    _ = |dependency|::add(builder, candidate);
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
        _ = |dependency|::add(builder, helper);
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
        _ = |dependency|::add(builder, member);
        Build::bind(builder, windows_x86_64::ProgramEntry, main);
        Build::bind(builder, linux_x86_64::ProgramEntry, main);
        Build::bind(builder, macos_arm64::ProgramEntry, main);
        Build::bind(builder, linux_arm64::ProgramEntry, main);
    }"#,
    );
}

#[test]
fn catalog_review_keeps_the_provider_used_to_name_a_workspace_member() {
    isolated_catalog(write_catalog, check_catalog);
}

fn isolated_catalog(prepare: fn(&Path, &Path), check: fn(&Path, &Path)) {
    use std::process::Command;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../compiler/tests/fixtures/packages/executed-workspace");
    const DIRECTORY: &str = "OMEGA_PROJECT_TEST_DIRECTORY";
    if let Some(directory) = std::env::var_os(DIRECTORY) {
        check(Path::new(&directory), &fixture);
        return;
    }
    let temp = TemporaryDirectory::create("project-workspace", false).unwrap();
    prepare(temp.path(), &fixture);
    // Isolate Git objects and the process-wide execution counter. Each invocation
    // owns its temporary remote, including the branch it advances during review.
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            std::thread::current().name().unwrap(),
            "--nocapture",
        ])
        .env(DIRECTORY, temp.path())
        .env("XDG_CACHE_HOME", temp.path().join("cache"))
        .env("GIT_SSH_VARIANT", "simple")
        .env(
            "GIT_SSH_COMMAND",
            r#"sh -c 'case "$1" in
            git@omega-fixture.invalid) repo=workspace ;;
            git@helper-fixture.invalid) repo=catalog-helper ;;
            git@candidate-fixture.invalid) repo=candidate ;;
            git@roster-fixture.invalid) repo=roster ;;
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

#[test]
fn catalog_review_keeps_the_git_input_that_computes_workspace_membership() {
    isolated_catalog(write_root_catalog, check_root_catalog);
}

fn write_root_catalog(directory: &Path, fixture: &Path) {
    write_catalog(directory, fixture);
    let root = directory.join("workspace");
    fs::write(root.join("build.omg"), r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    let roster: DependencyDescription = |dependency|::from_source(Source::Git {
        repository: "ssh://git@roster-fixture.invalid/roster.git", revision: "main", selection: PackageSelection::Root
    });
    _ = |dependency|::add(builder, roster);
    Build::build_entry(builder, "members.omg");
}"#).unwrap();
    fs::write(
        root.join("members.omg"),
        r#"use {roster}/main.omg;
machine build(builder: @mutable Build) {
    transition |main|::include_member() {
        true -> selected(builder),
        _ -> other(builder)
    }
    machine selected(builder: @mutable Build) { Build::member(builder, "member"); }
    machine other(builder: @mutable Build) { Build::member(builder, "other"); }
}"#,
    )
    .unwrap();
    write_package(
        &root.join("other"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "other"); }"#,
    );
    git(&root, &["add", "."]);
    git(
        &root,
        &[
            "commit",
            "--quiet",
            "-m",
            "Compute the workspace member path in the successor",
        ],
    );
    let roster = directory.join("roster");
    write_package(
        &roster,
        r#"machine build(builder: @mutable Build) { Build::package(builder, "roster"); }"#,
    );
    fs::write(
        roster.join("main.omg"),
        "public machine include_member() -> bool { true }",
    )
    .unwrap();
    git(&roster, &["init", "--quiet", "-b", "main"]);
    git(&roster, &["add", "."]);
    git(
        &roster,
        &["commit", "--quiet", "-m", "Include the selected member"],
    );
}

fn check_root_catalog(directory: &Path, _: &Path) {
    let host = TargetProfile::host_if_supported().unwrap();
    let review = || {
        let project = resolve_catalog(directory, host).unwrap();
        CanonicalSourceClosureSubject::from_resolved(
            &project.sources.closure.for_exact_target(host),
            Default::default(),
        )
        .unwrap()
    };
    let initial = review();
    let [input] = initial.catalog_dependencies() else {
        panic!("the workspace's preparing input must appear in its catalog review");
    };
    assert_eq!(
        input.provider.root().selected().key().name().as_str(),
        "roster"
    );
    assert!(input.observer.member().is_none());
    let preparation = input.preparation.as_ref().unwrap();
    assert_eq!(
        (preparation.ordinal, preparation.successor.as_str()),
        (0, "members.omg")
    );
    let text = initial.canonical_text(Default::default()).unwrap();
    assert_eq!(
        initial,
        CanonicalSourceClosureSubject::recover_text(&text, Default::default()).unwrap()
    );
    let source_acquisition::ImmutableSourceResolution::Git { commit, .. } =
        input.observer.resolution()
    else {
        panic!("a Git workspace root must carry its repository revision")
    };
    let (prefix, observer_and_remainder) = text.split_once("observer workspace\n").unwrap();
    let commit = commit.to_hex();
    let replacement = if commit.starts_with('0') { "1" } else { "0" }.to_owned() + &commit[1..];
    let forged = format!(
        "{prefix}observer workspace\n{}",
        observer_and_remainder.replacen(&commit, &replacement, 1)
    );
    let error =
        CanonicalSourceClosureSubject::recover_text(&forged, Default::default()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("no selected member at its Git resolution"),
        "{error}"
    );
    let roster = directory.join("roster");
    git(
        &roster,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "Same roster result from a new commit",
        ],
    );
    let revised = review();
    assert_eq!(initial.packages(), revised.packages());
    assert_eq!(initial.dependency_requests(), revised.dependency_requests());
    assert_ne!(initial.fingerprint(), revised.fingerprint());
    assert!(!initial.same_source_graph(&revised));
    fs::write(
        roster.join("main.omg"),
        "public machine include_member() -> bool { false }",
    )
    .unwrap();
    git(
        &roster,
        &[
            "commit",
            "--quiet",
            "-am",
            "Choose the other workspace member",
        ],
    );
    let Err(error) = resolve_catalog(directory, host) else {
        panic!("the selected package left the workspace")
    };
    assert!(
        error
            .to_string()
            .contains("workspace has no declared package `workspace_selected`"),
        "{error}"
    );
}

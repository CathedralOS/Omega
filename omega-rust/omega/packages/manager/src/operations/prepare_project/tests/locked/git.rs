use super::super::super::Path;
use super::{
    ExternalSourceContext, LOCAL_PROJECT_CONTEXT, LocalSourceLimits, PackageRootSourceRequest,
    Project, TargetProfile, fs, prepare_local_project_in_storage,
};
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::operations::LocalProjectPreparationOptions;
use crate::resolution::graph::reconcile::resolve_package_source_closure;
use crate::resolution::source::{
    ResolvePackageSourceError, resolve_external_local_project_source, resolve_git_package_source,
};
use package_source::GitSourceRequest;

fn git(repository: &Path, arguments: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(repository)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_target_precedes_git_acquisition_even_when_the_recorded_selector_moved() {
    let project = Project::new("package");
    let repository = project.0.join("dependency");
    git(&repository, &["init", "--quiet"]);
    git(&repository, &["config", "user.name", "Omega Tests"]);
    git(
        &repository,
        &["config", "user.email", "omega@example.invalid"],
    );
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "--quiet", "-m", "accepted"]);
    git(&repository, &["branch", "-M", "main"]);
    let locator = "https://github.com/CathedralOS/locked-preparation-fixture.git";
    fs::write(project.root().join("build.omg"), format!("machine build(builder: &mut Build) {{ builder.package(\"root\"); builder.depend_as(\"dependency\", Source::Git {{ repository: \"{locator}\", revision: \"main\" }}); }}\n")).unwrap();
    let storage = project.storage();
    let context = ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT);
    let root_path = project.root().canonicalize().unwrap();
    let root = resolve_external_local_project_source(
        &root_path,
        &storage,
        LocalSourceLimits::default(),
        context.clone(),
    )
    .unwrap();
    let request = GitSourceRequest::for_local_test_repository(
        &repository,
        Some("main".into()),
        Some(locator),
    )
    .unwrap();
    // Fixture acquisition is routed locally; missing-target preparation must
    // fail before it could try the ordinary authored remote request.
    let closure = resolve_package_source_closure(
        PackageRootSourceRequest::ExternalLocal { requested_root: root_path, source_context: context },
        root.into_custody(),
        |_, edge| -> Result<_, ResolvePackageSourceError> {
            assert!(matches!(edge, DependencySourceRequest::Git { repository, revision, .. } if repository == locator && revision == "main"));
            Ok(resolve_git_package_source(&request, &storage, LocalSourceLimits::default())?.into_custody())
        },
    ).unwrap();
    let lock = project.lock_closure(&closure);
    fs::write(
        repository.join("main.omg"),
        "pub machine value() -> u64 { 99 }\n",
    )
    .unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "--quiet", "-m", "advanced"]);
    // Refresh the test cache's selector too, so selecting main afresh would
    // produce different source even with no network access.
    let advanced =
        resolve_git_package_source(&request, &storage, LocalSourceLimits::default()).unwrap();
    let accepted = lock
        .target(TargetProfile::host())
        .unwrap()
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "dependency")
        .unwrap();
    assert_ne!(advanced.resolution(), accepted.resolution());
    fs::write(
        project.root().join("main.omg"),
        "pub machine value() -> u64 { 19 }\n",
    )
    .unwrap();
    fs::rename(&repository, project.0.join("offline-repository")).unwrap();
    let other = TargetProfile::ALL
        .into_iter()
        .find(|target| *target != TargetProfile::host())
        .unwrap();
    let error = prepare_local_project_in_storage(
        &project.root().join("main.omg"),
        LocalProjectPreparationOptions {
            target: other,
            offline: false,
        },
        |_| panic!("missing target must reject before opening a cold Git cache"),
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("no accepted section for exact target")
    );
    assert_eq!(
        fs::read_to_string(project.root().join("omega.lock")).unwrap(),
        lock.canonical_text().unwrap()
    );
}

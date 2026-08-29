use super::super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::*;

#[test]
fn two_named_packages_share_one_exact_git_acquisition() {
    let repository = temp_root("git-shared-acquisition-repository");
    let cache = temp_root("git-shared-acquisition-cache");
    std::fs::create_dir_all(repository.join("packages")).expect("create repository");
    std::fs::write(
        repository.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.member("packages/first");
    builder.member("packages/second");
}
"#,
    )
    .expect("write workspace build");
    write_package(&repository.join("packages/first"), "first", None);
    write_package(&repository.join("packages/second"), "second", None);
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
    let acquisition = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        None,
        "https://github.com/CathedralOS/shared-acquisition.git",
    )
    .expect("validated local Git request");
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained Git resolver storage");
    let mut acquisitions = GitAcquisitionCache::default();
    let selected = |name| {
        crate::resolution::GitPackageSourceRequest::new(
            acquisition.clone(),
            crate::manifest::PackageSelection::Named(
                omega_package_source::PackageName::parse(name).expect("package name"),
            ),
        )
    };

    let first = acquisitions
        .resolve_selected(
            &selected("first"),
            SourceCacheLane::Retained(storage.git_sources()),
            LocalSourceLimits::default(),
        )
        .expect("select first member");
    let second = acquisitions
        .resolve_selected(
            &selected("second"),
            SourceCacheLane::Retained(storage.git_sources()),
            LocalSourceLimits::default(),
        )
        .expect("select second member");

    assert_eq!(acquisitions.acquisition_count(), 1);
    assert_eq!(first.acquisition_root(), second.acquisition_root());
    assert_eq!(first.resolution(), second.resolution());

    drop(storage);
    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}

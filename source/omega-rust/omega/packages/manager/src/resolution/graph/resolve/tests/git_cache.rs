use super::super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::super::dependencies::register_git_repository;
use super::*;
use std::collections::BTreeMap;

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
        crate::resolution::source::GitPackageSourceRequest::new(
            acquisition.clone(),
            crate::manifest::PackageSelection::Named(
                crate::manifest::PackageName::parse(name).expect("package name"),
            ),
        )
    };

    let first = acquisitions
        .resolve_selected(
            &selected("first"),
            SourceCacheLane::Retained(storage.git_sources()),
            SourceCacheLane::Retained(storage.workspace_members()),
            LocalSourceLimits::default(),
        )
        .expect("select first member");
    std::fs::write(
        repository.join("packages/second/drift.omg"),
        b"machine Drift::value() {}\n",
    )
    .expect("write moving-branch drift");
    run_test_git(&repository, ["add", "."]);
    run_test_git(
        &repository,
        [
            "commit",
            "--quiet",
            "-m",
            "move branch after first selection",
        ],
    );
    let second = acquisitions
        .resolve_selected(
            &selected("second"),
            SourceCacheLane::Retained(storage.git_sources()),
            SourceCacheLane::Retained(storage.workspace_members()),
            LocalSourceLimits::default(),
        )
        .expect("select second member");

    assert_eq!(acquisitions.acquisition_count(), 1);
    assert_eq!(first.source().commit(), second.source().commit());
    assert!(!second.snapshot_root().join("drift.omg").exists());
    assert_ne!(first.snapshot_root(), second.snapshot_root());
    assert_eq!(first.resolution(), second.resolution());
    assert_ne!(first.selection_evidence(), second.selection_evidence());
    assert_eq!(
        first
            .selection_evidence()
            .git_workspace()
            .expect("first workspace selection")
            .workspace_evidence(),
        second
            .selection_evidence()
            .git_workspace()
            .expect("second workspace selection")
            .workspace_evidence(),
    );
    let mut workspaces = BTreeMap::new();
    register_git_repository(
        &mut workspaces,
        &acquisition,
        first.key().source_lineage(),
        first.resolution(),
        first.selection_evidence(),
        first.source_limits(),
    )
    .expect("register first selected package");
    register_git_repository(
        &mut workspaces,
        &acquisition,
        second.key().source_lineage(),
        second.resolution(),
        second.selection_evidence(),
        second.source_limits(),
    )
    .expect("same workspace evidence reconciles across selected members");
    assert_eq!(workspaces.len(), 1);

    drop(storage);
    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}

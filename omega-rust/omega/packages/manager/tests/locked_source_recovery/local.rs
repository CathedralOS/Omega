use super::*;
use omega_package_manager::resolution::graph::{
    PackageSourceClosureResolutionError, ResolveLockedPackageClosureError,
    resolve_external_local_project_closure_with_storage,
};
use omega_package_manager::resolution::source::ResolvePackageSourceError;
use omega_package_source::SourceResolveError;

fn diamond(tree: &Tree, storage: &SourceResolverStorage) -> ResolvedPackageSourceClosure {
    package(
        &tree.path("sources/root"),
        "root",
        concat!(
            " builder.depend_as(\"left_branch\", Source::Path { location: \"../left\" });\n",
            " builder.depend_as(\"right_branch\", Source::Path { location: \"../right\" });\n",
        ),
    );
    for (directory, alias) in [("left", "shared_left"), ("right", "shared_right")] {
        package(
            &tree.path(&format!("sources/{directory}")),
            "same-name",
            &format!(
                " builder.depend_as(\"{alias}\", Source::Path {{ location: \"../shared\" }});\n"
            ),
        );
    }
    package(&tree.path("sources/shared"), "shared", "");
    resolve_external_local_project_closure_with_storage(
        tree.path("sources/root"),
        ExternalSourceContext::derive(b"locked-source-diamond"),
        storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap()
}

#[test]
fn recovered_diamond_preserves_request_occurrences_and_builds_fresh_compiler_inputs() {
    let tree = Tree::new();
    let (lock, request) = {
        let storage = tree.storage("old-cache");
        let closure = diamond(&tree, &storage);
        capture_lock(&closure, &tree.path("build"))
    };
    let storage = tree.storage("new-cache");
    let fresh = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    assert_fresh_matches(&lock, &fresh);
    let subject = lock.target(TARGET).unwrap().source();
    assert_eq!(subject.packages().len(), 4);
    assert_eq!(subject.dependency_requests().len(), 4);
    let mut aliases = subject
        .dependency_requests()
        .iter()
        .map(|request| request.alias().as_str())
        .collect::<Vec<_>>();
    aliases.sort_unstable();
    assert_eq!(
        aliases,
        ["left_branch", "right_branch", "shared_left", "shared_right"]
    );
    assert_eq!(
        subject
            .packages()
            .iter()
            .filter(|package| package.key().name().as_str() == "same-name")
            .count(),
        2
    );
    // Exact canonical subject equality above also pins requester-local ordinals,
    // selections and declared dependency order; cache paths are not identities.
    let fresh_cache = fs::canonicalize(tree.path("new-cache")).unwrap();
    for custody in fresh.custodies() {
        assert!(custody.snapshot_root().starts_with(&fresh_cache));
    }
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions {
                source_limits: LocalSourceLimits {
                    max_bytes: 1,
                    ..LocalSourceLimits::default()
                },
                ..LockedSourceRecoveryOptions::default()
            }
        ),
        Err(RecoverLockedSourcesError::Resolution(
            ResolveLockedPackageClosureError::Source(ResolvePackageSourceError::Source(
                SourceResolveError::TooManyBytes { limit: 1 }
            ))
        ))
    ));
}

#[test]
fn unavailable_or_changed_local_source_does_not_erase_readable_accepted_policy() {
    let tree = Tree::new();
    let (lock, request) = {
        let storage = tree.storage("old-cache");
        let closure = diamond(&tree, &storage);
        capture_lock(&closure, &tree.path("build"))
    };
    let text = lock.canonical_text().unwrap();
    let storage = tree.storage("new-cache");
    fs::write(
        tree.path("sources/shared/main.omg"),
        "pub machine value() -> u64 { 8 }\n",
    )
    .unwrap();
    let changed = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap_err();
    let RecoverLockedSourcesError::Resolution(mut changed) = changed else {
        panic!("changed source must fail source resolution: {changed:?}");
    };
    // The shared node may be reached through either branch, but its mismatch
    // must remain the reason rather than an unrelated storage/adapter failure.
    while let ResolveLockedPackageClosureError::Closure(error) = changed {
        let PackageSourceClosureResolutionError::Adapter { error, .. } = *error else {
            panic!("unexpected closure error: {error:?}");
        };
        changed = error;
    }
    assert!(matches!(
        changed,
        ResolveLockedPackageClosureError::SourceMismatch { package, .. }
            if package.name().as_str() == "shared"
    ));
    fs::rename(
        tree.path("sources/root"),
        tree.path("sources/unavailable-root"),
    )
    .unwrap();
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default()
        ),
        Err(RecoverLockedSourcesError::Resolution(
            ResolveLockedPackageClosureError::Source(ResolvePackageSourceError::Source(
                SourceResolveError::Io { .. }
            ))
        ))
    ));
    assert_eq!(lock.canonical_text().unwrap(), text);
    assert_eq!(
        PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap(),
        lock
    );
}

#[test]
fn missing_target_mismatched_request_and_lowered_graph_limits_precede_storage_access() {
    let tree = Tree::new();
    let (lock, request) = {
        let storage = tree.storage("old-cache");
        let closure = diamond(&tree, &storage);
        capture_lock(&closure, &tree.path("build"))
    };
    let storage = tree.storage("new-cache");
    fs::rename(tree.path("new-cache"), tree.path("retired-cache")).unwrap();
    let mut wrong_request = request.clone();
    let PackageRootSourceRequest::ExternalLocal { source_context, .. } = &mut wrong_request else {
        panic!("fixture is an external local project");
    };
    *source_context = ExternalSourceContext::derive(b"different-locked-source-context");
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &wrong_request,
            &storage,
            LockedSourceRecoveryOptions::default()
        ),
        Err(RecoverLockedSourcesError::Resolution(
            ResolveLockedPackageClosureError::RootRequestMismatch
        ))
    ));
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TargetProfile::LinuxArm64,
            &request,
            &storage,
            LockedSourceRecoveryOptions {
                git_acquisition: GitExactRevisionAcquisition::AllowFetch,
                ..LockedSourceRecoveryOptions::default()
            }
        ),
        Err(RecoverLockedSourcesError::MissingTarget {
            target: TargetProfile::LinuxArm64
        })
    ));
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions {
                closure_limits: PackageSourceClosureLimits {
                    max_packages: 3,
                    ..PackageSourceClosureLimits::default()
                },
                ..LockedSourceRecoveryOptions::default()
            }
        ),
        Err(RecoverLockedSourcesError::Resolution(
            ResolveLockedPackageClosureError::LimitExceeded
        ))
    ));
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions {
                subject_limits: CanonicalSourceClosureSubjectLimits {
                    maximum_request_bytes: 0,
                    ..CanonicalSourceClosureSubjectLimits::default()
                },
                ..LockedSourceRecoveryOptions::default()
            }
        ),
        Err(RecoverLockedSourcesError::Resolution(
            ResolveLockedPackageClosureError::Subject(_)
        ))
    ));
    assert!(!tree.path("new-cache").exists());
}

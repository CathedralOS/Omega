#[test]
fn offline_update_resume_with_only_candidate_cached_retains_lock_policy_comparison() {
    run(
        "offline_update_resume_with_only_candidate_cached_retains_lock_policy_comparison",
        |fixture| {
            use package_manager::operations::PackageCommandKind;
            use package_source::git::resolution::resolve_git_source_with_storage;
            use package_source::{GitSourceRequest, LocalSourceLimits, SourceResolverStorage};

            fixture.package("repository", "git-library", "");
            let old = fixture.commit();
            assert_eq!(
                fixture.install(None, None).unwrap().status,
                PackageCommandStatus::Published
            );
            fixture.write("repository/main.omg", &format!("{PURE}{ASSUMPTION}"));
            let proposed = fixture.commit();
            let pending = fixture
                .execute(PackageCommand::Update {
                    packages: Vec::new(),
                    revision: None,
                })
                .unwrap();
            assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
            let before = fixture.pair();
            let proposal = fixture.read("root/build/package-manager/proposal");
            let review_before: Vec<_> = pending
                .review_paths
                .iter()
                .map(|path| std::fs::read(path).unwrap())
                .collect();

            // Warm only the candidate in fresh storage. A shallow fetch of HEAD
            // cannot supply the old accepted tree to source-diff recovery.
            std::fs::rename(fixture.path("cache"), fixture.path("previous-cache")).unwrap();
            let storage = SourceResolverStorage::for_hardened_base(fixture.path("cache")).unwrap();
            let request = GitSourceRequest::new(REPOSITORY, None).unwrap();
            let calls = fixture.transport_calls();
            drop(
                resolve_git_source_with_storage(&request, &storage, LocalSourceLimits::default())
                    .unwrap(),
            );
            assert!(fixture.transport_calls() > calls);
            drop(storage);
            fixture.write("repository/main.omg", "pub machine value() -> u64 { 99 }\n");
            assert_ne!(fixture.commit(), proposed);

            let resume = PackageCommand::Resume {
                kind: PackageCommandKind::Update,
            };
            let still_pending = fixture.execute_with_offline(resume.clone(), true).unwrap();
            assert_eq!(still_pending.status, PackageCommandStatus::ReviewRequired);
            assert!(
                still_pending
                    .report
                    .contains("recorded Git commit/tree could not be recovered or verified"),
                "{}",
                still_pending.report
            );
            assert!(
                still_pending
                    .report
                    .contains("standalone candidate audit only"),
                "{}",
                still_pending.report
            );
            assert!(
                still_pending
                    .report
                    .contains("Capability comparison uses accepted lock policy"),
                "{}",
                still_pending.report
            );
            assert_eq!(fixture.pair(), before);
            assert_eq!(
                fixture.read("root/build/package-manager/proposal"),
                proposal
            );
            for (path, bytes) in pending.review_paths.iter().zip(review_before) {
                assert_eq!(
                    std::fs::read(path).unwrap(),
                    bytes,
                    "missing old source must not change policy findings or decisions"
                );
            }
            let source = fixture.read("root/build/package-manager/source-diff.txt");
            assert!(
                source.contains(&format!("candidate_git_commit {proposed}\n")),
                "{source}"
            );
            assert!(
                !source.contains(&format!("baseline_git_commit {old}\n")),
                "{source}"
            );

            fixture.accept_required(&still_pending);
            let published = fixture.execute_with_offline(resume, true).unwrap();
            assert_eq!(
                published.status,
                PackageCommandStatus::Published,
                "{}",
                published.report
            );
            assert!(
                published.report.contains("standalone candidate audit only"),
                "{}",
                published.report
            );
            assert!(
                published
                    .report
                    .contains("Capability comparison uses accepted lock policy"),
                "{}",
                published.report
            );
            assert!(fixture.read("root/omega.lock").contains(&proposed));
            assert!(!fixture.path("root/build/package-manager/proposal").exists());
        },
    );
}

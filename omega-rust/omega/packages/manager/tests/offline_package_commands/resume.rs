#[test]
fn offline_install_resume_publishes_the_reviewed_named_pin_after_head_moves() {
    run(
        "offline_install_resume_publishes_the_reviewed_named_pin_after_head_moves",
        |fixture| {
            fixture.write(
                "repository/modules/selected/main.omg",
                &format!("{PURE}{ASSUMPTION}"),
            );
            let proposed = fixture.commit();
            let before = fixture.pair();
            let pending = fixture.install(Some("exact-math"), Some("math")).unwrap();
            assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
            assert_eq!(fixture.pair(), before);
            assert!(fixture.transport_calls() > 0);
            fixture.write("repository/modules/selected/main.omg", PURE);
            assert_ne!(fixture.commit(), proposed);
            let resume = PackageCommand::Resume {
                kind: PackageCommandKind::Install,
            };
            assert_eq!(
                fixture
                    .execute_with_offline(resume.clone(), true)
                    .unwrap()
                    .status,
                PackageCommandStatus::ReviewRequired
            );
            assert_eq!(fixture.pair(), before);
            fixture.accept_required(&pending);
            let published = fixture.execute_with_offline(resume, true).unwrap();
            assert_eq!(
                published.status,
                PackageCommandStatus::Published,
                "{}",
                published.report
            );
            assert_git_pin(fixture, &proposed);
            let lock = fixture.lock();
            let edge = &lock.targets()[0].source().dependency_requests()[0];
            assert_eq!(edge.alias().as_str(), "math");
            assert_eq!(edge.selected().key().name().as_str(), "exact-math");
            assert!(!fixture.path("root/build/package-manager/proposal").exists());
        },
    );
}

#[test]
fn offline_resume_with_missing_proposed_cache_cannot_publish_install_or_update() {
    run(
        "offline_resume_with_missing_proposed_cache_cannot_publish_install_or_update",
        |fixture| {
            fixture.package("repository", "git-library", "");
            fixture.write("repository/main.omg", &format!("{PURE}{ASSUMPTION}"));
            fixture.commit();
            let mut pending = fixture.install(None, None).unwrap();
            for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
                assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
                fixture.accept_required(&pending);
                let before = fixture.pair();
                let proposal = fixture.read("root/build/package-manager/proposal");
                let reviews: Vec<_> = pending
                    .review_paths
                    .iter()
                    .map(|path| fs::read(path).unwrap())
                    .collect();
                fs::rename(fixture.path("cache"), fixture.path("previous-cache")).unwrap();
                let resume = PackageCommand::Resume { kind };
                let error = fixture
                    .execute_with_offline(resume.clone(), true)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("recorded Git revision"), "{error}");
                assert!(
                    error.contains("is unavailable in the retained cache"),
                    "{error}"
                );
                assert_eq!(fixture.pair(), before);
                assert_eq!(
                    fixture.read("root/build/package-manager/proposal"),
                    proposal
                );
                for (path, bytes) in pending.review_paths.iter().zip(reviews) {
                    assert_eq!(fs::read(path).unwrap(), bytes);
                }
                fs::rename(
                    fixture.path("cache"),
                    fixture.path(format!("empty-{kind:?}").as_str()),
                )
                .unwrap();
                fs::rename(fixture.path("previous-cache"), fixture.path("cache")).unwrap();
                assert_eq!(
                    fixture.execute_with_offline(resume, true).unwrap().status,
                    PackageCommandStatus::Published
                );
                if kind == PackageCommandKind::Install {
                    fixture.write("repository/main.omg", PURE);
                    fixture.commit();
                    pending = fixture
                        .execute(PackageCommand::Update {
                            packages: Vec::new(),
                            revision: None,
                        })
                        .unwrap();
                }
            }
        },
    );
}

#[test]
fn offline_update_resume_uses_candidate_pin_after_head_moves_again() {
    run(
        "offline_update_resume_uses_candidate_pin_after_head_moves_again",
        |fixture| {
            fixture.package("repository", "git-library", "");
            let old = fixture.commit();
            assert_eq!(
                fixture.install(None, None).unwrap().status,
                PackageCommandStatus::Published
            );
            fixture.write("repository/main.omg", &format!("{PURE}{ASSUMPTION}"));
            let proposed = fixture.commit();
            let before = fixture.pair();
            let pending = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["git_library".into()],
                    revision: None,
                })
                .unwrap();
            assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
            assert_eq!(fixture.pair(), before);
            fixture.accept_required(&pending);
            fixture.write("repository/main.omg", "pub machine value() -> u64 { 99 }\n");
            assert_ne!(fixture.commit(), proposed);
            let published = fixture
                .execute_with_offline(
                    PackageCommand::Resume {
                        kind: PackageCommandKind::Update,
                    },
                    true,
                )
                .unwrap();
            assert_eq!(
                published.status,
                PackageCommandStatus::Published,
                "{}",
                published.report
            );
            assert_git_pin(fixture, &proposed);
            let source = fixture.read("root/build/package-manager/source-diff.txt");
            assert!(
                source.contains(&format!("baseline_git_commit {old}\n")),
                "{source}"
            );
            assert!(
                source.contains(&format!("candidate_git_commit {proposed}\n")),
                "{source}"
            );
        },
    );
}

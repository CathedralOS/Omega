#[test]
fn offline_new_git_install_fails_without_changing_project_files() {
    run(
        "offline_new_git_install_fails_without_changing_project_files",
        |fixture| {
            fixture.package("repository", "git-library", "");
            fixture.commit();
            let before = fixture.pair();
            let error = fixture
                .execute_with_offline(
                    PackageCommand::Install {
                        source: REPOSITORY.into(),
                        revision: None,
                        alias: None,
                        package: None,
                    },
                    true,
                )
                .unwrap_err()
                .to_string();
            assert!(error.contains("offline"), "{error}");
            assert_eq!(fixture.pair(), before);
            assert_eq!(fixture.transport_calls(), 0);
            assert!(!fixture.path("root/build/package-manager/proposal").exists());
        },
    );
}

#[test]
fn offline_update_cannot_refresh_selected_or_unpinned_warm_git_sources() {
    run(
        "offline_update_cannot_refresh_selected_or_unpinned_warm_git_sources",
        |fixture| {
            fixture.package("repository", "git-library", "");
            fixture.commit();
            assert_eq!(
                fixture.install(None, None).unwrap().status,
                PackageCommandStatus::Published
            );
            fixture.write("repository/main.omg", "pub machine value() -> u64 { 99 }\n");
            let moved = fixture.commit();
            for command in [
                PackageCommand::Update {
                    packages: Vec::new(),
                    revision: None,
                },
                PackageCommand::Update {
                    packages: vec!["git_library".into()],
                    revision: None,
                },
                PackageCommand::Update {
                    packages: vec!["git_library".into()],
                    revision: Some(moved),
                },
            ] {
                let before = fixture.pair();
                let error = fixture
                    .execute_with_offline(command, true)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("offline"), "{error}");
                assert_eq!(fixture.pair(), before);
                assert!(!fixture.path("root/build/package-manager/proposal").exists());
            }
            fs::rename(
                fixture.path("root/omega.lock"),
                fixture.path("previous.lock"),
            )
            .unwrap();
            let before = fixture.pair();
            let error = fixture
                .execute_with_offline(
                    PackageCommand::Update {
                        packages: Vec::new(),
                        revision: None,
                    },
                    true,
                )
                .unwrap_err()
                .to_string();
            assert!(error.contains("offline"), "{error}");
            assert_eq!(fixture.pair(), before);
        },
    );
}

#[test]
fn offline_local_install_and_update_publish_current_source() {
    run(
        "offline_local_install_and_update_publish_current_source",
        |fixture| {
            fixture.package("local", "local-library", "");
            let installed = fixture
                .execute_with_offline(
                    PackageCommand::Install {
                        source: "../local".into(),
                        revision: None,
                        alias: None,
                        package: None,
                    },
                    true,
                )
                .unwrap();
            assert_eq!(
                installed.status,
                PackageCommandStatus::Published,
                "{}",
                installed.report
            );
            let before = fixture.pair();
            fixture.write("local/main.omg", "pub machine value() -> u64 { 99 }\n");
            let updated = fixture
                .execute_with_offline(
                    PackageCommand::Update {
                        packages: vec!["local_library".into()],
                        revision: None,
                    },
                    true,
                )
                .unwrap();
            assert_eq!(
                updated.status,
                PackageCommandStatus::Published,
                "{}",
                updated.report
            );
            let after = fixture.pair();
            assert_eq!(after.0, before.0);
            assert_ne!(after.1, before.1);
            let source = fixture.read("root/build/package-manager/source-diff.txt");
            assert!(
                source.contains("{ 7 }") && source.contains("{ 99 }"),
                "{source}"
            );
            assert_eq!(fixture.transport_calls(), 0);
        },
    );
}

#[test]
fn offline_locked_inspection_checks_edited_root_without_refreshing_git() {
    run(
        "offline_locked_inspection_checks_edited_root_without_refreshing_git",
        |fixture| {
            fixture.package(
                "repository/modules/selected",
                "exact-math",
                " builder.depend(Source::Path { location: \"../other\" });\n",
            );
            let accepted = fixture.commit();
            assert_eq!(
                fixture.install(Some("exact-math"), None).unwrap().status,
                PackageCommandStatus::Published
            );
            assert!(fixture.transport_calls() > 0);
            fixture.write(
                "repository/modules/other/main.omg",
                "pub machine value() -> u64 { 999 }\n",
            );
            let moved = fixture.commit();
            fixture.write("root/main.omg", &format!("{PURE}{ASSUMPTION}"));
            let before = fixture.pair();
            let inspected = inspect_with_offline(fixture, true);
            assert!(inspected.complete, "{}", inspected.report);
            assert!(inspected.requires_decision, "{}", inspected.report);
            assert!(
                inspected.report.contains("trusted_zero"),
                "{}",
                inspected.report
            );
            assert!(
                inspected.report.matches(&accepted).count() >= 2,
                "{}",
                inspected.report
            );
            assert!(!inspected.report.contains(&moved), "{}", inspected.report);
            assert_eq!(fixture.pair(), before);
        },
    );
}

#[test]
fn offline_cold_inspection_retains_accepted_policy_and_project_files() {
    run(
        "offline_cold_inspection_retains_accepted_policy_and_project_files",
        |fixture| {
            fixture.package("repository", "git-library", "");
            fixture.write("repository/main.omg", &format!("{PURE}{ASSUMPTION}"));
            let accepted = fixture.commit();
            let pending = fixture.install(None, None).unwrap();
            assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
            fixture.accept_required(&pending);
            assert_eq!(
                fixture.resume().unwrap().status,
                PackageCommandStatus::Published
            );
            let before = fixture.pair();
            let reviews: Vec<_> = pending
                .review_paths
                .iter()
                .map(|path| std::fs::read(path).unwrap())
                .collect();
            std::fs::rename(fixture.path("cache"), fixture.path("previous-cache")).unwrap();
            let inspected = inspect_with_offline(fixture, true);
            assert!(!inspected.complete, "{}", inspected.report);
            assert!(
                inspected.report.contains("fresh-analysis unavailable"),
                "{}",
                inspected.report
            );
            assert!(inspected.report.contains(&accepted), "{}", inspected.report);
            assert!(
                inspected.report.contains("trusted_zero"),
                "{}",
                inspected.report
            );
            assert_eq!(fixture.pair(), before);
            for (path, bytes) in pending.review_paths.iter().zip(reviews) {
                assert_eq!(std::fs::read(path).unwrap(), bytes);
            }
            assert!(!fixture.path("root/build/package-manager/proposal").exists());
        },
    );
}

#[test]
fn offline_unlocked_inspection_cannot_use_a_warm_selector_as_a_pin() {
    run(
        "offline_unlocked_inspection_cannot_use_a_warm_selector_as_a_pin",
        |fixture| {
            fixture.package("repository", "git-library", "");
            fixture.commit();
            assert_eq!(
                fixture.install(None, None).unwrap().status,
                PackageCommandStatus::Published
            );
            std::fs::rename(
                fixture.path("root/omega.lock"),
                fixture.path("previous.lock"),
            )
            .unwrap();
            let before = fixture.pair();
            let inspected = inspect_with_offline(fixture, true);
            assert!(!inspected.complete, "{}", inspected.report);
            assert!(inspected.report.contains("offline"), "{}", inspected.report);
            assert_eq!(fixture.pair(), before);
        },
    );
}

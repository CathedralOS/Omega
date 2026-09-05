use super::fixture::*;
use package_manager::operations::{PackageCommand, PackageCommandStatus};

#[test]
fn update_renders_exact_old_git_root_after_selector_moves() {
    run(
        "update_renders_exact_old_git_root_after_selector_moves",
        |fixture| {
            fixture.package("repository", "git-library", "");
            let old = fixture.commit();
            assert_eq!(
                fixture.install(None, None).unwrap().status,
                PackageCommandStatus::Published
            );
            fixture.write("repository/main.omg", "pub machine value() -> u64 { 9 }\n");
            let new = fixture.commit();
            let updated = fixture
                .execute(PackageCommand::Update {
                    packages: Vec::new(),
                    revision: None,
                })
                .unwrap();
            assert_eq!(
                updated.status,
                PackageCommandStatus::Published,
                "{}",
                updated.report
            );
            assert!(
                updated.report.contains("Source diff: git-library"),
                "{}",
                updated.report
            );
            let source = fixture.read("root/build/package-manager/source-diff.txt");
            assert!(
                source.contains(&format!("baseline_git_commit {old}\n")),
                "{source}"
            );
            assert!(
                source.contains(&format!("candidate_git_commit {new}\n")),
                "{source}"
            );
            assert!(source.contains("{ 7 }"));
            assert!(source.contains("{ 9 }"));
        },
    );
}

#[test]
fn named_and_relative_members_use_the_accepted_repository_pin_with_cold_storage() {
    run(
        "named_and_relative_members_use_the_accepted_repository_pin_with_cold_storage",
        |fixture| {
            fixture.package(
                "repository/modules/selected",
                "exact-math",
                " builder.depend_as(\"other\", Source::Path { location: \"../other\" });\n",
            );
            let old = fixture.commit();
            assert_eq!(
                fixture.install(Some("exact-math"), None).unwrap().status,
                PackageCommandStatus::Published
            );
            // This server permits fetching advertised exact objects. The
            // command requests the commit, never this tag or moving HEAD.
            fixture.git(&["tag", "accepted-source", &old]);
            // Displace this fixture's cache to require exact old-commit acquisition
            // after the candidate has already warmed a different revision.
            std::fs::rename(fixture.path("cache"), fixture.path("previous-cache")).unwrap();
            fixture.write(
                "repository/modules/selected/main.omg",
                "pub machine value() -> u64 { 13 }\n",
            );
            fixture.write(
                "repository/modules/other/main.omg",
                "pub machine value() -> u64 { 17 }\n",
            );
            let new = fixture.commit();
            let updated = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["exact_math".into()],
                    revision: None,
                })
                .unwrap();
            assert_eq!(
                updated.status,
                PackageCommandStatus::Published,
                "{}",
                updated.report
            );
            assert!(
                updated.report.contains("Source diff: exact-math"),
                "{}",
                updated.report
            );
            assert!(
                updated.report.contains("Source diff: other-library"),
                "{}",
                updated.report
            );
            let source = fixture.read("root/build/package-manager/source-diff.txt");
            assert_eq!(
                source
                    .matches(&format!("baseline_git_commit {old}\n"))
                    .count(),
                2,
                "{source}"
            );
            assert_eq!(
                source
                    .matches(&format!("candidate_git_commit {new}\n"))
                    .count(),
                2,
                "{source}"
            );
        },
    );
}

#[test]
fn unavailable_old_git_source_preserves_policy_comparison_without_selector_fallback() {
    run(
        "unavailable_old_git_source_preserves_policy_comparison_without_selector_fallback",
        |fixture| {
            fixture.package("repository", "git-library", "");
            fixture.commit();
            assert_eq!(
                fixture.install(None, None).unwrap().status,
                PackageCommandStatus::Published
            );
            let accepted = fixture.lock();
            std::fs::rename(fixture.path("cache"), fixture.path("previous-cache")).unwrap();
            // The fixture server refuses unadvertised old objects. A new unrelated
            // history still supplies a valid candidate with the same source lineage.
            fixture.git(&["checkout", "--orphan", "replacement"]);
            fixture.write("repository/main.omg", "pub machine value() -> u64 { 19 }\n");
            let new = fixture.commit();
            let branches =
                fixture.git(&["for-each-ref", "--format=%(refname:short)", "refs/heads"]);
            for branch in branches.lines().filter(|branch| *branch != "replacement") {
                fixture.git(&["branch", "-D", branch]);
            }
            let updated = fixture
                .execute(PackageCommand::Update {
                    packages: Vec::new(),
                    revision: None,
                })
                .unwrap();
            assert_eq!(
                updated.status,
                PackageCommandStatus::Published,
                "{}",
                updated.report
            );
            assert!(
                updated
                    .report
                    .contains("recorded Git commit/tree could not be recovered or verified"),
                "{}",
                updated.report
            );
            assert!(updated.report.contains("standalone candidate audit only"));
            assert!(
                updated
                    .report
                    .contains("Capability comparison uses accepted lock policy")
            );
            let source = fixture.read("root/build/package-manager/source-diff.txt");
            assert!(source.contains(&format!("candidate_git_commit {new}\n")));
            assert!(!source.contains("baseline_git_commit"));
            assert_eq!(
                fixture.lock().targets()[0].baselines(),
                accepted.targets()[0].baselines()
            );
        },
    );
}

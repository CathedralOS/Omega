use super::fixture::*;
use package_manager::operations::{
    PackageCommandStatus, PackageInspectionOptions, inspect_packages_with_storage,
};
use package_source::SourceResolverStorage;

#[test]
fn inspection_keeps_accepted_git_pin_when_head_moves_and_cache_is_missing() {
    run(
        "inspection_keeps_accepted_git_pin_when_head_moves_and_cache_is_missing",
        |fixture| {
            fixture.package("repository", "git-library", "");
            let old = fixture.commit();
            assert_eq!(
                fixture.install(None, None).unwrap().status,
                PackageCommandStatus::Published
            );
            fixture.git(&["tag", "accepted-source", &old]);
            fixture.write(
                "repository/main.omg",
                "pub machine value() -> u64 { 999 }\n",
            );
            let new = fixture.commit();
            let before = fixture.read("root/omega.lock");
            for cold in [false, true] {
                if cold {
                    std::fs::rename(fixture.path("cache"), fixture.path("previous-cache")).unwrap();
                }
                let inspected = inspect(fixture);
                assert!(inspected.complete, "{}", inspected.report);
                assert!(!inspected.requires_decision, "{}", inspected.report);
                assert!(inspected.report.contains(&old), "{}", inspected.report);
                assert!(!inspected.report.contains(&new), "{}", inspected.report);
                assert_eq!(fixture.read("root/omega.lock"), before);
            }
        },
    );
}

#[test]
fn inspection_keeps_named_and_relative_members_at_one_accepted_repository_pin() {
    run(
        "inspection_keeps_named_and_relative_members_at_one_accepted_repository_pin",
        |fixture| {
            fixture.package(
                "repository/modules/selected",
                "exact-math",
                " builder.depend(Source::Path { location: \"../other\" });\n",
            );
            let old = fixture.commit();
            assert_eq!(
                fixture.install(Some("exact-math"), None).unwrap().status,
                PackageCommandStatus::Published
            );
            fixture.git(&["tag", "accepted-source", &old]);
            fixture.write(
                "repository/modules/other/main.omg",
                "pub machine value() -> u64 { 999 }\n",
            );
            let new = fixture.commit();
            let before = fixture.read("root/omega.lock");
            std::fs::rename(fixture.path("cache"), fixture.path("previous-cache")).unwrap();
            let inspected = inspect(fixture);
            assert!(inspected.complete, "{}", inspected.report);
            assert!(!inspected.requires_decision, "{}", inspected.report);
            assert!(inspected.report.contains("exact-math"));
            assert!(inspected.report.contains("other-library"));
            assert!(inspected.report.matches(&old).count() >= 2);
            assert!(!inspected.report.contains(&new));
            assert_eq!(fixture.read("root/omega.lock"), before);
        },
    );
}

fn inspect(fixture: &Fixture) -> package_manager::operations::PackageInspectionOutcome {
    inspect_packages_with_storage(
        PackageInspectionOptions {
            project_root: fixture.path("root"),
            targets: Vec::new(),
            details: false,
        },
        &SourceResolverStorage::for_hardened_base(fixture.path("cache")).unwrap(),
    )
    .unwrap()
}

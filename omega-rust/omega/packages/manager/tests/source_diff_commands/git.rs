use super::fixture::*;
use package_manager::declarations::DependencyPurpose;
use package_manager::{PackageCommand, PackageCommandStatus};
use package_source::ImmutableSourceResolution;

include!("offline.rs");

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
                fixture.lock().targets()[0].occurrences(),
                accepted.targets()[0].occurrences()
            );
        },
    );
}

/// `(purpose, alias, selected commit)` for the root's accepted lock edges.
fn root_git_edges(fixture: &Fixture) -> Vec<(DependencyPurpose, String, String)> {
    let lock = fixture.lock();
    let subject = lock.targets()[0].source();
    let mut edges = subject
        .dependency_requests()
        .iter()
        .filter(|edge| edge.requester() == subject.root().selected().key())
        .map(|edge| {
            let ImmutableSourceResolution::Git { commit, .. } = edge.selected().resolution() else {
                panic!("fixture dependency selections are Git resolutions");
            };
            (
                edge.purpose(),
                edge.alias().as_str().to_owned(),
                commit.to_hex(),
            )
        })
        .collect::<Vec<_>>();
    edges.sort();
    edges
}

fn git_source(member: &str, revision: &str) -> String {
    format!(
        "Source::Git {{ repository: \"{REPOSITORY}\", revision: \"{revision}\", selection: PackageSelection::Named {{ package: \"{member}\" }} }}"
    )
}

#[test]
fn update_selects_a_build_scope_alias_and_retargets_its_git_row() {
    run(
        "update_selects_a_build_scope_alias_and_retargets_its_git_row",
        |fixture| {
            let original = fixture.commit();
            // The root authors only a build-purpose Git edge: a selector that
            // update must refresh and a row `--to` must rewrite in the build
            // scope rather than reporting a missing root Git dependency.
            fixture.write(
                "root/build.omg",
                &format!(
                    "machine build(builder: &mut Build) {{\n builder.package(\"consumer\");\n builder.build_depend_as(\"host\", {});\n}}\n",
                    git_source("exact-math", "HEAD"),
                ),
            );
            assert_eq!(
                fixture
                    .execute(PackageCommand::Update {
                        packages: Vec::new(),
                        revision: None,
                    })
                    .unwrap()
                    .status,
                PackageCommandStatus::Published
            );
            assert_eq!(
                root_git_edges(fixture),
                [(
                    DependencyPurpose::Build,
                    "host".to_owned(),
                    original.clone()
                )]
            );

            fixture.write(
                "repository/modules/selected/main.omg",
                "pub machine value() -> u64 { 23 }\n",
            );
            let moved = fixture.commit();
            assert_ne!(moved, original);

            // The build alias selects its package for refresh; nothing else
            // in the graph is touched.
            let updated = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["host".into()],
                    revision: None,
                })
                .unwrap();
            assert_eq!(
                updated.status,
                PackageCommandStatus::Published,
                "{}",
                updated.report
            );
            assert_eq!(
                root_git_edges(fixture),
                [(DependencyPurpose::Build, "host".to_owned(), moved.clone())]
            );

            // `--to` rewrites the build row's requested revision in place and
            // republishes it under the same purpose, never as a product row.
            // The fixture server permits fetching advertised exact objects,
            // so the retargeted commit needs a ref naming it.
            fixture.git(&["tag", "retargeted-source", &original]);
            let retargeted = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["host".into()],
                    revision: Some(original.clone()),
                })
                .unwrap();
            assert_eq!(
                retargeted.status,
                PackageCommandStatus::Published,
                "{}",
                retargeted.report
            );
            let build = fixture.read("root/build.omg");
            assert!(
                build.contains("builder.build_depend_as(\"host\""),
                "{build}"
            );
            assert!(!build.contains("builder.depend"), "{build}");
            assert!(
                build.contains(&format!("revision: \"{original}\"")),
                "{build}"
            );
            assert_eq!(
                root_git_edges(fixture),
                [(DependencyPurpose::Build, "host".to_owned(), original)]
            );
        },
    );
}

#[test]
fn update_to_retargets_both_scope_rows_of_a_dual_purpose_package() {
    run(
        "update_to_retargets_both_scope_rows_of_a_dual_purpose_package",
        |fixture| {
            let original = fixture.commit();
            // One package serves both purposes under two independently
            // authorized rows. The single source pin cannot split, so `--to`
            // rewrites each scope's row rather than leaving a divergent
            // build request that could never resolve one custody.
            let request = git_source("exact-math", &original);
            fixture.write(
                "root/build.omg",
                &format!(
                    "machine build(builder: &mut Build) {{\n builder.package(\"consumer\");\n builder.depend_as(\"tool\", {request});\n builder.build_depend_as(\"tool\", {request});\n}}\n"
                ),
            );
            assert_eq!(
                fixture
                    .execute(PackageCommand::Update {
                        packages: Vec::new(),
                        revision: None,
                    })
                    .unwrap()
                    .status,
                PackageCommandStatus::Published
            );

            fixture.write(
                "repository/modules/selected/main.omg",
                "pub machine value() -> u64 { 29 }\n",
            );
            let moved = fixture.commit();
            let updated = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["tool".into()],
                    revision: Some(moved.clone()),
                })
                .unwrap();
            assert_eq!(
                updated.status,
                PackageCommandStatus::Published,
                "{}",
                updated.report
            );
            let build = fixture.read("root/build.omg");
            for operation in ["depend_as", "build_depend_as"] {
                assert!(
                    build.contains(&format!(
                        "builder.{operation}(\"tool\", Source::Git {{ repository: \"{REPOSITORY}\", revision: \"{moved}\""
                    )),
                    "{build}"
                );
            }
            assert_eq!(
                root_git_edges(fixture),
                [
                    (DependencyPurpose::Product, "tool".to_owned(), moved.clone()),
                    (DependencyPurpose::Build, "tool".to_owned(), moved),
                ]
            );
        },
    );
}

#[test]
fn update_shared_cross_scope_alias_refreshes_both_selections() {
    run(
        "update_shared_cross_scope_alias_refreshes_both_selections",
        |fixture| {
            fixture.commit();
            // One spelling authorizes a different package per scope: refresh
            // covers both selections, while `--to` still requires one.
            fixture.write(
                "root/build.omg",
                &format!(
                    "machine build(builder: &mut Build) {{\n builder.package(\"consumer\");\n builder.depend_as(\"shared\", {});\n builder.build_depend_as(\"shared\", {});\n}}\n",
                    git_source("exact-math", "HEAD"),
                    git_source("other-library", "HEAD"),
                ),
            );
            assert_eq!(
                fixture
                    .execute(PackageCommand::Update {
                        packages: Vec::new(),
                        revision: None,
                    })
                    .unwrap()
                    .status,
                PackageCommandStatus::Published
            );

            fixture.write(
                "repository/modules/selected/main.omg",
                "pub machine value() -> u64 { 31 }\n",
            );
            let moved = fixture.commit();

            // `--to` cannot retarget one authored revision when the spelling
            // selects a different package per scope.
            let error = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["shared".into()],
                    revision: Some(moved.clone()),
                })
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("--to requires exactly one package or root dependency alias"),
                "{error}"
            );

            let updated = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["shared".into()],
                    revision: None,
                })
                .unwrap();
            assert_eq!(
                updated.status,
                PackageCommandStatus::Published,
                "{}",
                updated.report
            );
            assert_eq!(
                root_git_edges(fixture),
                [
                    (
                        DependencyPurpose::Product,
                        "shared".to_owned(),
                        moved.clone()
                    ),
                    (DependencyPurpose::Build, "shared".to_owned(), moved),
                ]
            );
        },
    );
}

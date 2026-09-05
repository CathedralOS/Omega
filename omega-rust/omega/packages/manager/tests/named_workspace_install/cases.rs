use super::fixture::*;
use package_manager::declarations::{PackageName, PackageSelection};
use package_manager::operations::{PackageCommand, PackageCommandStatus};
use package_manager::resolution::graph::CanonicalDependencySourceRequest;
use package_source::ImmutableSourceResolution;
use std::fs;

#[test]
fn named_member_uses_declared_default_alias_and_member_relative_dependencies() {
    run(
        "named_member_uses_declared_default_alias_and_member_relative_dependencies",
        |fixture| {
            fixture.package(
                "repository/modules/selected",
                "exact-math",
                " builder.depend(Source::Path { location: \"../other\" });\n",
            );
            fixture.write(
                "repository/modules/selected/main.omg",
                "use other_library::main;\npub machine calculate() -> u64 { value() }\n",
            );
            fixture.write(
                "root/main.omg",
                "use exact_math::main;\npub machine result() -> u64 { calculate() }\n",
            );
            fixture.commit();
            assert_eq!(
                fixture.install(Some("exact-math"), None).unwrap().status,
                PackageCommandStatus::Published
            );
            let lock = fixture.lock();
            let source = lock.targets()[0].source();
            assert_eq!(source.packages().len(), 3);
            let edge = source
                .dependency_requests()
                .iter()
                .find(|edge| edge.requester() == source.root().selected().key())
                .unwrap();
            assert_eq!(edge.alias().as_str(), "exact_math");
            assert_named(edge.request(), "exact-math");
            let build = fixture.read("root/build.omg");
            assert!(!build.contains("depend_as"));
            assert!(build.contains("PackageSelection::Named { package: \"exact-math\" }"));
            assert!(!build.contains("modules/selected"));
        },
    );
}

#[test]
fn named_member_alias_override_does_not_rename_the_selected_package() {
    run(
        "named_member_alias_override_does_not_rename_the_selected_package",
        |fixture| {
            fixture.write(
                "root/main.omg",
                "use math::main;\npub machine result() -> u64 { value() }\n",
            );
            fixture.commit();
            assert_eq!(
                fixture
                    .install(Some("exact-math"), Some("math"))
                    .unwrap()
                    .status,
                PackageCommandStatus::Published
            );
            let lock = fixture.lock();
            let source = lock.targets()[0].source();
            assert_eq!(
                source.packages().len(),
                2,
                "unreferenced workspace members stay out of the graph"
            );
            let edge = &source.dependency_requests()[0];
            assert_eq!(edge.alias().as_str(), "math");
            assert_eq!(edge.selected().key().name().as_str(), "exact-math");
            assert_named(edge.request(), "exact-math");
        },
    );
}

#[test]
fn unknown_declared_name_rejects_without_publishing() {
    run(
        "unknown_declared_name_rejects_without_publishing",
        |fixture| {
            fixture.commit();
            let before = fixture.pair();
            let error = fixture
                .install(Some("unknown-library"), None)
                .unwrap_err()
                .to_string();
            assert!(error.contains("no member package named"), "{error}");
            assert_eq!(fixture.pair(), before);
            assert!(!fixture.path("root/build/package-manager/proposal").exists());
        },
    );
}

#[test]
fn duplicate_declared_names_reject_without_publishing() {
    run(
        "duplicate_declared_names_reject_without_publishing",
        |fixture| {
            fixture.package("repository/modules/other", "exact-math", "");
            fixture.commit();
            let before = fixture.pair();
            let error = fixture
                .install(Some("exact-math"), None)
                .unwrap_err()
                .to_string();
            assert!(error.contains("multiple member paths"), "{error}");
            assert_eq!(fixture.pair(), before);
            assert!(!fixture.path("root/build/package-manager/proposal").exists());
        },
    );
}

#[test]
fn omitted_selection_keeps_root_package_behavior() {
    run("omitted_selection_keeps_root_package_behavior", |fixture| {
        fixture.package("repository", "root-library", "");
        fixture.commit();
        assert_eq!(
            fixture.install(None, None).unwrap().status,
            PackageCommandStatus::Published
        );
        let lock = fixture.lock();
        let edge = &lock.targets()[0].source().dependency_requests()[0];
        assert_eq!(edge.alias().as_str(), "root_library");
        assert!(matches!(
            edge.request(),
            CanonicalDependencySourceRequest::Git {
                selection: PackageSelection::Root,
                ..
            }
        ));
        assert!(!fixture.read("root/build.omg").contains("selection:"));
    });
}

#[test]
fn omitted_selection_does_not_guess_a_workspace_member() {
    run(
        "omitted_selection_does_not_guess_a_workspace_member",
        |fixture| {
            fixture.commit();
            let before = fixture.pair();
            assert!(fixture.install(None, None).is_err());
            assert_eq!(fixture.pair(), before);
        },
    );
}

#[test]
fn named_review_resume_retains_selection_alias_and_exact_revision() {
    run(
        "named_review_resume_retains_selection_alias_and_exact_revision",
        |fixture| {
            fixture.write(
                "repository/modules/selected/main.omg",
                &format!("{PURE}{ASSUMPTION}"),
            );
            let original_revision = fixture.commit();
            let before = fixture.pair();
            let pending = fixture.install(Some("exact-math"), Some("math")).unwrap();
            assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
            assert_eq!(fixture.pair(), before);
            assert!(
                fixture
                    .read("root/build/package-manager/proposal")
                    .contains("PackageSelection::Named")
            );
            assert_eq!(
                fixture.resume().unwrap().status,
                PackageCommandStatus::ReviewRequired
            );
            // A moving remote selector cannot change the candidate during resume.
            fixture.write(
                "repository/modules/selected/main.omg",
                "pub machine value() -> u64 { 99 }\n",
            );
            assert_ne!(fixture.commit(), original_revision);
            for path in &pending.review_paths {
                let before = fs::read_to_string(path).unwrap();
                let mut count = 0;
                let after: String = before
                    .split_inclusive('\n')
                    .map(|line| {
                        if line.starts_with("decision ") {
                            count += 1;
                            format!("{} accept\n", line.strip_suffix(" pending\n").unwrap())
                        } else {
                            line.to_owned()
                        }
                    })
                    .collect();
                assert!(count > 0);
                fs::write(path, after).unwrap();
            }
            assert_eq!(
                fixture.resume().unwrap().status,
                PackageCommandStatus::Published
            );
            let lock = fixture.lock();
            let edge = &lock.targets()[0].source().dependency_requests()[0];
            assert_eq!(edge.alias().as_str(), "math");
            assert_named(edge.request(), "exact-math");
            let ImmutableSourceResolution::Git { commit, .. } = edge.selected().resolution() else {
                panic!("Git pin");
            };
            assert_eq!(commit.to_hex(), original_revision);
            assert!(!fixture.path("root/build/package-manager/proposal").exists());
        },
    );
}

#[test]
fn named_selection_rejects_local_sources_and_member_paths() {
    run(
        "named_selection_rejects_local_sources_and_member_paths",
        |fixture| {
            let before = fixture.pair();
            let error = fixture
                .execute(PackageCommand::Install {
                    source: "../repository".into(),
                    revision: None,
                    alias: None,
                    package: Some("exact-math".into()),
                })
                .unwrap_err()
                .to_string();
            assert!(error.contains("only valid for a Git source"), "{error}");
            assert!(fixture.install(Some("modules/selected"), None).is_err());
            assert_eq!(fixture.pair(), before);
        },
    );
}

#[test]
fn selected_member_update_moves_reachable_members_but_preserves_unrelated_pins() {
    run(
        "selected_member_update_moves_reachable_members_but_preserves_unrelated_pins",
        |fixture| {
            fixture.package(
                "repository/modules/selected",
                "exact-math",
                " builder.depend(Source::Path { location: \"../other\" });\n",
            );
            let original = fixture.commit();
            assert_eq!(
                fixture.install(Some("exact-math"), None).unwrap().status,
                PackageCommandStatus::Published
            );
            // The test server serves the same bytes at a distinct generic
            // repository namespace. No adapter establishes lineage equivalence.
            assert_eq!(
                fixture
                    .execute(PackageCommand::Install {
                        source: "git@unrelated-fixture.invalid:workspace.git".into(),
                        revision: None,
                        alias: Some("unrelated".into()),
                        package: Some("other-library".into()),
                    })
                    .unwrap()
                    .status,
                PackageCommandStatus::Published
            );
            fixture.write(
                "repository/modules/selected/main.omg",
                &format!("// new source\n{PURE}"),
            );
            fixture.write(
                "repository/modules/other/main.omg",
                &format!("// changed sibling\n{PURE}"),
            );
            let updated = fixture.commit();
            assert_ne!(updated, original);
            let outcome = fixture
                .execute(PackageCommand::Update {
                    packages: vec!["exact_math".into()],
                    revision: None,
                })
                .unwrap();
            assert_eq!(outcome.status, PackageCommandStatus::Published);
            let lock = fixture.lock();
            let source = lock.targets()[0].source();
            assert_eq!(source.packages().len(), 4);
            let selected = source
                .dependency_requests()
                .iter()
                .find(|edge| {
                    edge.requester() == source.root().selected().key()
                        && edge.alias().as_str() == "exact_math"
                })
                .unwrap();
            let mut affected = 0;
            for package in source.packages() {
                let ImmutableSourceResolution::Git { commit, .. } = package.resolution() else {
                    continue;
                };
                if package.key().source_lineage() == selected.selected().key().source_lineage() {
                    affected += 1;
                    assert_eq!(commit.to_hex(), updated);
                } else {
                    assert_eq!(commit.to_hex(), original);
                }
            }
            assert_eq!(affected, 2);
            let report = fs::read_to_string(&outcome.review_paths[0]).unwrap();
            assert!(report.contains("exact-math"));
            assert!(report.contains("other-library"));
        },
    );
}

fn assert_named(request: &CanonicalDependencySourceRequest, expected: &str) {
    let CanonicalDependencySourceRequest::Git { selection, .. } = request else {
        panic!("Git request");
    };
    assert_eq!(
        selection,
        &PackageSelection::Named(PackageName::parse(expected).unwrap())
    );
}

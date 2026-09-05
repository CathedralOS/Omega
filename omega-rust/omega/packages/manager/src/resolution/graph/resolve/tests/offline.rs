//! Invocation-wide offline policy through real graph traversal and counted SSH transport.

use super::*;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ResolvedPackageSourceClosure,
};
use package_source::git::resolution::GitExactRevisionAcquisition;

mod support;
use support::*;

#[test]
fn preserved_cached_git_pins_resolve_live_and_staged_without_transport() {
    run(
        "preserved_cached_git_pins_resolve_live_and_staged_without_transport",
        |fixture| {
            fixture.repository();
            let dependency = dependency("HEAD");
            fixture.root(&dependency);
            let storage = fixture.storage("cache");
            let before = fixture
                .resolve(&storage, None, GitResolutionOptions::default())
                .unwrap();
            let accepted = subject(&before);
            let calls = fixture.transport_calls();
            assert!(
                calls > 0,
                "online baseline must exercise the counted transport"
            );
            std::fs::write(
                fixture.path("repository/advanced.omg"),
                "machine advanced() {}\n",
            )
            .unwrap();
            fixture.commit();
            let pins =
                GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::AllowFetch)
                    .unwrap();
            for proposed in [None, Some(dependency.as_str())] {
                let candidate = fixture
                    .resolve(&storage, proposed, offline(Some(pins)))
                    .unwrap();
                assert_eq!(subject(&candidate), accepted);
                assert_eq!(fixture.transport_calls(), calls);
            }
        },
    );
}

#[test]
fn offline_overrides_preserved_allow_fetch_on_missing_cache() {
    run(
        "offline_overrides_preserved_allow_fetch_on_missing_cache",
        |fixture| {
            fixture.repository();
            let dependency = dependency("HEAD");
            fixture.root(&dependency);
            let warm = fixture.storage("warm");
            let accepted = subject(
                &fixture
                    .resolve(&warm, None, GitResolutionOptions::default())
                    .unwrap(),
            );
            let pins =
                GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::AllowFetch)
                    .unwrap();
            let calls = fixture.transport_calls();
            let cold = fixture.storage("cold");
            for proposed in [None, Some(dependency.as_str())] {
                let error = fixture
                    .resolve(&cold, proposed, offline(Some(pins)))
                    .unwrap_err();
                assert!(matches!(
                    error,
                    ResolveExternalLocalPackageClosureError::Closure(
                        PackageSourceClosureResolutionError::Adapter {
                            error: ResolveDependencySourceError::Source(
                                crate::resolution::source::ResolvePackageSourceError::Source(
                                    package_source::SourceResolveError::GitExactRevisionUnavailable { .. }
                                )
                            ),
                            ..
                        }
                    )
                ));
                assert_eq!(fixture.transport_calls(), calls);
            }
        },
    );
}

#[test]
fn offline_rejects_selected_refresh_and_unpinned_warm_cache() {
    run(
        "offline_rejects_selected_refresh_and_unpinned_warm_cache",
        |fixture| {
            fixture.repository();
            let dependency = dependency("HEAD");
            fixture.root(&dependency);
            let storage = fixture.storage("cache");
            let before = fixture
                .resolve(&storage, None, GitResolutionOptions::default())
                .unwrap();
            let accepted = subject(&before);
            let selected = [before
                .custodies()
                .iter()
                .find(|source| source.key().name().as_str() == "dependency")
                .unwrap()
                .key()
                .clone()];
            let pins =
                GitDependencyPins::new(&accepted, &selected, GitExactRevisionAcquisition::Offline)
                    .unwrap();
            let calls = fixture.transport_calls();
            for policy in [offline(Some(pins)), offline(None)] {
                for proposed in [None, Some(dependency.as_str())] {
                    assert_selection_rejected(
                        fixture.resolve(&storage, proposed, policy).unwrap_err(),
                    );
                    assert_eq!(fixture.transport_calls(), calls);
                }
            }
        },
    );
}

#[test]
fn offline_rejects_new_and_changed_git_requests_without_transport() {
    run(
        "offline_rejects_new_and_changed_git_requests_without_transport",
        |fixture| {
            fixture.repository();
            fixture.root("");
            let storage = fixture.storage("cache");
            let accepted = subject(&fixture.resolve(&storage, None, offline(None)).unwrap());
            let pins =
                GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::AllowFetch)
                    .unwrap();
            let original = dependency("HEAD");
            assert_selection_rejected(
                fixture
                    .resolve(&storage, Some(&original), offline(Some(pins)))
                    .unwrap_err(),
            );
            assert_eq!(fixture.transport_calls(), 0);
            fixture.root(&original);
            assert_selection_rejected(
                fixture
                    .resolve(&storage, None, offline(Some(pins)))
                    .unwrap_err(),
            );
            assert_eq!(fixture.transport_calls(), 0);

            let accepted = subject(
                &fixture
                    .resolve(&storage, None, GitResolutionOptions::default())
                    .unwrap(),
            );
            let pins =
                GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::AllowFetch)
                    .unwrap();
            let calls = fixture.transport_calls();
            let changed = dependency("main");
            assert_selection_rejected(
                fixture
                    .resolve(&storage, Some(&changed), offline(Some(pins)))
                    .unwrap_err(),
            );
            fixture.root(&changed);
            assert_selection_rejected(
                fixture
                    .resolve(&storage, None, offline(Some(pins)))
                    .unwrap_err(),
            );
            assert_eq!(fixture.transport_calls(), calls);
        },
    );
}

#[test]
fn offline_checks_unseen_transitive_git_below_local_paths() {
    run(
        "offline_checks_unseen_transitive_git_below_local_paths",
        |fixture| {
            fixture.repository();
            let local = "builder.depend(Source::Path { location: \"../middle\" });";
            fixture.root(local);
            write_package(&fixture.path("middle"), "middle", Some("../leaf"));
            write_package(&fixture.path("leaf"), "leaf", None);
            let storage = fixture.storage("cache");
            let accepted = subject(&fixture.resolve(&storage, None, offline(None)).unwrap());
            let pins =
                GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::AllowFetch)
                    .unwrap();
            std::fs::write(
                fixture.path("leaf/build.omg"),
                format!(
                    "machine build(builder: &mut Build) {{ builder.package(\"leaf\"); {} }}\n",
                    dependency("HEAD"),
                ),
            )
            .unwrap();
            for proposed in [None, Some(local)] {
                let error = fixture
                    .resolve(&storage, proposed, offline(Some(pins)))
                    .unwrap_err();
                assert!(
                    error.to_string().contains("leaf"),
                    "transitive requester must be reported: {error}"
                );
                assert_selection_rejected(error);
                assert_eq!(fixture.transport_calls(), 0);
            }
        },
    );
}

#[test]
fn offline_local_only_closures_use_no_transport() {
    run("offline_local_only_closures_use_no_transport", |fixture| {
        let local = "builder.depend(Source::Path { location: \"../middle\" });";
        fixture.root(local);
        write_package(&fixture.path("middle"), "middle", Some("../leaf"));
        write_package(&fixture.path("leaf"), "leaf", None);
        let storage = fixture.storage("cache");
        for proposed in [None, Some(local)] {
            let closure = fixture.resolve(&storage, proposed, offline(None)).unwrap();
            assert_eq!(closure.custodies().len(), 3);
            assert_eq!(fixture.transport_calls(), 0);
        }
    });
}

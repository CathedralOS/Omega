//! A matching alias graph cannot authorize a different acquired request.

use super::*;
use build_declarations::{BuildDependencySource, BuildRequest, DependencyPurpose, ProjectName};
use package_compilation::{BuildDeclarationKind, PackageDependencyBinding, PackageSourceBinding};
use platform_custody::temporary_directory::TemporaryDirectory;
use semantic_vocabulary::PackageKeyIdentity;

fn path_request(path: &str, purpose: DependencyPurpose) -> BuildRequest {
    BuildRequest::Dependency {
        purpose,
        alias: Some(ProjectName::parse("peer").unwrap()),
        source: BuildDependencySource::Path(path.into()),
    }
}

fn git_request(revision: &str) -> BuildRequest {
    BuildRequest::Dependency {
        purpose: DependencyPurpose::Product,
        alias: Some(ProjectName::parse("peer").unwrap()),
        source: BuildDependencySource::Git {
            repository: "https://example.invalid/peer.git".into(),
            revision: revision.into(),
            package: None,
        },
    }
}

fn inputs(directory: &std::path::Path) -> PackageCompilationInputs {
    let root = PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let peer = PackageKeyIdentity::from_digest([2; 32]).unwrap();
    for name in ["root", "peer"] {
        std::fs::create_dir_all(directory.join(name)).unwrap();
    }
    PackageCompilationInputs::new(
        root,
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(root, "source_join_root", directory.join("root")),
            PackageSourceBinding::new(peer, "source_join_peer", directory.join("peer")),
        ],
        vec![PackageDependencyBinding::new(root, "peer", peer)],
    )
    .unwrap()
}

fn acquired(inputs: &PackageCompilationInputs, request: &BuildRequest) -> AcquiredBuildSourceJoin {
    AcquiredBuildSourceJoin {
        source: inputs.source_inputs(),
        requests: DependencyProjections::from_executed_requests([request]).unwrap(),
        selections: vec![BuildDependencySelection {
            purpose: DependencyPurpose::Product,
            request_ordinal: 0,
            package: PackageKeyIdentity::from_digest([2; 32]).unwrap(),
        }],
    }
}

#[test]
fn same_alias_and_target_do_not_substitute_source_request_or_purpose() {
    let directory = TemporaryDirectory::create("build-source-join", false).unwrap();
    let inputs = inputs(directory.path());
    let expected = path_request("../acquired-peer", DependencyPurpose::Product);
    let join = acquired(&inputs, &expected);
    assert_eq!(
        join.resolve_issued_requests(&inputs, &[&expected]).unwrap(),
        join.selections
    );
    for substitute in [
        path_request("../different-peer", DependencyPurpose::Product),
        path_request("../acquired-peer", DependencyPurpose::Build),
    ] {
        assert!(
            join.resolve_issued_requests(&inputs, &[&substitute])
                .unwrap_err()
                .contains("requests differ")
        );
    }
    assert!(
        join.resolve_issued_requests(&inputs, &[])
            .unwrap_err()
            .contains("requests differ")
    );
    assert!(
        join.resolve_issued_requests(&inputs, &[&expected, &expected])
            .unwrap_err()
            .contains("requests differ")
    );
    let pinned = git_request("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let changed = git_request("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert!(
        acquired(&inputs, &pinned)
            .resolve_issued_requests(&inputs, &[&changed])
            .unwrap_err()
            .contains("requests differ")
    );
}

#[test]
fn matching_requests_cannot_rebind_acquired_source_routing() {
    let first = TemporaryDirectory::create("build-source-join-first", false).unwrap();
    let second = TemporaryDirectory::create("build-source-join-second", false).unwrap();
    let original = inputs(first.path());
    let replacement = inputs(second.path());
    let request = path_request("../acquired-peer", DependencyPurpose::Product);
    assert!(
        acquired(&original, &request)
            .resolve_issued_requests(&replacement, &[&request])
            .unwrap_err()
            .contains("another acquired source graph")
    );
}

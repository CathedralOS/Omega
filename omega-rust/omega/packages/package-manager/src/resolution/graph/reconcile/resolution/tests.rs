//! Graph-state tests, not compiler acceptance. The small source capture supplies
//! materialization; declared names and dependency rows are explicit graph inputs.

use super::*;
use crate::declarations::PackageName;
use crate::declarations::dependencies::read::DependencyProjections;
use package_source::{ExternalSourceContext, LocalSourceLimits};

fn source(name: &str, product: &[&str], build: &[&str]) -> PackageSourceCustody {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/omega/pass/build/separate_build_product_imports_exit");
    let acquired = crate::resolution::source::AcquiredLocalSource::capture(
        &root,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"graph-state-test"),
    )
    .unwrap();
    let captured = acquired.source_capture();
    let lineage = package_source::SourceLineage::ExternalLocal(
        package_source::ExternalLocalLineage::from_canonical_path(
            captured.canonical_live_root().to_str().unwrap().to_owned(),
            ExternalSourceContext::derive(b"graph-state-test"),
        )
        .unwrap(),
    );
    let requests = |names: &[&str]| {
        names
            .iter()
            .map(|name| DependencySourceRequest::Path {
                explicit_alias: None,
                location: (*name).to_owned(),
            })
            .collect::<Vec<_>>()
            .into()
    };
    PackageSourceCustody::from_resolved_parts(
        PackageKey::new(PackageName::parse(name).unwrap(), lineage),
        BuildDeclarationKind::Package,
        package_source::ImmutableSourceResolution::external_local(
            package_source::SourceContentDigest::derive(
                captured.normalized().content_identity.as_bytes(),
            ),
        ),
        crate::resolution::source::PackageSourceMaterialization::from_local(captured.normalized()),
        captured.canonical_live_root().to_path_buf(),
        crate::resolution::source::PackageSourceNavigation::Root,
        crate::resolution::source::PackageSourceSelectionEvidence::Root,
        LocalSourceLimits::default(),
        DependencyProjections::new(requests(product), requests(build)),
    )
}

fn begin(
    root: PackageSourceCustody,
    limits: PackageSourceClosureLimits,
) -> Result<SourceClosureStep, PackageSourceClosureResolutionError<String>> {
    begin_source_closure(
        PackageRootSourceRequest::ExternalLocal {
            requested_root: root.snapshot_root().to_path_buf(),
            source_context: ExternalSourceContext::derive(b"graph-state-test"),
        },
        root,
        limits,
    )
}

fn request(step: SourceClosureStep) -> PendingSourceResolution {
    match step {
        SourceClosureStep::Request(request) => request,
        SourceClosureStep::Complete(_) => panic!("outstanding request was skipped"),
    }
}

#[test]
fn suspended_requests_retain_scope_order_and_shared_custody() {
    let first =
        request(begin(source("root", &["shared"], &["shared"]), Default::default()).unwrap());
    assert_eq!(first.requester().key().name().as_str(), "root");
    assert_eq!(first.purpose, DependencyPurpose::Product);
    assert_eq!(first.dependency_index, 0);
    // The selected declaration can be produced while the request independently
    // owns the traversal; nothing re-enters a resolver callback to produce it.
    let shared = source("shared", &["leaf"], &[]);
    let second = request(first.complete::<String>(shared.clone()).unwrap());
    assert_eq!(second.purpose, DependencyPurpose::Build);
    assert_eq!(second.dependency_index, 0);
    let third = request(second.complete::<String>(shared).unwrap());
    assert_eq!(third.requester().key().name().as_str(), "shared");
    let completed = third.complete::<String>(source("leaf", &[], &[])).unwrap();
    let SourceClosureStep::Complete(closure) = completed else {
        panic!("closure did not join");
    };
    assert_eq!(closure.graph().packages().len(), 3);
    assert_eq!(closure.source_requests().dependencies().count(), 3);
}

#[test]
fn request_limits_are_checked_before_issuing_work() {
    let limits = PackageSourceClosureLimits {
        max_packages: 0,
        ..Default::default()
    };
    assert!(matches!(
        begin(source("root", &[], &[]), limits),
        Err(PackageSourceClosureResolutionError::LimitExceeded {
            kind: PackageSourceClosureLimitKind::Packages,
            limit: 0,
        })
    ));
    let limits = PackageSourceClosureLimits {
        max_dependency_requests: 1,
        ..Default::default()
    };
    let first = request(begin(source("root", &["one", "two"], &[]), limits).unwrap());
    let error = first
        .complete::<String>(source("one", &[], &[]))
        .err()
        .expect("second request exceeds ceiling");
    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::LimitExceeded {
            kind: PackageSourceClosureLimitKind::DependencyRequests,
            limit: 1,
        }
    ));
    let limits = PackageSourceClosureLimits {
        max_depth: 0,
        ..Default::default()
    };
    assert!(matches!(
        begin(source("root", &["one"], &[]), limits),
        Err(PackageSourceClosureResolutionError::LimitExceeded {
            kind: PackageSourceClosureLimitKind::Depth,
            limit: 0,
        })
    ));
}

#[test]
fn adapter_failure_retains_the_exact_outstanding_occurrence() {
    let first = request(begin(source("root", &[], &["helper"]), Default::default()).unwrap());
    let error = first.fail("acquisition refused".to_owned());
    let PackageSourceClosureResolutionError::Adapter {
        requester,
        purpose,
        dependency_index,
        request,
        error,
    } = error
    else {
        panic!("adapter context was discarded");
    };
    assert_eq!(requester.name().as_str(), "root");
    assert_eq!(purpose, DependencyPurpose::Build);
    assert_eq!(dependency_index, 0);
    assert!(
        matches!(request, DependencySourceRequest::Path { location, .. } if location == "helper")
    );
    assert_eq!(error, "acquisition refused");
}

#[test]
fn resumed_sources_keep_role_alias_conflict_and_cycle_checks() {
    let first = request(begin(source("root", &["one"], &[]), Default::default()).unwrap());
    let mut application = source("one", &[], &[]);
    application = PackageSourceCustody::from_resolved_parts(
        application.key().clone(),
        BuildDeclarationKind::Application,
        application.resolution().clone(),
        application.materialization().clone(),
        application.snapshot_root().to_path_buf(),
        application.navigation().clone(),
        application.selection_evidence().clone(),
        application.source_limits(),
        application.dependency_projections().clone(),
    );
    assert!(matches!(
        first.complete::<String>(application),
        Err(PackageSourceClosureResolutionError::InvalidDependencyRole { .. })
    ));

    let first = request(begin(source("root", &["same", "same"], &[]), Default::default()).unwrap());
    let second = request(first.complete::<String>(source("same", &[], &[])).unwrap());
    assert!(matches!(
        second.complete::<String>(source("same", &[], &[])),
        Err(PackageSourceClosureResolutionError::InvalidAliases { .. })
    ));

    let root = source("root", &["child"], &[]);
    let first = request(begin(root.clone(), Default::default()).unwrap());
    let second = request(
        first
            .complete::<String>(source("child", &["root"], &[]))
            .unwrap(),
    );
    assert!(matches!(
        second.complete::<String>(root),
        Err(PackageSourceClosureResolutionError::InvalidClosure { .. })
    ));

    let first =
        request(begin(source("root", &["shared"], &["shared"]), Default::default()).unwrap());
    let second = request(
        first
            .complete::<String>(source("shared", &[], &[]))
            .unwrap(),
    );
    assert!(matches!(
        second.complete::<String>(source("shared", &["different"], &[])),
        Err(PackageSourceClosureResolutionError::ConflictingCustody { .. })
    ));
}

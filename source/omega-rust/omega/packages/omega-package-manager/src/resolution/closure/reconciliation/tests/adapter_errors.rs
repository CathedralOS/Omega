use super::super::*;
use super::support::*;
use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::PackageSourceCustody;

#[test]
fn returns_adapter_error_with_exact_request_context() {
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("missing")],
    );

    let error = resolve_package_source_closure(git_root_request(&root), root, |_, _| {
        Err::<PackageSourceCustody, _>("network unavailable")
    })
    .expect_err("adapter failure returns");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::Adapter {
            requester,
            dependency_index: 0,
            request: DependencySourceRequest::Path { location, .. },
            error: "network unavailable",
        } if requester.name().as_str() == "application" && location == "missing"
    ));
}

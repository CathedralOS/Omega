//! Review publishes source-free evidence under the caller's exact byte budget.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CheckedCompileRequest, CompilationSession, PackageReviewFailure};
use package_evidence::PackageReviewProjectionError;

#[test]
fn review_product_keeps_exact_custody_and_policy_budget() {
    let target = "linux_x86_64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/calls/machine_self_call_recursion_compile/main.omg");
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("fixture has admitted package inputs");
    let inputs = prepared.package_inputs.expect("package-aware fixture");
    let package = inputs.root();
    let mut session = CompilationSession::default();
    let mut review = |maximum_bytes| {
        let build = session
            .prepare_package_build(CheckedCompileRequest {
                package_inputs: Some(inputs.clone()),
                ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
            })
            .expect("prepare package Build");
        session.review_package_build(build, maximum_bytes)
    };
    let first = review(usize::MAX).expect("review product");
    let bytes = first.evidence.policy_canonical_bytes;
    assert_eq!(first.evidence.policy.package(), package);
    assert_eq!(first.evidence.generated_source_bundle.package(), package);
    assert_eq!(
        first.evidence.source_consumption_commitment,
        first
            .evidence
            .generated_source_bundle
            .source_consumption_commitment()
    );
    assert_eq!(
        first
            .evidence
            .policy
            .canonical_bytes()
            .expect("policy encoding")
            .len(),
        bytes
    );
    assert!(
        first
            .source
            .facts()
            .consumed_units
            .as_ref()
            .is_some_and(|units| !units.is_empty())
    );
    assert!(!first.source.facts().tolerated_unsettled_service_fields);
    assert!(bytes > 0);
    assert!(matches!(
        review(bytes - 1),
        Err(PackageReviewFailure::Projection(
            PackageReviewProjectionError::PolicyBudget
        ))
    ));
    let exact = review(bytes).expect("exact budget is sufficient");
    assert_eq!(exact.evidence.policy, first.evidence.policy);
    assert_eq!(
        exact.evidence.source_consumption_commitment,
        first.evidence.source_consumption_commitment
    );
}

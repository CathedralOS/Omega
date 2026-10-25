//! Review publishes source-free evidence under the caller's exact byte budget.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CheckedCompileRequest, CompilationSession, PackageReviewFailure};
use package_evidence::PackageReviewProjectionError;

#[test]
fn dependency_review_retains_constant_values_without_admitting_new_callers() {
    let target = "linux_x86_64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/reviewed_dependency_constants/main.omg");
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("constant dependency fixture has admitted package inputs");
    let inputs = prepared.package_inputs.clone().expect("root custody");
    let request = CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
    };
    let occurrences = fixture_package_inputs::fixture_occurrences(&prepared, request)
        .expect("dependency-first fixture occurrences");
    assert_eq!(occurrences.len(), 2, "one dependency and its consumer");

    // Ordinary fixture compilation prepares dependency Builds but checks only
    // the consumer Product. Review must check each dependency first, in this
    // same session, so the consumer extends its resolved/typed source cohort.
    let mut session = CompilationSession::default();
    let root_package = inputs.root();
    let mut dependency_bundles = Vec::new();
    for mut occurrence in occurrences {
        occurrence.package_inputs = Some(
            occurrence
                .package_inputs
                .take()
                .expect("occurrence custody")
                .with_complete_dependency_generated_sources(dependency_bundles.clone())
                .expect("complete dependency Build outputs, including empty bundles"),
        );
        let build = session
            .prepare_package_build(occurrence)
            .expect("prepare dependency or consumer Build");
        if build.package_inputs().root() != root_package {
            dependency_bundles.push(build.generated_source_bundle().clone());
        }
        session
            .review_package_build(build, usize::MAX)
            .expect("review retained constant dependency and consumer");
    }

    // This entry lives in the same sealed source inventory. Only the lower
    // bound changes: a retained dependency value is not proof of a new call's
    // precondition, even after the valid consumer has also been reviewed.
    let invalid = prepared.root_path.with_file_name("invalid_bounds.omg");
    let refused = session
        .prepare_package_build(CheckedCompileRequest {
            package_inputs: Some(
                inputs
                    .with_complete_dependency_generated_sources(dependency_bundles)
                    .expect("invalid caller uses the same dependency Build outputs"),
            ),
            ..CheckedCompileRequest::new(&invalid, Some(target))
        })
        .map_err(PackageReviewFailure::Compilation)
        .and_then(|build| session.review_package_build(build, usize::MAX));
    let diagnostics = match refused {
        Err(PackageReviewFailure::Compilation(failure)) => failure.into_diagnostics(),
        Err(failure) => panic!("bounds control reached an unrelated review failure: {failure:?}"),
        Ok(_) => panic!("a dependency judgment admitted inverted bounds in a new caller"),
    };
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("lower <= upper"), "{rendered}");
    assert!(rendered.contains("4 <= 3"), "{rendered}");
}

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

#[test]
fn review_policy_round_trip_preserves_authored_and_inferred_reach() {
    use package_evidence::encoding::PackagePolicyRecoveryLimits;
    use package_evidence::record::PackagePolicyBaseline;

    let root = fixture_package_inputs::repo_root();
    for (fixture, expected) in [
        (
            "tests/omega/pass/facts/unannotated_imported_boundary_call_through_local_exit/support/main.omg",
            "inferred",
        ),
        (
            "tests/omega/pass/effects/optional_reach_declared_slack_exit/main.omg",
            "slack",
        ),
        (
            "tests/omega/pass/effects/optional_reach_empty_exit/main.omg",
            "empty",
        ),
    ] {
        let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &root.join(fixture),
            Some("macos_arm64"),
        )
        .expect("reviewable reach fixture");
        let mut session = CompilationSession::default();
        let build = session
            .prepare_package_build(CheckedCompileRequest {
                package_inputs: prepared.package_inputs,
                ..CheckedCompileRequest::new(&prepared.root_path, Some("macos_arm64"))
            })
            .expect("prepare reach review");
        let reviewed = session
            .review_package_build(build, usize::MAX)
            .expect("reach review");
        let bytes = reviewed
            .evidence
            .policy
            .canonical_bytes()
            .expect("encode reach policy");
        let recovered = PackagePolicyBaseline::recover_canonical(
            &bytes,
            PackagePolicyRecoveryLimits::default(),
        )
        .expect("recover source-free reach policy");
        // Provider identity tables retain construction interning beyond the
        // rows they serialize. Compare persisted meaning, then inspect the
        // recovered reach fields themselves below.
        assert_eq!(
            recovered.canonical_bytes().expect("re-encode policy"),
            bytes
        );
        let callables = recovered.callables().callables().iter().collect::<Vec<_>>();
        assert!(!callables.is_empty(), "{fixture}");
        match expected {
            "inferred" => {
                let callable = callables
                    .iter()
                    .find(|callable| callable.identity().path().contains("fetch_mark"))
                    .expect("exported inferred helper");
                assert!(callable.declared_service_reach().is_none());
                let realized = callable
                    .checked_service_reach()
                    .realized()
                    .expect("checked body");
                assert_eq!(realized.len(), 1);
                assert_eq!(realized[0].path(), "Mark");
                assert_eq!(callable.published_service_reach(), Some(realized));
            }
            "slack" => {
                let callable = callables
                    .iter()
                    .find(|callable| callable.identity().path().contains("read_mark"))
                    .expect("exported bounded helper");
                let declared = callable
                    .declared_service_reach()
                    .expect("authored wider ceiling");
                let realized = callable
                    .checked_service_reach()
                    .realized()
                    .expect("checked body");
                assert_eq!(declared.len(), 2);
                assert_eq!(realized.len(), 1);
                assert_eq!(callable.published_service_reach(), Some(declared));
            }
            "empty" => {
                let explicit = callables
                    .iter()
                    .find(|callable| callable.identity().path().contains("read_mark"))
                    .expect("explicit empty helper");
                let inferred = callables
                    .iter()
                    .find(|callable| callable.identity().path().contains("inferred_empty"))
                    .expect("inferred empty helper");
                assert_eq!(explicit.declared_service_reach(), Some([].as_slice()));
                assert!(inferred.declared_service_reach().is_none());
                assert_eq!(explicit.published_service_reach(), Some([].as_slice()));
                assert_eq!(inferred.published_service_reach(), Some([].as_slice()));
            }
            _ => unreachable!(),
        }
    }
}

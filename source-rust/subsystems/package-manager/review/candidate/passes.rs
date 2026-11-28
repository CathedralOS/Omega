//! Resolve the review's dependency roster and discover its consumer-scoped bindings.
//! A bound pass recovers evidence against the final inputs and shares the invocation's
//! accounting with the discovery pass.

use crate::resolution::graph::CanonicalSourceClosureSubject;
use package_compilation::build_execution::BuildSnapshotRequest;
#[path = "passes_occurrences.rs"]
mod occurrences;

use super::semantic_bindings::{candidate_semantic_binding_inputs, semantic_bindings_by_consumer};
use super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet,
    ConsumerScopedSemanticBindingReviewInput,
};
use crate::build_session::PackageBuildSession;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::timings;
use occurrences::{PackageReviewPass, RetainedPackageReviews};

pub(crate) use crate::build_session::PackageSourceBindings;

#[derive(Clone, Copy)]
pub(crate) struct CandidateReview<'request, 'closure> {
    pub target_closure: &'request ExactTargetPackageSourceClosure<'closure>,
    pub root_build_snapshot: Option<&'request BuildSnapshotRequest>,
}

pub(crate) fn review_resolved_packages(
    candidate: CandidateReview<'_, '_>,
    session: &mut PackageBuildSession,
    source_bindings: &mut PackageSourceBindings,
    builds: &crate::project::PackageReviewBuilds,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    session.require_open()?;
    let result = review_candidate(candidate, source_bindings, builds);
    let accounting = match &result {
        Ok(compiled) => compiled.source_completion_accounting(),
        Err(error) => error
            .source_completion_accounting()
            .cloned()
            .unwrap_or_default(),
    };
    session.record_source_completion(accounting);
    result.map(|compiled| compiled.reviews)
}

fn review_candidate(
    candidate: CandidateReview<'_, '_>,
    source_bindings: &mut PackageSourceBindings,
    builds: &crate::project::PackageReviewBuilds,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
    let CandidateReview {
        target_closure,
        root_build_snapshot,
    } = candidate;
    let _requested = timings::requested(source_bindings.collect_timings);
    let _stage = timings::stage("candidate_compilation");
    source_bindings.size_for(target_closure.source_closure());
    let preliminary = {
        let _pass = timings::subject_stage(|| {
            format!(
                "review_pass discovery root={}",
                target_closure
                    .source_closure()
                    .graph()
                    .root()
                    .name()
                    .as_str()
            )
        });
        note_pass_accounting("discovery", || {
            review_pass(
                target_closure,
                &[],
                root_build_snapshot,
                None,
                source_bindings,
                builds,
            )
        })?
    };
    let accounting = preliminary.source_completion_accounting();
    let discovered = candidate_semantic_binding_inputs(
        &preliminary.reviews,
        target_closure.source_closure().graph().root(),
    )
    .map_err(|error| error.with_source_completion_accounting(accounting.clone()))?;
    if discovered.is_empty() {
        return Ok(preliminary);
    }
    let _pass = timings::subject_stage(|| {
        format!(
            "review_pass bound root={} discovered_bindings={}",
            target_closure
                .source_closure()
                .graph()
                .root()
                .name()
                .as_str(),
            discovered.len()
        )
    });
    note_pass_accounting("bound", || {
        review_pass(
            target_closure,
            &discovered,
            root_build_snapshot,
            Some(preliminary),
            source_bindings,
            builds,
        )
    })
    .map_err(|error| error.with_source_completion_accounting(accounting))
}

fn note_pass_accounting(
    pass: &'static str,
    run: impl FnOnce() -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError>,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
    let result = run()?;
    let occurrences = result.occurrences.len();
    let carried = result.carried;
    timings::note(|| format!("review_pass {pass} occurrences={occurrences} carried={carried}"));
    Ok(result)
}

fn review_pass(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    bindings: &[ConsumerScopedSemanticBindingReviewInput],
    root_build_snapshot: Option<&BuildSnapshotRequest>,
    prior: Option<RetainedPackageReviews>,
    source_bindings: &mut PackageSourceBindings,
    builds: &crate::project::PackageReviewBuilds,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let execution_profile = target::TargetProfile::host_if_supported();
    let source = CanonicalSourceClosureSubject::from_resolved(target_closure, Default::default())
        .map_err(|error| {
        CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::Projection {
                package: closure.graph().root().clone(),
                diagnostics: vec![diagnostics::Diagnostic::error(error.to_string())],
            },
        )
    })?;
    let roster = crate::lock::PackageOccurrenceRoster::derive(&source).map_err(|error| {
        CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::Projection {
                package: closure.graph().root().clone(),
                diagnostics: vec![diagnostics::Diagnostic::error(error.to_string())],
            },
        )
    })?;
    let bindings =
        semantic_bindings_by_consumer(target_closure, execution_profile, &roster, bindings)?;
    let plan = PackageReviewPass::admit(
        target_closure,
        execution_profile,
        &roster,
        &bindings,
        root_build_snapshot,
    )?;
    plan.review(source_bindings, builds, prior)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_custody::temporary_directory::TemporaryDirectory;
    use std::path::Path;

    #[test]
    fn bound_pass_moves_an_unchanged_occurrences_canonical_rows() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        package_compilation::toolchain_library::configure(&repository.join("source/library"))
            .unwrap();
        let entry = repository.join("source-rust/subsystems/compiler/tests/fixtures/build/pass/acquired_generator_completed_output_set/generator/main.omg");
        let profile = target::TargetProfile::host_if_supported().unwrap();
        let preparation =
            crate::dependencies::begin_local_project_preparation(&entry, &[profile], true).unwrap();
        let workspace = TemporaryDirectory::create("carry-package-review", false).unwrap();
        let mut session = PackageBuildSession::create(workspace.path()).unwrap();
        let project = crate::project::resolve_build_sources(
            &mut session,
            preparation.acquire_root().unwrap(),
            &mut preparation.source_acquisitions().unwrap(),
            Default::default(),
        )
        .unwrap();
        let closure = project.sources.into_source_closure();
        let target = closure.for_exact_target(profile);
        let mut bindings = PackageSourceBindings::default();
        bindings.size_for(&closure);
        let mut pass =
            |prior| review_pass(&target, &[], None, prior, &mut bindings, &project.builds);
        let preliminary = pass(None).unwrap();
        let [review] = preliminary.reviews.reviews() else {
            panic!("expected one package review");
        };
        assert!(!review.policy_rows.is_empty());
        let rows = review.policy_rows.as_ptr();
        let charge = preliminary.occurrences[0].policy_canonical_bytes;
        let bound = pass(Some(preliminary)).unwrap();
        assert_eq!(bound.carried, 1);
        assert_eq!(bound.reviews.reviews()[0].policy_rows.as_ptr(), rows);
        assert_eq!(bound.occurrences[0].policy_canonical_bytes, charge);
        session.close().unwrap();
    }
}

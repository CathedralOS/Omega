/*
Candidate checking, project policy comparison, and proposed target lock section construction.

`review_package_change_reusing` checks a resolved source closure for one target profile
using the shared `PackageBuildSession` and compares its compiler-issued policy baselines
against the accepted target section in `omega.lock`. `PackageChangeReview::propose_lock_target`
verifies that all required policy changes were accepted, re-verifies transitive source
custody and compiler input identity, and assembles the proposed `PackageLockTarget`
without writing `build.omg` or `omega.lock`.
*/

use crate::build_session::PackageBuildSession;

mod error;
pub(crate) use error::PackageChangeError;

use crate::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ResolvedPackageSourceClosure,
};
use crate::review::{
    CandidateReview, CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits,
    PackagePolicyChangeSet, PackagePolicyResolution, PackageSourceBindings,
    PackageSourceVerificationPhase, compare_package_policy_changes, review_resolved_packages,
    verify_transitive_source_custody,
};
use target::TargetProfile;

#[derive(Debug)]
pub struct PackageChangeReview {
    source_closure: ResolvedPackageSourceClosure,
    reviews: CompilerIssuedPackageReviewSet,
    changes: PackagePolicyChangeSet,
    target: TargetProfile,
    compiler_identity: [u8; 32],
}

impl PackageChangeReview {
    pub const fn source_closure(&self) -> &ResolvedPackageSourceClosure {
        &self.source_closure
    }

    pub const fn reviews(&self) -> &CompilerIssuedPackageReviewSet {
        &self.reviews
    }

    pub const fn changes(&self) -> &PackagePolicyChangeSet {
        &self.changes
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub fn propose_lock_target(
        &self,
        resolution: &PackagePolicyResolution,
    ) -> Result<PackageLockTarget, PackageChangeError> {
        verify_compiler_identity(self.compiler_identity)?;
        if resolution.comparison() != self.changes.fingerprint() {
            return Err(PackageChangeError::Decisions(
                crate::lock::HistoricalPackagePolicyError::ResolutionMismatch,
            ));
        }
        if !resolution.all_required_changes_accepted() {
            return Err(PackageChangeError::RejectedChanges);
        }
        verify_transitive_source_custody(
            &self.source_closure,
            self.source_closure.graph().root(),
            PackageSourceVerificationPhase::AfterCompilation,
        )
        .map_err(PackageChangeError::Compilation)?;
        let source = CanonicalSourceClosureSubject::from_resolved(
            &self.source_closure.for_exact_target(self.target),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(PackageChangeError::SourceSubject)?;
        let decisions = HistoricalPackagePolicyDecisions::capture_policy(
            &source,
            &self.changes,
            resolution,
            HistoricalPackagePolicyLimits::default(),
        )
        .map_err(PackageChangeError::Decisions)?;
        let mut policies = Vec::new();
        crate::reserve!(policies, self.reviews.reviews().len(), PackageChangeError);
        for package in source.packages() {
            for purpose in crate::declarations::DependencyPurpose::ALL {
                if let Some(review) = self.reviews.review_occurrence(package.key(), purpose) {
                    policies.push((
                        review.checked_context(),
                        review.policy(),
                        review.semantic_bindings(),
                    ));
                }
            }
        }
        PackageLockTarget::from_accepted_policies(
            source,
            &policies,
            self.compiler_identity,
            decisions,
        )
        .map_err(PackageChangeError::Lock)
    }
}

pub fn review_package_change_reusing(
    source_closure: ResolvedPackageSourceClosure,
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    session: &mut PackageBuildSession,
    root_build_snapshot: Option<&build_settings::BuildSnapshotRequest>,
    source_bindings: &mut PackageSourceBindings,
    execute: &mut crate::review::PackageReviewExecutor<'_>,
) -> Result<PackageChangeReview, PackageChangeError> {
    if accepted.is_some_and(|accepted| accepted.target() != target) {
        return Err(PackageChangeError::Comparison(
            crate::review::PackagePolicyChangeError::TargetMismatch,
        ));
    }
    let compiler_identity = super::accepted_compilation::compiler_input_identity()
        .map_err(PackageChangeError::CompilerIdentity)?;
    let target_closure = source_closure.for_exact_target(target);
    let reviews = review_resolved_packages(
        CandidateReview {
            target_closure: &target_closure,
            root_build_snapshot,
        },
        session,
        source_bindings,
        execute,
    )
    .map_err(PackageChangeError::Compilation)?;
    verify_compiler_identity(compiler_identity)?;
    let changes = compare_package_policy_changes(
        accepted,
        &reviews,
        &target_closure,
        PackagePolicyChangeLimits::default(),
    )
    .map_err(PackageChangeError::Comparison)?;
    Ok(PackageChangeReview {
        source_closure,
        reviews,
        changes,
        target,
        compiler_identity,
    })
}

fn verify_compiler_identity(expected: [u8; 32]) -> Result<(), PackageChangeError> {
    let current = super::accepted_compilation::compiler_input_identity()
        .map_err(PackageChangeError::CompilerIdentity)?;
    if current != expected {
        return Err(PackageChangeError::CompilerIdentity(
            "compiler or toolchain library inputs changed after review; run omega packages update again"
                .into(),
        ));
    }
    Ok(())
}

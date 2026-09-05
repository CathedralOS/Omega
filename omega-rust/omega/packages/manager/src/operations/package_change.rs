//! Candidate checking, project review, and a proposed source lock section.

mod error;
pub use error::PackageChangeError;

use crate::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ResolvedPackageSourceClosure,
};
use crate::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits, PackagePolicyChangeSet,
    PackagePolicyResolution, PackageSourceVerificationPhase, compare_package_policy_changes,
    compile_resolved_package_candidate_reviews, verify_transitive_source_custody,
};
use std::path::Path;
use target::TargetProfile;

/// One checked candidate and its comparison. This holds the source snapshot
/// needed for review and lock construction, not a package certificate or file
/// publication permission. No accepted project file is changed by this type.
#[derive(Debug)]
pub struct PackageChangeReview {
    source_closure: ResolvedPackageSourceClosure,
    reviews: CompilerIssuedPackageReviewSet,
    changes: PackagePolicyChangeSet,
    target: TargetProfile,
}

impl PackageChangeReview {
    pub const fn source_closure(&self) -> &ResolvedPackageSourceClosure {
        &self.source_closure
    }

    pub const fn reviews(&self) -> &CompilerIssuedPackageReviewSet {
        &self.reviews
    }

    /// Feed this report to the ordinary review document renderer and recovery
    /// functions. An absent baseline means fresh review, not silent acceptance.
    pub const fn changes(&self) -> &PackagePolicyChangeSet {
        &self.changes
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    /// Construct the proposed target section after complete accepting choices.
    /// Recheck retained source snapshots and selections before using their pins.
    ///
    /// The caller still owns file-transaction locking, concurrent project edits,
    /// immediate pre-publication source/build checks, and other selected targets.
    /// This function writes neither `build.omg` nor `omega.lock`, and emits no
    /// native artifact. A rejection leaves this review available for inspection.
    pub fn propose_lock_target(
        &self,
        resolution: &PackagePolicyResolution,
    ) -> Result<PackageLockTarget, PackageChangeError> {
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
        let mut baselines = Vec::new();
        baselines
            .try_reserve_exact(source.packages().len())
            .map_err(|_| PackageChangeError::AllocationFailed)?;
        for package in source.packages() {
            let review = self
                .reviews
                .review(package.key())
                .expect("comparison checked complete source coverage");
            baselines.push(review.policy().clone());
        }
        PackageLockTarget::from_parts(source, baselines, decisions)
            .map_err(PackageChangeError::Lock)
    }
}

/// Check and compare a resolved install/update candidate for one exact target.
/// The caller resolves the requested candidate before executing build code.
/// Only the compiler's existing scoped build evaluator is used; dependencies
/// receive no resolver or host credential object through this interface.
///
/// Ordinary proof failures cannot be approved as capability changes. Explicit
/// accepted assumptions remain policy findings and use the normal decisions.
/// No old source, native emission, reconstruction question, or admission
/// promotion is needed. Resume after a source edit by creating a new review and
/// recovering saved decisions against its new comparison, not the old object.
pub fn review_package_change(
    source_closure: ResolvedPackageSourceClosure,
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    build_root: &Path,
) -> Result<PackageChangeReview, PackageChangeError> {
    if accepted.is_some_and(|accepted| accepted.target() != target) {
        return Err(PackageChangeError::Comparison(
            crate::review::PackagePolicyChangeError::TargetMismatch,
        ));
    }
    let target_closure = source_closure.for_exact_target(target);
    let reviews = compile_resolved_package_candidate_reviews(&target_closure, build_root)
        .map_err(PackageChangeError::Compilation)?;
    for review in reviews.reviews() {
        let count = review
            .obligation_results()
            .open_contract_entailment_obligations()
            .len();
        if count != 0 {
            return Err(PackageChangeError::UndischargedContract {
                package: Box::new(review.key().clone()),
                count,
            });
        }
    }
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
    })
}

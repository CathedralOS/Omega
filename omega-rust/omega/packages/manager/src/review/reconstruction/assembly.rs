use super::encoding::{encode_question, fingerprint};
use super::model::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionLimits,
};
use super::validation::validate_association;
use crate::lock::PackageOccurrenceRoster;
use crate::resolution::graph::{CanonicalSourceClosureSubject, ExactTargetPackageSourceClosure};
use crate::resolution::package_compilation_inputs_for;
use crate::review::{CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet};
use std::collections::BTreeMap;

impl CanonicalPackageReconstructionQuestion {
    /// Associate one freshly resolved source closure with the complete review
    /// set produced by package-aware local compilation.
    ///
    /// `CompilerIssuedPackageReviewSet` has no public constructor and each of
    /// its ledgers has already passed exact local reconstruction. This method
    /// additionally rejoins every review to resolver identity, immutable
    /// resolution, and the exact transitive source graph.
    pub fn from_resolved_and_reviews(
        target_closure: &ExactTargetPackageSourceClosure<'_>,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<Self, CanonicalPackageReconstructionQuestionError> {
        Self::associate_resolved_reviews(target_closure, reviews, limits)
            .map(|(question, _)| question)
    }

    /// Retain the exact source-ordered review association for the rest of this
    /// operation. These borrows are scratch data, not a new acceptance carrier.
    pub(super) fn associate_resolved_reviews<'reviews>(
        target_closure: &ExactTargetPackageSourceClosure<'_>,
        reviews: &'reviews CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<
        (Self, Vec<&'reviews CompilerIssuedPackageReview>),
        CanonicalPackageReconstructionQuestionError,
    > {
        let limits = limits.compiler_bounded();
        let source_closure =
            CanonicalSourceClosureSubject::from_resolved(target_closure, limits.source_closure)
                .map_err(|_| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "could not project the canonical source-closure subject",
                    )
                })?;
        let roster = PackageOccurrenceRoster::derive(&source_closure).map_err(|_| {
            CanonicalPackageReconstructionQuestionError::new(
                "could not derive the source closure occurrence roster",
            )
        })?;

        let closure = target_closure.source_closure();
        let mut reviews_by_package = BTreeMap::new();
        for review in reviews.reviews() {
            if reviews_by_package.insert(review.key(), review).is_some() {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package review set contains a duplicate package",
                ));
            }
        }
        if reviews_by_package.len() != source_closure.packages().len() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "source closure and package review set are not bijective",
            ));
        }

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(source_closure.packages().len())
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction entry allocation failed",
                )
            })?;
        let mut associated_reviews = Vec::new();
        associated_reviews
            .try_reserve_exact(source_closure.packages().len())
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package review association allocation failed",
                )
            })?;
        for selected in source_closure.packages() {
            let review = reviews_by_package.remove(selected.key()).ok_or_else(|| {
                CanonicalPackageReconstructionQuestionError::new(
                    "source package has no matching package review",
                )
            })?;
            if review.resolution() != selected.resolution() {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package review immutable resolution does not match source custody",
                ));
            }
            let expected_dependency_closure =
                package_compilation_inputs_for(closure, selected.key())
                    .map_err(|_| {
                        CanonicalPackageReconstructionQuestionError::new(
                            "could not independently reconstruct the package dependency closure",
                        )
                    })?
                    .dependency_closure();
            if review.obligations().dependency_closure() != &expected_dependency_closure {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package review dependency closure does not match current source custody",
                ));
            }
            // Where the review carried a build observation, its activation must
            // name this exact package occurrence and the closure's target, not
            // another package's build or a different target's admission.
            if let Some(summary) = review.build_observation_summary() {
                let activation = summary.activation();
                if activation
                    .root_package_identity()
                    .is_some_and(|root| root != review.key().identity())
                {
                    return Err(CanonicalPackageReconstructionQuestionError::new(
                        "package review build activation names a different package occurrence",
                    ));
                }
                if activation
                    .selected_target_profile()
                    .is_some_and(|target| target != target_closure.target_profile())
                {
                    return Err(CanonicalPackageReconstructionQuestionError::new(
                        "package review build activation names a different target",
                    ));
                }
            }
            entries.push(CanonicalPackageReconstructionEntry {
                package: selected.key().clone(),
                occurrence_purposes: roster
                    .purposes(selected.key())
                    .expect("validated source package has an occurrence")
                    .to_vec(),
                obligations: review.obligations().clone(),
            });
            associated_reviews.push(review);
        }
        if !reviews_by_package.is_empty() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package review set contains a package outside the source closure",
            ));
        }

        let question = Self::finish(source_closure, entries, limits)?;
        Ok((question, associated_reviews))
    }

    /// Reproject the complete question from current resolver custody and fresh
    /// package-aware reviews, then require exact equality.
    pub fn matches_resolved_and_reviews(
        &self,
        target_closure: &ExactTargetPackageSourceClosure<'_>,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<bool, CanonicalPackageReconstructionQuestionError> {
        Ok(self == &Self::from_resolved_and_reviews(target_closure, reviews, limits)?)
    }

    pub(super) fn finish(
        source_closure: CanonicalSourceClosureSubject,
        entries: Vec<CanonicalPackageReconstructionEntry>,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<Self, CanonicalPackageReconstructionQuestionError> {
        let limits = limits.compiler_bounded();
        validate_association(&source_closure, &entries, limits)?;
        let canonical_bytes = encode_question(&source_closure, &entries, limits)?;
        let fingerprint = fingerprint(&canonical_bytes);
        Ok(Self {
            source_closure,
            entries,
            canonical_bytes,
            fingerprint,
        })
    }
}

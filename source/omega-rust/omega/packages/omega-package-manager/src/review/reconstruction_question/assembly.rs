use super::encoding::{encode_question, fingerprint};
use super::model::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionLimits,
};
use super::validation::validate_association;
use crate::{
    CanonicalSourceClosureSubject, CompilerIssuedPackageReviewSet, ResolvedPackageSourceClosure,
    package_compilation_inputs_for,
};
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
        closure: &ResolvedPackageSourceClosure,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<Self, CanonicalPackageReconstructionQuestionError> {
        let limits = limits.compiler_bounded();
        let source_closure =
            CanonicalSourceClosureSubject::from_resolved(closure, limits.source_closure).map_err(
                |_| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "could not project the canonical source-closure subject",
                    )
                },
            )?;

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
            if review.obligation_ledger().dependency_closure() != &expected_dependency_closure {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package review dependency closure does not match current source custody",
                ));
            }
            entries.push(CanonicalPackageReconstructionEntry {
                package: selected.key().clone(),
                obligation_ledger: review.obligation_ledger().clone(),
            });
        }
        if !reviews_by_package.is_empty() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package review set contains a package outside the source closure",
            ));
        }

        Self::finish(source_closure, entries, limits)
    }

    /// Reproject the complete question from current resolver custody and fresh
    /// package-aware reviews, then require exact equality.
    pub fn matches_resolved_and_reviews(
        &self,
        closure: &ResolvedPackageSourceClosure,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<bool, CanonicalPackageReconstructionQuestionError> {
        Ok(self == &Self::from_resolved_and_reviews(closure, reviews, limits)?)
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

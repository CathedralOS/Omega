//! In-memory transitive composition of locally reconstructed open claims.

use super::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionError,
    CanonicalPackageReconstructionQuestionLimits,
};
use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::CompilerIssuedPackageReviewSet;
use omega_package_evidence::ledger::{
    OrdinaryPackageAcceptedClaimObligation, OrdinaryPackageObligationResultSet,
};
use std::collections::BTreeMap;

/// One package's locally reconstructed result set within an exact source
/// closure. No producer policy decision is representable here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocallyComposedPackageObligationEntry {
    package: PackageKey,
    results: OrdinaryPackageObligationResultSet,
}

impl LocallyComposedPackageObligationEntry {
    pub const fn package(&self) -> &PackageKey {
        &self.package
    }

    pub const fn results(&self) -> &OrdinaryPackageObligationResultSet {
        &self.results
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenAcceptedClaimReference {
    package_index: usize,
    claim_index: usize,
}

/// Exact source/question association plus every open accepted claim reachable
/// by the selected root.
///
/// This is deliberately in-memory. It has no codec, lock promotion, admission
/// disposition, or `PackageInstance` constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocallyComposedPackageObligationResults {
    question: CanonicalPackageReconstructionQuestion,
    entries: Vec<LocallyComposedPackageObligationEntry>,
    root_open_accepted_claims: Vec<OpenAcceptedClaimReference>,
}

impl LocallyComposedPackageObligationResults {
    /// Compose fresh compiler results over one exact resolver-owned closure.
    pub fn from_resolved_and_reviews(
        closure: &ResolvedPackageSourceClosure,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<Self, CanonicalPackageReconstructionQuestionError> {
        let question = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
            closure, reviews, limits,
        )?;
        let mut reviews_by_package = BTreeMap::new();
        for review in reviews.reviews() {
            if reviews_by_package.insert(review.key(), review).is_some() {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package obligation composition contains a duplicate review",
                ));
            }
        }

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(question.entries().len())
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package obligation composition entry allocation failed",
                )
            })?;
        let mut open_count = 0usize;
        for question_entry in question.entries() {
            let review = reviews_by_package
                .remove(question_entry.package())
                .ok_or_else(|| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package obligation composition is missing a reviewed package",
                    )
                })?;
            let results = review.obligation_results();
            if results.package() != question_entry.package().identity()
                || results.schema() != question_entry.obligations().schema()
                || results.target() != question_entry.obligations().target()
                || results.dependency_closure() != question_entry.obligations().dependency_closure()
            {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package obligation results do not match their reconstructed question",
                ));
            }
            open_count = open_count
                .checked_add(results.open_accepted_claims().len())
                .ok_or_else(|| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package obligation open-claim count overflowed",
                    )
                })?;
            entries.push(LocallyComposedPackageObligationEntry {
                package: question_entry.package().clone(),
                results: results.clone(),
            });
        }
        if !reviews_by_package.is_empty() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package obligation composition contains a review outside the source closure",
            ));
        }

        let mut root_open_accepted_claims = Vec::new();
        root_open_accepted_claims
            .try_reserve_exact(open_count)
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package obligation open-claim reference allocation failed",
                )
            })?;
        for (package_index, entry) in entries.iter().enumerate() {
            for claim_index in 0..entry.results.open_accepted_claims().len() {
                root_open_accepted_claims.push(OpenAcceptedClaimReference {
                    package_index,
                    claim_index,
                });
            }
        }

        Ok(Self {
            question,
            entries,
            root_open_accepted_claims,
        })
    }

    pub const fn question(&self) -> &CanonicalPackageReconstructionQuestion {
        &self.question
    }

    pub fn entries(&self) -> &[LocallyComposedPackageObligationEntry] {
        &self.entries
    }

    /// Iterate every open claim propagated to the selected root. The owner is
    /// retained independently, so a dependency claim cannot become a root-
    /// authored claim.
    pub fn root_open_accepted_claims(
        &self,
    ) -> impl ExactSizeIterator<Item = (&PackageKey, &OrdinaryPackageAcceptedClaimObligation)> {
        self.root_open_accepted_claims.iter().map(|reference| {
            let entry = &self.entries[reference.package_index];
            (
                &entry.package,
                &entry.results.open_accepted_claims()[reference.claim_index],
            )
        })
    }
}

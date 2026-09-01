//! In-memory transitive composition of locally reconstructed open obligations.

use super::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionError,
    CanonicalPackageReconstructionQuestionLimits,
};
use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::CompilerIssuedPackageReviewSet;
use omega_package_evidence::ledger::{
    OrdinaryPackageAcceptedClaimObligation, OrdinaryPackageDangerousAuthorityObligation,
    OrdinaryPackageExternalExecutableSupplyObligation, OrdinaryPackageObligationResultSet,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenExternalExecutableSupplyReference {
    package_index: usize,
    supply_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenDangerousAuthorityReference {
    package_index: usize,
    authority_index: usize,
}

/// Exact source/question association plus every supported open obligation
/// reachable by the selected root.
///
/// This is deliberately in-memory. It has no codec, lock promotion, admission
/// disposition, or `PackageInstance` constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocallyComposedPackageObligationResults {
    question: CanonicalPackageReconstructionQuestion,
    entries: Vec<LocallyComposedPackageObligationEntry>,
    root_open_accepted_claims: Vec<OpenAcceptedClaimReference>,
    root_open_external_executable_supplies: Vec<OpenExternalExecutableSupplyReference>,
    root_open_dangerous_authorities: Vec<OpenDangerousAuthorityReference>,
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
        let mut open_claim_count = 0usize;
        let mut open_external_supply_count = 0usize;
        let mut open_dangerous_authority_count = 0usize;
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
            open_claim_count = open_claim_count
                .checked_add(results.open_accepted_claims().len())
                .ok_or_else(|| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package obligation open-claim count overflowed",
                    )
                })?;
            open_external_supply_count = open_external_supply_count
                .checked_add(results.open_external_executable_supplies().len())
                .ok_or_else(|| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package obligation open external executable-supply count overflowed",
                    )
                })?;
            open_dangerous_authority_count = open_dangerous_authority_count
                .checked_add(results.open_dangerous_authorities().len())
                .ok_or_else(|| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package obligation open dangerous-authority count overflowed",
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
            .try_reserve_exact(open_claim_count)
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

        let mut root_open_external_executable_supplies = Vec::new();
        root_open_external_executable_supplies
            .try_reserve_exact(open_external_supply_count)
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package obligation open external executable-supply reference allocation failed",
                )
            })?;
        for (package_index, entry) in entries.iter().enumerate() {
            for supply_index in 0..entry.results.open_external_executable_supplies().len() {
                root_open_external_executable_supplies.push(
                    OpenExternalExecutableSupplyReference {
                        package_index,
                        supply_index,
                    },
                );
            }
        }

        let mut root_open_dangerous_authorities = Vec::new();
        root_open_dangerous_authorities
            .try_reserve_exact(open_dangerous_authority_count)
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package obligation open dangerous-authority reference allocation failed",
                )
            })?;
        for (package_index, entry) in entries.iter().enumerate() {
            for authority_index in 0..entry.results.open_dangerous_authorities().len() {
                root_open_dangerous_authorities.push(OpenDangerousAuthorityReference {
                    package_index,
                    authority_index,
                });
            }
        }

        Ok(Self {
            question,
            entries,
            root_open_accepted_claims,
            root_open_external_executable_supplies,
            root_open_dangerous_authorities,
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

    /// Iterate every open external executable supply propagated to the
    /// selected root while retaining its original package owner.
    pub fn root_open_external_executable_supplies(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &PackageKey,
            &OrdinaryPackageExternalExecutableSupplyObligation,
        ),
    > {
        self.root_open_external_executable_supplies
            .iter()
            .map(|reference| {
                let entry = &self.entries[reference.package_index];
                (
                    &entry.package,
                    &entry.results.open_external_executable_supplies()[reference.supply_index],
                )
            })
    }

    /// Iterate every dangerous authority propagated to the selected root
    /// while retaining its original package owner.
    pub fn root_open_dangerous_authorities(
        &self,
    ) -> impl ExactSizeIterator<Item = (&PackageKey, &OrdinaryPackageDangerousAuthorityObligation)>
    {
        self.root_open_dangerous_authorities
            .iter()
            .map(|reference| {
                let entry = &self.entries[reference.package_index];
                (
                    &entry.package,
                    &entry.results.open_dangerous_authorities()[reference.authority_index],
                )
            })
    }
}

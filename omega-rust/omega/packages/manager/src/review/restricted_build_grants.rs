//! The execution-time grant join between a fresh compile and the consuming
//! project's accepted lock policy
//! (wiki/spec/packages/acceptance.md#restricted-build-acceptance).
//!
//! Review compiles are observational: they run package builds under the
//! caller's staging sponsors so a decision document can describe what each
//! activation asked of the host, and a fresh install must be able to observe
//! a not-yet-accepted request. A lock that already retains decisions is
//! different: an operation consuming that compile must refuse any occurrence
//! whose projected restricted build requests lack retained accepted-request
//! meaning. The lock stores decisions, not credentials, so the join
//! re-projects each occurrence's acceptance rows and compares complete
//! normalized request meaning — never host state — before the compile's
//! generated sources and checked results are consumed.

use std::collections::BTreeSet;

use package_evidence::record::PackagePolicyRowKind;
use semantic_vocabulary::PackageKeyIdentity;

use super::CompilerIssuedPackageReviewSet;
use crate::declarations::DependencyPurpose;
use crate::lock::{
    PackageAcceptanceRow, PackageCheckedContext, PackageLockError, PackageLockTarget,
    PackagePolicyAcceptance,
};

/// One normalized restricted build request a checked package occurrence
/// projected without identical retained accepted-request meaning. Its
/// compile result must not be consumed as locked evidence: acceptance is the
/// only authority a locked operation honors for restricted host reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UngrantedRestrictedBuildRequest {
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    request_meaning: String,
}

impl UngrantedRestrictedBuildRequest {
    /// Canonical identity of the package whose occurrence asked for the
    /// ungranted authority.
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    /// The authorized context — product or build — the request belongs to.
    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }

    /// The complete normalized request meaning the accepted policy did not
    /// grant, in the same canonical row form the decision document renders.
    pub fn request_meaning(&self) -> &str {
        &self.request_meaning
    }
}

impl std::fmt::Display for UngrantedRestrictedBuildRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut identity = String::new();
        for byte in &self.package.digest()[..4] {
            identity.push_str(&format!("{byte:02x}"));
        }
        write!(
            formatter,
            "package {identity}… {} occurrence requests:\n    {}",
            self.purpose.name(),
            self.request_meaning
        )
    }
}

/// The restricted-request consent a consuming compile carries into the
/// candidate pass: the accepted target's retained request meanings bound to
/// the exact checked occurrence — package identity plus `PackageCheckedContext`
/// — each grants.
///
/// Observation passes carry no checkpoint and never consult one: audit-only
/// inspection must not issue grants, and a not-yet-accepted request has to
/// project so the decision document can present it. A consuming compile arms
/// the checkpoint so an occurrence's projected request joins before its
/// review is retained or its generated-source bundle hands off to a consumer;
/// acceptance recorded for one purpose or target never authorizes a different
/// occurrence's restricted action on the same package.
#[derive(Debug, Clone)]
pub struct RestrictedBuildCheckpoint {
    grants: Vec<(PackageKeyIdentity, PackageCheckedContext, BTreeSet<String>)>,
}

impl RestrictedBuildCheckpoint {
    /// Retain each accepted occurrence's granted restricted-request meanings
    /// under its own checked context. Rows outside the restricted-request
    /// kind never enter the grant view.
    pub fn derive(accepted: &PackageLockTarget) -> Self {
        Self {
            grants: accepted
                .occurrences()
                .iter()
                .map(|occurrence| {
                    (
                        occurrence.acceptance().package(),
                        occurrence.context(),
                        occurrence
                            .acceptance()
                            .rows()
                            .iter()
                            .filter(|row| {
                                row.kind() == PackagePolicyRowKind::RestrictedBuildRequest
                            })
                            .map(|row| row.canonical_text().to_owned())
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    /// The projected request meanings one checked occurrence asked that this
    /// checkpoint does not grant — an absent occurrence, a missing row, or a
    /// request that widened since acceptance each surface ungranted.
    pub fn ungranted_requests<'a>(
        &self,
        package: PackageKeyIdentity,
        context: PackageCheckedContext,
        projected: impl Iterator<Item = &'a str>,
    ) -> Vec<UngrantedRestrictedBuildRequest> {
        let granted = self
            .grants
            .iter()
            .find(|(identity, granted_context, _)| {
                *identity == package && *granted_context == context
            })
            .map(|(_, _, texts)| texts);
        projected
            .filter(|meaning| granted.is_none_or(|texts| !texts.contains(*meaning)))
            .map(|meaning| UngrantedRestrictedBuildRequest {
                package,
                purpose: context.purpose(),
                request_meaning: meaning.to_owned(),
            })
            .collect()
    }
}

/// Join one accepted lock target against the restricted build requests a
/// fresh review projected per package occurrence.
///
/// A request is granted when its occurrence's retained acceptance rows carry
/// an identical normalized request meaning. An absent occurrence, a missing
/// row, or a request that widened since acceptance all surface ungranted.
/// Occurrences match on the complete checked context — package identity,
/// purpose, selected target, and build execution profile — so a build-purpose
/// grant never authorizes a product-purpose activation.
pub fn ungranted_restricted_build_requests(
    accepted: &PackageLockTarget,
    reviews: &CompilerIssuedPackageReviewSet,
) -> Result<Vec<UngrantedRestrictedBuildRequest>, PackageLockError> {
    let checkpoint = RestrictedBuildCheckpoint::derive(accepted);
    let mut ungranted = Vec::new();
    for review in reviews.reviews() {
        if review.restricted_build_requests().is_empty() {
            continue;
        }
        // Re-project this review's own acceptance rows so both sides of the
        // join read the same normalized meaning form the lock retained.
        let projected = PackagePolicyAcceptance::from_policy(review.policy())?;
        ungranted.extend(
            checkpoint.ungranted_requests(
                review.key().identity(),
                review.checked_context(),
                projected
                    .rows()
                    .iter()
                    .filter(|row| row.kind() == PackagePolicyRowKind::RestrictedBuildRequest)
                    .map(PackageAcceptanceRow::canonical_text),
            ),
        );
    }
    Ok(ungranted)
}

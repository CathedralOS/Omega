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

use diagnostics::Diagnostic;
use package_evidence::record::PackagePolicyRowKind;
use semantic_vocabulary::PackageKeyIdentity;

use super::CompilerIssuedPackageReviewSet;
use crate::declarations::DependencyPurpose;
use crate::lock::{
    PackageAcceptanceRow, PackageCheckedContext, PackageLockError, PackageLockTarget,
    PackagePolicyAcceptance,
};
use crate::resolution::graph::{DependencyRequestPath, ResolvedPackageSourceClosure};
use crate::review::timings;

/// One normalized restricted build request a checked package occurrence
/// projected without identical retained accepted-request meaning. Its
/// compile result must not be consumed as locked evidence: acceptance is the
/// only authority a locked operation honors for restricted host reach.
///
/// Restricted-build acceptance attributes each request to the originating
/// package *and the dependency path* by which resolution reached it, so the
/// record carries the occurrence's shortest root-to-package route alongside
/// its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UngrantedRestrictedBuildRequest {
    package: PackageKeyIdentity,
    context: PackageCheckedContext,
    request_meaning: String,
    request_path: Option<DependencyRequestPath>,
}

impl UngrantedRestrictedBuildRequest {
    /// Canonical identity of the package whose occurrence asked for the
    /// ungranted authority.
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    /// The complete checked context — purpose, selected target, and build
    /// execution profile — whose request lacks retained accepted meaning.
    /// Sibling occurrences of one package grant independently, so the
    /// context is the attribution, not an optional refinement.
    pub const fn context(&self) -> PackageCheckedContext {
        self.context
    }

    /// The authorized context — product or build — the request belongs to.
    pub const fn purpose(&self) -> DependencyPurpose {
        self.context.purpose()
    }

    /// The complete normalized request meaning the accepted policy did not
    /// grant, in the same canonical row form the decision document renders.
    pub fn request_meaning(&self) -> &str {
        &self.request_meaning
    }

    /// The shortest root-to-package request path by which source resolution
    /// reached the requesting occurrence, in the same root/alias/target form
    /// the decision document's `path` lines render. `None` only when the
    /// reviewed key holds no custody in the joining closure.
    pub fn request_path(&self) -> Option<&DependencyRequestPath> {
        self.request_path.as_ref()
    }
}

impl std::fmt::Display for UngrantedRestrictedBuildRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut identity = String::new();
        for byte in &self.package.digest()[..4] {
            identity.push_str(&format!("{byte:02x}"));
        }
        writeln!(
            formatter,
            "package {identity}… {} {} occurrence requests:\n    {}",
            self.context.purpose().name(),
            self.context.target().identity().as_str(),
            self.request_meaning
        )?;
        if let Some(profile) = self.context.build_execution_profile() {
            writeln!(formatter, "    build host {}", profile.identity().as_str())?;
        }
        match &self.request_path {
            Some(path) => {
                write!(formatter, "    path ")?;
                for byte in path.root().identity().digest() {
                    write!(formatter, "{byte:02x}")?;
                }
                for step in path.steps() {
                    write!(formatter, " -> {:?}", step.alias().as_str())?;
                    write!(formatter, " ")?;
                    for byte in step.target().identity().digest() {
                        write!(formatter, "{byte:02x}")?;
                    }
                }
                Ok(())
            }
            None => write!(formatter, "    path none"),
        }
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
    /// request that widened since acceptance each surface ungranted. The
    /// occurrence's dependency route is shared across its projected requests
    /// and attaches to each gap record.
    pub fn ungranted_requests<'a>(
        &self,
        package: PackageKeyIdentity,
        context: PackageCheckedContext,
        request_path: Option<DependencyRequestPath>,
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
                context,
                request_meaning: meaning.to_owned(),
                request_path: request_path.clone(),
            })
            .collect()
    }

    /// Arm the checkpoint for one admitted activation: the returned binding
    /// crosses to the compile worker and joins each restricted request the
    /// occurrence's build machine projects — by canonical accepted-request
    /// meaning — before that request's own build effect executes. An
    /// occurrence absent from the accepted target carries an empty grant
    /// view: every request it projects refuses.
    pub fn grants(
        &self,
        package: PackageKeyIdentity,
        context: PackageCheckedContext,
        request_path: Option<DependencyRequestPath>,
    ) -> CheckpointRestrictedBuildGrants {
        CheckpointRestrictedBuildGrants {
            package,
            context,
            request_path,
            granted: self
                .grants
                .iter()
                .find(|(identity, granted_context, _)| {
                    *identity == package && *granted_context == context
                })
                .map(|(_, _, texts)| texts.clone())
                .unwrap_or_default(),
        }
    }
}

/// Occurrence-bound consent one consuming compile hands to `checking`: each
/// restricted build-host request the activation's admitted build machine
/// projects joins here — in issue order — before that request's own build
/// effect executes, so an absent or widened request never runs.
///
/// The binding is owned data (`Send` across the compile thread): it consults
/// the request's canonical acceptance-row meaning against the granted
/// meanings retained for this exact package identity and checked context.
/// A refusal fails the compile with the pending meaning attached, the same
/// record the post-pass join would surface.
pub struct CheckpointRestrictedBuildGrants {
    package: PackageKeyIdentity,
    context: PackageCheckedContext,
    request_path: Option<DependencyRequestPath>,
    granted: BTreeSet<String>,
}

impl compiler::RestrictedBuildGrants for CheckpointRestrictedBuildGrants {
    fn admit(
        &mut self,
        request: &build_evaluation::RestrictedBuildRequest,
    ) -> Result<(), Vec<Diagnostic>> {
        let meaning = package_evidence::encoding::restricted_build_request_acceptance_text(
            self.package,
            self.context.target(),
            request,
        )
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        if self.granted.contains(&meaning) {
            return Ok(());
        }
        Err(vec![Diagnostic::error(
            UngrantedRestrictedBuildRequest {
                package: self.package,
                context: self.context,
                request_meaning: meaning,
                request_path: self.request_path.clone(),
            }
            .to_string(),
        )])
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
    closure: &ResolvedPackageSourceClosure,
) -> Result<Vec<UngrantedRestrictedBuildRequest>, PackageLockError> {
    let _stage = timings::stage("restricted_build_grant_join");
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
                closure.dependency_path(review.key()),
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

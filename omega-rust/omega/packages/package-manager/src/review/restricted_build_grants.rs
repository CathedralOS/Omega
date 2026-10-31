//! The execution-time grant join between a fresh compile and the consuming
//! project's accepted lock policy
//! (wiki/spec/packages/acceptance.md#restricted-build-acceptance).
//!
//! Review compiles are observational: private issued snapshot/staging builds
//! run without restricted authority, while a non-exempt request stops before
//! execution and remains available for a decision. A lock that retains decisions is
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

use crate::Hex;
use crate::lock::{PackageCheckedContext, PackageLockTarget};
use crate::resolution::graph::DependencyRequestPath;

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
    /// Bind a pre-effect request to the exact requesting occurrence. Both a
    /// fresh pending observation and a consuming compile's consent refusal use
    /// the same canonical meaning; neither path grants execution.
    pub(crate) fn project(
        package: PackageKeyIdentity,
        context: PackageCheckedContext,
        request_path: Option<DependencyRequestPath>,
        request: &package_compilation::build_execution::RestrictedBuildRequest,
    ) -> Result<Self, package_evidence::encoding::PackageReviewEncodingError> {
        let request = request.clone().for_target(Some(context.target()));
        let request_meaning = package_evidence::encoding::restricted_build_request_acceptance_text(
            package,
            context.target(),
            &request,
        )?;
        Ok(Self {
            package,
            context,
            request_meaning,
            request_path,
        })
    }

    /// The complete normalized request meaning the accepted policy did not
    /// grant, in the same canonical row form the decision document renders.
    pub(crate) fn request_meaning(&self) -> &str {
        &self.request_meaning
    }
}

impl std::fmt::Display for UngrantedRestrictedBuildRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let identity = Hex(&self.package.digest()[..4]).to_string();
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
                write!(formatter, "{}", Hex(&path.root().identity().digest()))?;
                for step in path.steps() {
                    write!(formatter, " -> {:?} ", step.alias().as_str())?;
                    write!(formatter, "{}", Hex(&step.target().identity().digest()))?;
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
pub(crate) struct RestrictedBuildCheckpoint {
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
/// A refusal fails the compile with the pending meaning attached. A review
/// carried from the persisted cache ran no Build, so the candidate pass admits
/// its retained requests through this same binding.
pub(crate) struct CheckpointRestrictedBuildGrants {
    package: PackageKeyIdentity,
    context: PackageCheckedContext,
    request_path: Option<DependencyRequestPath>,
    granted: BTreeSet<String>,
}

impl package_compilation::build_execution::RestrictedBuildGrants
    for CheckpointRestrictedBuildGrants
{
    fn admit(
        &mut self,
        request: &package_compilation::build_execution::RestrictedBuildRequest,
    ) -> Result<(), Vec<Diagnostic>> {
        // The Build evaluates without a realized target; this checkpoint's
        // checked context names the target whose acceptance rows apply.
        let pending = UngrantedRestrictedBuildRequest::project(
            self.package,
            self.context,
            self.request_path.clone(),
            request,
        )
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        if self.granted.contains(pending.request_meaning()) {
            return Ok(());
        }
        Err(vec![Diagnostic::error(pending.to_string())])
    }
}

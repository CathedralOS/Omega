//! Exact installed selected-provider closure review for D45's implemented
//! compiler-intrinsic, normalized-foreign, direct-syscall, and bounded
//! checked-physical roles.

mod context;
mod operations;
mod reviewer;

use omega_abstract_operations::AbstractOperationPlan;
use omega_effects::{SelectedProviderPlanFacts, TerminalAuthorityClosureReviewReceipt};

use super::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};
use crate::realization::providers::AdmittedTerminalMechanism;
use context::ReviewContext;
use reviewer::Reviewer;

pub(crate) fn review_terminal_authority_closure(
    terminal_artifact_identity: [u8; 32],
    target_profile: omega_target::TargetProfile,
    plan: &AbstractOperationPlan,
    selected: &SelectedProviderPlanFacts,
    physical_policy: &TerminalAuthorityPolicy,
    permission_policy: &TerminalAuthorityPermissionPolicy,
    mechanisms: &[AdmittedTerminalMechanism],
    installed_candidates: &[psi_terminal::ProviderCandidateConformance],
) -> Result<TerminalAuthorityClosureReviewReceipt, String> {
    let context = ReviewContext::new(
        target_profile,
        plan,
        selected,
        physical_policy,
        permission_policy,
        mechanisms,
        installed_candidates,
    )?;
    let root_edges = context.reachable_authority_edges(plan.entry)?;
    if !root_edges.checked_physical.is_empty() {
        return Err(
            "root-reachable checked physical operation has no selected provider requirement custody"
                .into(),
        );
    }
    let mut reviewer = Reviewer::new(context);
    for boundary in root_edges.boundaries {
        reviewer.expand_boundary(boundary)?;
    }
    TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        terminal_artifact_identity,
        target_profile.native_target(),
        selected.identity_digest(),
        physical_policy.identity(),
        permission_policy.identity(),
        reviewer.into_leaves(),
    )
    .map_err(|error| format!("terminal-authority review receipt rejected: {error:?}"))
}

#[cfg(test)]
#[path = "terminal_authority_review/tests.rs"]
mod tests;

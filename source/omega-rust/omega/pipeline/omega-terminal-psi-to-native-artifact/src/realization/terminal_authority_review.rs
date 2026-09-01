//! Exact installed selected-provider closure review for D45's implemented
//! compiler-intrinsic and normalized-foreign terminal roles.

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
    target: omega_target::NativeTarget,
    plan: &AbstractOperationPlan,
    selected: &SelectedProviderPlanFacts,
    physical_policy: &TerminalAuthorityPolicy,
    permission_policy: &TerminalAuthorityPermissionPolicy,
    mechanisms: &[AdmittedTerminalMechanism],
    installed_candidates: &[psi_terminal::ProviderCandidateConformance],
) -> Result<TerminalAuthorityClosureReviewReceipt, String> {
    let context = ReviewContext::new(
        plan,
        selected,
        physical_policy,
        permission_policy,
        mechanisms,
        installed_candidates,
    )?;
    let root_boundaries = context.reachable_boundaries(plan.entry)?;
    let mut reviewer = Reviewer::new(context);
    for boundary in root_boundaries {
        reviewer.expand_boundary(boundary)?;
    }
    TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        terminal_artifact_identity,
        target,
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

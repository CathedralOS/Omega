//! Exact installed selected-provider closure review for D45's implemented
//! compiler-intrinsic, normalized-foreign, direct-syscall, and bounded
//! checked-physical roles.

mod context;
mod operations;
pub(crate) mod reachability;
mod reviewer;

use effects::{SelectedProviderPlanFacts, TerminalAuthorityClosureReviewReceipt};

use super::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};
use crate::provider_admission::AdmittedTerminalMechanism;
use context::ReviewContext;
use reviewer::Reviewer;

/// Review the selected-provider closure's terminal leaves. `permission_policy`
/// is the receiver-admission axis: `Some` adjudicates every leaf against one
/// exact supplied row and binds that policy identity into the receipt; `None`
/// retains exact mechanisms and any available classifications, without claiming
/// receiver admission. Unknown classifications are not empty dispositions.
pub(crate) fn review_terminal_authority_closure(
    terminal_artifact_identity: [u8; 32],
    target_profile: target::TargetProfile,
    input: &crate::VerifiedPsiOptimizationInput,
    selected: &SelectedProviderPlanFacts,
    physical_policy: &TerminalAuthorityPolicy,
    permission_policy: Option<&TerminalAuthorityPermissionPolicy>,
    mechanisms: &[AdmittedTerminalMechanism],
    installed_candidates: &[terminal_psi::ProviderCandidateConformance],
) -> Result<TerminalAuthorityClosureReviewReceipt, String> {
    let context = ReviewContext::new(
        target_profile,
        input,
        selected,
        physical_policy,
        permission_policy,
        mechanisms,
        installed_candidates,
    )?;
    let root_edges = context
        .graph
        .reachable_authority_edges(input.plan().entry)?;
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
    let (leaves, leaf_identities) = reviewer.into_leaves();
    TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        terminal_artifact_identity,
        target_profile.native_target(),
        selected.identity_digest(),
        physical_policy.identity(),
        permission_policy.map(|policy| policy.identity()),
        leaves,
        &leaf_identities,
    )
    .map_err(|error| format!("terminal-authority review receipt rejected: {error:?}"))
}

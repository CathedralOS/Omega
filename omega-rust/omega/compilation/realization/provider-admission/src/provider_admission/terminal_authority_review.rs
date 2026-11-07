//! Exact installed selected-provider closure review for D45's implemented
//! compiler-intrinsic, normalized-foreign, direct-syscall, and bounded
//! checked-physical roles.

mod context;
mod operations;
pub(crate) mod reachability;
mod reviewer;

// Receiver admission derives constrained file-handle occurrences from the
// retained executable graph. The permission table is an independent input.
#[cfg(test)]
mod windows_handle_activations;
mod windows_handle_aliases;
mod windows_handle_arguments;
mod windows_handle_calls;
#[cfg(test)]
mod windows_handle_conditions;
mod windows_handle_contracts;
mod windows_handle_inputs;
mod windows_handle_lifetimes;
mod windows_handle_origins;
#[cfg(test)]
mod windows_handle_paths;
mod windows_handle_replay;
mod windows_handle_scalar_table;
mod windows_handle_storage;
mod windows_handle_values;

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
    input: &crate::AdmissionSource<'_>,
    selected: &SelectedProviderPlanFacts,
    physical_policy: &TerminalAuthorityPolicy,
    permission_policy: Option<&TerminalAuthorityPermissionPolicy>,
    mechanisms: &[AdmittedTerminalMechanism],
    settlements: &[crate::AdmittedBoundarySettlement<'_>],
    installed_candidates: &[terminal_psi::ProviderCandidateConformance],
    components: &[crate::provider_admission::NativeProviderSettlement<'_>],
) -> Result<TerminalAuthorityClosureReviewReceipt, String> {
    let context = ReviewContext::new(
        target_profile,
        input,
        selected,
        physical_policy,
        permission_policy,
        mechanisms,
        settlements,
        installed_candidates,
        components,
    )?;
    let root_edges = context
        .graph
        .reachable_authority_edges(input.plan().entry)?;
    let mut reviewer = Reviewer::new(context, terminal_artifact_identity);
    for instruction in root_edges.checked_operations {
        reviewer.admit_program_root(instruction)?;
    }
    for boundary in root_edges.boundaries {
        reviewer.expand_boundary(boundary)?;
    }
    let (leaves, leaf_identities, physical_identity) = reviewer.into_leaves();
    TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        terminal_artifact_identity,
        target_profile.native_target(),
        selected.identity_digest(),
        physical_identity,
        permission_policy.map(|policy| policy.identity()),
        leaves,
        &leaf_identities,
    )
    .map_err(|error| format!("terminal-authority review receipt rejected: {error:?}"))
}

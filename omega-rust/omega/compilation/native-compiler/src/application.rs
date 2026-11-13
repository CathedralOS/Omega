// Assemble native realization custody from a produced Terminal application and
// admit package terminal-authority permission rows. `TerminalApplication::prepare`
// projects installed inputs, binds the physical entry adapter when requested,
// and attaches target provider proposals without consulting checked source.

use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy;
use terminal_psi_to_abstract_operations::{
    RetainedTerminalArtifact, TerminalNativeProposalRequest,
    project_terminal_native_realization_proposal,
};

pub struct TerminalApplication {
    pub artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub receipt: terminal_psi::CheckedProgramEntryTerminalReceipt,
    pub provider_proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    pub physical_entry_adapter: bool,
    /// The evidence seals of `artifact`, when the checker on the lowered
    /// form published it. They stay beside the artifact until Omega 00
    /// lowers it.
    pub evidence: Option<terminal_codec::EvidenceSeals>,
}

impl TerminalApplication {
    pub fn prepare(
        self,
        request: TerminalNativeProposalRequest<'_>,
    ) -> Result<RetainedTerminalArtifact, Vec<Diagnostic>> {
        let mut proposal =
            project_terminal_native_realization_proposal(&self.artifact, self.receipt, request)?;
        if self.physical_entry_adapter {
            proposal
                .bind_physical_adapter(&self.artifact)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
        Ok(
            RetainedTerminalArtifact::new_with_native_realization_proposal(
                self.artifact,
                proposal.with_target_provider_proposals(self.provider_proposals),
            )
            .with_evidence(self.evidence),
        )
    }
}

pub fn admit_package_permissions(
    permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    identities: terminal_psi::TerminalIdentityTable,
) -> Result<TerminalAuthorityPermissionPolicy, Vec<Diagnostic>> {
    use terminal_psi_to_abstract_operations::provider_admission::{
        terminal_authority_permission_policy_with_rows,
        terminal_authority_permissions::validate_package_terminal_authority_permission_custody,
    };
    validate_package_terminal_authority_permission_custody(permissions.iter(), &identities)?;
    terminal_authority_permission_policy_with_rows(permissions, identities).map_err(|error| {
        vec![Diagnostic::error(format!(
            "checked package terminal-authority permissions are invalid: {error:?}"
        ))]
    })
}

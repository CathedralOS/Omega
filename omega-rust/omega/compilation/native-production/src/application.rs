//! Assemble native realization custody from a produced Terminal application.
//!
//! The source producer supplies its exact entry receipt and closed Build
//! companion. This owner projects them against canonical Terminal, binds the
//! physical adapter when requested, and retains the result without source IR.
//! Preparation does not emit an image or authorize execution: artifact-only
//! publication and native compilation consume the same retained product.

use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_psi_to_abstract_operations::{
    RetainedTerminalArtifact, TerminalNativeProposalRequest,
    project_terminal_native_realization_proposal,
};

/// One application's portable code and source-issued realization inputs.
/// Code may be shared; the entry receipt and Build companion remain per use.
pub struct TerminalApplication {
    pub artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub receipt: terminal_psi::CheckedProgramEntryTerminalReceipt,
    pub provider_proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    /// Select the authored physical adapter, without granting entry authority.
    pub physical_entry_adapter: bool,
}

impl TerminalApplication {
    /// Project installed inputs, bind exact arrival custody, then retain the
    /// same application's provider proposals. No checked source is consulted.
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
            ),
        )
    }
}

/// Admit the package's independently accepted permission rows. Receiver policy
/// is a separate axis and cannot excuse duplicate or malformed package custody.
pub fn admit_package_permissions(
    permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    identities: terminal_psi::TerminalIdentityTable,
) -> Result<
    terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    Vec<Diagnostic>,
> {
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

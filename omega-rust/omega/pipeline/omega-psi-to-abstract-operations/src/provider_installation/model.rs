use crate::shared::*;

/// Bind Omega's provider policy only to exact rows preserved from the verified
/// terminal catalog. Psi independently replays artifact verification before it
/// returns the private-field installation carrier consumed by its interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderAdapter {
    pub requirement_identity: String,
    pub provider_identity: String,
    pub machine_identity: String,
}

/// One exact structural Unit boundary occurrence bound to the checked provider
/// row selected for its requirement. Private fields prevent target lowering
/// from reconstructing this authority from a candidate machine ID alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledProviderUnitCall {
    pub(crate) caller: MachineId,
    pub(crate) psi_operation: OperationId,
    pub(crate) boundary: psi_core::BoundaryMachineId,
    pub(crate) provider: ProviderCandidateConformance,
    pub(crate) scalar_arguments: Vec<psi_core::ValueId>,
    pub(crate) structural_arguments: Vec<StructuralArgument>,
    pub(crate) completion_claim_sources: Vec<CompletionClaimSource>,
    pub(crate) completion_receipts: Vec<CompletionReceipt>,
}

impl AdmittedInstalledProviderUnitCall {
    pub const fn caller(&self) -> MachineId {
        self.caller
    }

    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    pub const fn boundary(&self) -> psi_core::BoundaryMachineId {
        self.boundary
    }

    pub const fn provider(&self) -> &ProviderCandidateConformance {
        &self.provider
    }

    pub fn scalar_arguments(&self) -> &[psi_core::ValueId] {
        &self.scalar_arguments
    }

    pub fn structural_arguments(&self) -> &[StructuralArgument] {
        &self.structural_arguments
    }

    pub fn completion_claim_sources(&self) -> &[CompletionClaimSource] {
        &self.completion_claim_sources
    }

    pub fn completion_receipts(&self) -> &[CompletionReceipt] {
        &self.completion_receipts
    }
}

/// Omega-owned installation custody. The Psi carrier remains sealed and is
/// exposed only by reference for reference execution; physical consumers use
/// the fully replayed provider rows and call occurrences retained alongside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderInstallation {
    pub(crate) psi_installation: psi_terminal_interpreter::AdmittedProviderInstallation,
    pub(crate) psi: psi_terminal::TerminalPsiIdentity,
    pub(crate) installed_candidates: Vec<ProviderCandidateConformance>,
    pub(crate) installed_unit_calls: Vec<AdmittedInstalledProviderUnitCall>,
}

impl AdmittedProviderInstallation {
    pub const fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.psi
    }

    pub const fn psi_installation(
        &self,
    ) -> &psi_terminal_interpreter::AdmittedProviderInstallation {
        &self.psi_installation
    }

    pub fn installed_candidates(&self) -> &[ProviderCandidateConformance] {
        &self.installed_candidates
    }

    pub fn installed_unit_calls(&self) -> &[AdmittedInstalledProviderUnitCall] {
        &self.installed_unit_calls
    }
}

impl omega_installation_evidence::ProviderInstallationEvidence for AdmittedProviderInstallation {
    fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.psi
    }

    fn installed_provider_unit_calls(
        &self,
    ) -> Vec<omega_installation_evidence::InstalledProviderUnitCallEvidence> {
        self.installed_unit_calls
            .iter()
            .map(
                |call| omega_installation_evidence::InstalledProviderUnitCallEvidence {
                    caller: call.caller,
                    psi_operation: call.psi_operation,
                    boundary: call.boundary,
                    provider: call.provider.clone(),
                    scalar_arguments: call.scalar_arguments.clone(),
                    structural_arguments: call.structural_arguments.clone(),
                    completion_claim_sources: call
                        .completion_claim_sources
                        .iter()
                        .map(|source| {
                            omega_installation_evidence::InstalledProviderCompletionClaimSource {
                                claim: source.claim,
                                entry: source.entry.clone(),
                                content: source.content.clone(),
                            }
                        })
                        .collect(),
                    completion_receipts: call.completion_receipts.clone(),
                },
            )
            .collect()
    }
}

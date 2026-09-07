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

/// One exact boundary occurrence bound to the checked provider
/// row selected for its requirement. Private fields prevent target lowering
/// from reconstructing this authority from a candidate machine ID alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledProviderCall {
    pub(crate) caller: MachineId,
    pub(crate) psi_operation: OperationId,
    pub(crate) boundary: semantic_vocabulary::BoundaryMachineId,
    pub(crate) provider: ProviderCandidateConformance,
    pub(crate) result: terminal_psi::OperationResult,
    pub(crate) scalar_arguments: Vec<semantic_vocabulary::ValueId>,
    pub(crate) structural_arguments: Vec<StructuralArgument>,
    pub(crate) completion_claim_sources: Vec<CompletionClaimSource>,
    pub(crate) completion_receipts: Vec<CompletionReceipt>,
}

impl AdmittedInstalledProviderCall {
    pub const fn caller(&self) -> MachineId {
        self.caller
    }

    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    pub const fn boundary(&self) -> semantic_vocabulary::BoundaryMachineId {
        self.boundary
    }

    pub const fn provider(&self) -> &ProviderCandidateConformance {
        &self.provider
    }

    pub const fn result(&self) -> &terminal_psi::OperationResult {
        &self.result
    }

    pub fn scalar_arguments(&self) -> &[semantic_vocabulary::ValueId] {
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
    pub(crate) psi_installation: terminal_interpreter::AdmittedProviderInstallation,
    pub(crate) psi: terminal_psi::TerminalPsiIdentity,
    pub(crate) installed_candidates: Vec<ProviderCandidateConformance>,
    pub(crate) installed_calls: Vec<AdmittedInstalledProviderCall>,
}

impl AdmittedProviderInstallation {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    pub const fn psi_installation(&self) -> &terminal_interpreter::AdmittedProviderInstallation {
        &self.psi_installation
    }

    pub fn installed_candidates(&self) -> &[ProviderCandidateConformance] {
        &self.installed_candidates
    }

    pub fn installed_calls(&self) -> &[AdmittedInstalledProviderCall] {
        &self.installed_calls
    }
}

impl installation_evidence::ProviderInstallationEvidence for AdmittedProviderInstallation {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    fn installed_provider_calls(
        &self,
    ) -> Vec<installation_evidence::InstalledProviderCallEvidence> {
        self.installed_calls
            .iter()
            .map(
                |call| installation_evidence::InstalledProviderCallEvidence {
                    caller: call.caller,
                    psi_operation: call.psi_operation,
                    boundary: call.boundary,
                    provider: call.provider.clone(),
                    result: call.result.clone(),
                    scalar_arguments: call.scalar_arguments.clone(),
                    structural_arguments: call.structural_arguments.clone(),
                    completion_claim_sources: call
                        .completion_claim_sources
                        .iter()
                        .map(|source| {
                            installation_evidence::InstalledProviderCompletionClaimSource {
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

//! Identities, arguments and completion claims retained from an admitted provider call.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledAssumedBindingEvidence {
    pub required: semantic_vocabulary::BoundaryMachineId,
    pub supplied_requirement: semantic_vocabulary::BoundaryMachineId,
    pub implementation: semantic_vocabulary::MachineId,
    pub selected_requirement: semantic_vocabulary::BoundaryMachineId,
    pub supply_path: Vec<semantic_vocabulary::BoundaryMachineId>,
}

impl InstalledAssumedBindingEvidence {
    pub fn matches(
        &self,
        required: semantic_vocabulary::BoundaryMachineId,
        provider: &terminal_psi::ProviderCandidateConformance,
    ) -> bool {
        self.required == required
            && self.implementation == provider.candidate
            && self.selected_requirement == provider.boundary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderCallEvidence {
    pub caller: semantic_vocabulary::MachineId,
    pub invocation_site: semantic_vocabulary::InvocationSite,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub provider: terminal_psi::ProviderCandidateConformance,
    pub assumption: Option<InstalledAssumedBindingEvidence>,
    pub result: terminal_psi::OperationResult, // Exact caller-local result established by successful provider completion.
    pub scalar_arguments: Vec<semantic_vocabulary::ValueId>, // Ordered scalar values supplied by the admitted call occurrence.
    pub structural_arguments: Vec<terminal_psi::StructuralArgument>,
    pub completion_claim_sources: Vec<crate::completion_claims::CompletionClaimSource>,
    pub completion_receipts: Vec<terminal_psi::CompletionReceipt>,
}

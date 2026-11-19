use crate::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramInterfaces {
    pub services: Vec<ServiceDeclaration>,
    pub root_service_reach: TerminalRootServiceReach,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub calling_signatures: Vec<crate::CallingSignatureDeclaration>,
    pub calling_abi_signatures: Vec<crate::CallingAbiDeclaration>,
    pub calling_policy_requests: Vec<crate::CallingPolicyRequest>,
    pub service_schemas: Vec<crate::TerminalServiceSchema>,
    pub program_entry_contracts: Vec<crate::TerminalProgramEntryContract>,
    pub provider_selections: Vec<crate::TerminalProviderSelection>,
    pub external_providers: Vec<ExternalProviderConformance>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub provider_instance_requirements: Vec<ProviderInstanceRequirements>,
    pub boundary_application_candidates: Vec<crate::BoundaryApplicationCandidate>,
    pub callback_applications: Vec<crate::TerminalCallbackApplication>,
    pub installed_interrupt_registrations: Vec<crate::TerminalInstalledInterruptRegistration>,
    pub boundary_application_demands: Vec<TerminalBoundaryApplicationDemand>,
}

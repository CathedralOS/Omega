//! Physical adapter and checked semantic entry selected for one canonical program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntrySelection {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub adapter: semantic_vocabulary::MachineId,
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub inbound_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntryContext<S> {
    pub selection: S,
    pub target: target::NativeTarget,
    pub installed_reserve_bytes: u64,
    pub plan: calling_conventions::BoundaryEntryPlan,
    pub body_demand: machine_code::physical_stack::StackDemand,
    pub arrival_stack: target::TargetEntryStackGuarantee,
    pub requirements: PlannedRequirements,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRequirements {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub adapter: semantic_vocabulary::MachineId,
    pub contract: crate::ProgramEntryPhysicalContractPlan,
    pub stack: physical_recipes::physical_entry::PhysicalEntryStackRequirement,
    pub stack_bytes: u64,
    pub alignment: u64,
    pub minimum_stack_bytes: u64,
    pub installed_scopes: Vec<(semantic_vocabulary::MachineId, semantic_vocabulary::OperationId)>,
    pub installed_cpu: Option<physical_recipes::physical_entry::InstalledPhysicalCpuRequirement>,
}

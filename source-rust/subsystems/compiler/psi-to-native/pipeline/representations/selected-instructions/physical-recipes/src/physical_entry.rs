//! Checked stack-switch instructions and their physical-entry requirements.
use crate::{
    PhysicalRecipe,
    physical_stack_switch::{PhysicalIncomingStackRequirement, StackSwitchPlan},
};

pub type PhysicalEntryProgram<Origin, Context> =
    PhysicalRecipe<Origin, PhysicalEntryParts<Origin, Context>>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntryParts<Origin, Context> {
    pub context: Context,
    pub switch: StackSwitchPlan<Origin>,
}
impl<Origin, Context> PhysicalEntryProgram<Origin, Context> {
    pub fn into_parts(self) -> PhysicalEntryParts<Origin, Context> {
        self.instructions
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledPhysicalCpuRequirement {
    X86LongModeCpl0WithoutCet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalEntryStackRequirement {
    CheckedPrivateImage {
        incoming: PhysicalIncomingStackRequirement,
        minimum_private_stack_bytes: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntryContext<S, Contract> {
    pub selection: S,
    pub target: target::NativeTarget,
    pub installed_reserve_bytes: u64,
    pub plan: calling_conventions::BoundaryEntryPlan,
    pub body_demand: machine_code::physical_stack::StackDemand,
    pub arrival_stack: target::TargetEntryStackGuarantee,
    pub requirements: PlannedRequirements<Contract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRequirements<Contract> {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub adapter: semantic_vocabulary::MachineId,
    pub contract: Contract,
    pub stack: PhysicalEntryStackRequirement,
    pub stack_bytes: u64,
    pub alignment: u64,
    pub minimum_stack_bytes: u64,
    pub installed_scopes: Vec<(
        semantic_vocabulary::MachineId,
        semantic_vocabulary::OperationId,
    )>,
    pub installed_cpu: Option<InstalledPhysicalCpuRequirement>,
}

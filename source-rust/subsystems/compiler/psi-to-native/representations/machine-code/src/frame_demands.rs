//! Settled physical storage and preservation demands carried through encoding.
//! Frame checking needs these resources, not the instructions which caused them.
//! `plan` shares the machine plan so later records can be compared against it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFrameDemands {
    pub plan: std::sync::Arc<post_allocation_machine::PostAllocationMachinePlan>,
    pub target: target::NativeTarget,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub physical_register_model: register_model::PhysicalRegisterModelIdentity,
    pub functions: Vec<FunctionFrameDemands>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFrameDemands {
    pub machine: semantic_vocabulary::MachineId,
    pub contains_call: bool,
    pub writes_link_register: bool,
    pub stable_address_loans: Vec<selected_instructions::LocalStorageSlotId>,
    pub local_storage_slots: Vec<selected_instructions::SelectedLocalStorageSlot>,
    pub outgoing_arguments: Vec<selected_instructions::SelectedOutgoingArgumentSlot>,
}

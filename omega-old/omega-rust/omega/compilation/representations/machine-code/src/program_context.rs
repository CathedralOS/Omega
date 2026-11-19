//! Settled authority, target and identity data needed after physical encoding.
//! Executable physical instructions are absent; audits receive originals separately.
use std::sync::Arc;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedProgramContext {
    pub contracts: Arc<selected_instructions::SelectedProgramContracts>,
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub work_schedule: semantic_vocabulary::WorkScheduleIdentity,
    pub entry: semantic_vocabulary::MachineId,
    pub machine: Arc<crate::frame_demands::MachineFrameDemands>,
    pub effects: selected_instructions::PreAllocationMachineEffectIdentity,
    pub environment: register_environment::ValidatedTargetRegisterEnvironment,
    pub preservation: register_homes::AllocatedCalleeSavedRequirements,
    pub publication: Arc<register_homes::AllocationPublicationFacts>,
    pub phase: post_allocation_machine::PostAllocationMachinePhaseReceipt,
    pub identity: [u8; 32],
    pub checked_encoding: crate::SelectedFormEncodingIdentity,
}
impl EncodedProgramContext {
    pub fn physical(&self) -> &register_model::ValidatedPhysicalRegisterModel {
        self.environment.physical()
    }
}
impl std::ops::Deref for EncodedProgramContext {
    type Target = crate::frame_demands::MachineFrameDemands;
    fn deref(&self) -> &Self::Target {
        &self.machine
    }
}

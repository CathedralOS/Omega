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

/// Program identities and the frame plan carried from encoding to object verification.
/// The contract digest covers the storage geometry, ABI, publication and execution facts;
/// the frame is compared as the plan itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEncodingSubject {
    pub contracts: [u8; 32],
    pub target: target::NativeTarget,
    pub physical: [u8; 32],
    pub frame: Arc<crate::TargetFrameLayoutPlan>,
}

impl BoundEncodingSubject {
    pub fn from_encoded(
        encoded: &EncodedProgramContext,
        frame: Arc<crate::TargetFrameLayoutPlan>,
    ) -> Result<Self, &'static str> {
        let roots = encoded
            .contracts
            .execution()
            .ok_or("encoded program is missing its execution facts")?
            .roots();
        if roots.psi != encoded.psi
            || roots.entry != encoded.entry
            || roots.work_schedule != encoded.work_schedule
            || roots.pre_physical != encoded.publication.record().pre_physical
        {
            return Err("bound body contracts differ from the encoded program or its frame");
        }
        Ok(Self {
            contracts: encoded.contracts.identity_bytes(),
            target: encoded.target,
            physical: encoded.identity,
            frame,
        })
    }
}

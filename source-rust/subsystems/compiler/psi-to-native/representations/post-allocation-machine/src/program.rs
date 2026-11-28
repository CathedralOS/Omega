//! Physical forms retain the exact immutable instruction and function-contract
//! chunks they still use. The machine plan shares the final selected program
//! whose rows these are; no earlier program is retained.

use std::sync::Arc;

use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::AllocatedCalleeSavedRequirements;
use register_model::ValidatedPhysicalRegisterModel;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use terminal_psi::TerminalPsiIdentity;

use crate::PostAllocationMachinePlan;

/// The physical plan owns chosen alternatives and operands. Functions retain
/// unchanged operation/control chunks and the contracts needed to publish their
/// bytes. No consumer can recover an earlier program from this form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineProgram {
    pub contracts: Arc<selected_instructions::SelectedProgramContracts>,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub entry: MachineId,
    pub machine: Arc<PostAllocationMachinePlan>,
    pub environment: ValidatedTargetRegisterEnvironment,
    pub preservation: AllocatedCalleeSavedRequirements,
    pub publication: Arc<register_homes::AllocationPublicationFacts>,
    pub phase: crate::PostAllocationMachinePhaseReceipt,
    pub functions: selected_instructions::SelectedFunctions,
    pub identity: [u8; 32],
}

/// Unchanged function-owned contract and operation columns. Physical choices
/// live in the matching machine rows; sharing this chunk does not retain an
/// earlier program root or a producer-stage object.
pub type PhysicalFunction = selected_instructions::SelectedFunction;

impl PostAllocationMachineProgram {
    pub fn function(&self, machine: MachineId) -> Option<&PhysicalFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    pub fn physical(&self) -> &ValidatedPhysicalRegisterModel {
        self.environment.physical()
    }

    /// Bind the settled roots. Neither the machine plan, the function rows,
    /// the preservation requirements nor the allocation publication are
    /// hashed: every record that meets them shares them and compares them
    /// directly.
    pub fn recomputed_identity(&self) -> [u8; 32] {
        let mut digest =
            crate::digest_encoder::DomainDigestEncoder::new(b"omega.physical-machine-program.v5\0");
        digest.raw(self.psi.program_fingerprint.as_bytes());
        digest.raw(&self.psi.vocabulary_marker.get().to_le_bytes());
        digest.raw(&self.work_schedule.marker().to_le_bytes());
        digest.raw(&self.entry.get().to_le_bytes());
        digest.raw(&self.contracts.identity_bytes());
        digest.raw(&self.environment.identity().bytes());
        digest.raw(&self.physical().identity().bytes());
        digest.raw(&self.phase.complete_selection().bytes());
        digest.raw(&self.phase.phase_selections().bytes());
        digest.u64(self.functions.len() as u64);
        digest.finish()
    }
}

impl std::ops::Deref for PostAllocationMachineProgram {
    type Target = PostAllocationMachinePlan;

    fn deref(&self) -> &Self::Target {
        &self.machine
    }
}

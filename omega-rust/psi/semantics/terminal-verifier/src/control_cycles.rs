//! Verifier-derived runtime component topology and natural-rank questions.

mod reconstruction;
mod validation;

use proof_admission::{RecursiveComponentAcceptance, RecursiveComponentObligation};
use semantic_vocabulary::{BlockId, CycleComponentId, MachineId};

pub(crate) use reconstruction::reconstruct_validated_control_cycle_obligations;
pub use reconstruction::{
    control_cycle_identity, control_cycle_members, reconstruct_control_cycle_obligations,
};
pub(crate) use validation::validate_natural_cycles;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedControlCycleObligation {
    pub machine: MachineId,
    pub component: CycleComponentId,
    pub obligation: RecursiveComponentObligation<BlockId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedControlCycle {
    pub machine: MachineId,
    pub component: CycleComponentId,
    pub acceptance: RecursiveComponentAcceptance<BlockId>,
}

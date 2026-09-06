//! The current emitted machine-code program and its concept owners.
//!
//! Functions own executable bytes and their records. Calls, storage, control
//! flow, ownership, and boundaries describe the facts replay must check.
//! Unplaced function fragments retain their own identity before image assembly.

pub mod boundary;
pub mod calls;
pub mod control_flow;
pub mod encoding;
pub mod fragments;
pub mod functions;
pub mod instructions;
pub mod layout;
pub mod ownership;
pub mod provenance;
pub mod storage;

pub use boundary::*;
pub use calls::*;
pub use control_flow::*;
pub use encoding::*;
pub use fragments::*;
pub use functions::*;
pub use instructions::*;
pub use layout::text_section::*;
pub use layout::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedMachineLayout, ResolvedMachineProgram,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, ResolvedStructuralUnitCallLayout,
    ResolvedStructuralUnitFunctionLayout, SelectedFunctionLayoutPolicy,
    X86BranchRelaxationIdentity, resolved_machine_layout_identity,
};
pub use ownership::*;
pub use provenance::*;
pub use storage::*;

use semantic_vocabulary::MachineId;
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodePlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<MachineCodeFunction>,
}

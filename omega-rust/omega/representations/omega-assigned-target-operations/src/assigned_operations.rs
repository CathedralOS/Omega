//! Assigned target operations and their concrete physical homes.
//!
//! The root leads to function control, executable operations, value expressions,
//! call placements and storage. Cleanup and moves remain explicit Unit operations;
//! a register or frame location alone does not grant semantic ownership.

use omega_target::NativeTarget;
use psi_core::MachineId;
use psi_terminal::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<AssignedFunction>,
}

pub mod control_flow;
pub use control_flow::*;
pub mod calls;
pub use calls::*;
pub mod storage;
pub use storage::*;
pub mod values;
pub use values::*;
pub mod operations;
pub use operations::*;

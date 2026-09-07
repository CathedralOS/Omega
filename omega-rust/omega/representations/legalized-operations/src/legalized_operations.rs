//! Target-legal operations with explicit legality and semantic provenance.
//!
//! The program root retains distinct function/result shapes. Control flow,
//! values, calls and legality recipes own their fields beneath this root.
//! Identity encoding describes this representation; it is not a lowering pass.

use optimization_core::OptimizationUnitIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use sha2::{Digest, Sha256};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<LegalizedFunction>,
    /// Ordinary ordered scalar and Unit graphs with explicit ABI transport.
    pub scalar_functions: Vec<LegalizedScalarFunction>,
    /// Exact structural-call Unit functions. This roster is deliberately
    /// distinct from `scalar_functions` until the ordinary graph carries
    /// structural ABI placement and ownership transfer without erasure.
    pub structural_unit_functions: Vec<LegalizedStructuralUnitFunction>,
    /// Atomic result-bearing structural call/return closures. Instruction
    /// selection intentionally has no consumer for this roster yet.
    pub projected_structural_call_returns: Vec<LegalizedProjectedStructuralCallReturn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LegalizedOperationPlanIdentity([u8; 32]);

impl LegalizedOperationPlanIdentity {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

pub mod legality;
pub use legality::*;
pub mod calls;
pub use calls::*;
pub mod control_flow;
pub use control_flow::*;
pub mod values;
pub use values::*;

pub mod identity;
mod validation;
pub use identity::*;
pub use validation::LegalizedScalarCallShapeError;

#[cfg(test)]
mod tests;

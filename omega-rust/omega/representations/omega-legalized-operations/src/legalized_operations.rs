//! Target-legal operations with explicit legality and semantic provenance.
//!
//! The program root retains distinct function/result shapes. Control flow,
//! values, calls and legality recipes own their fields beneath this root.
//! Identity encoding describes this representation; it is not a lowering pass.

use omega_optimization_core::OptimizationUnitIdentity;
use omega_target::NativeTarget;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<LegalizedFunction>,
    /// Exact straight-line Unit functions admitted independently from the
    /// scalar conditional recipe inventory. Keeping this roster distinct
    /// prevents a value-less return from acquiring a fabricated scalar leaf.
    pub unit_functions: Vec<LegalizedUnitFunction>,
    /// Exact attached-Unit scalar-call bodies. This roster remains separate
    /// from plain Unit so calls, result homes, and ABI placement cannot be
    /// projected away by the value-less baseline.
    pub scalar_call_unit_functions: Vec<LegalizedScalarCallUnitFunction>,
    /// Exact structural-call Unit functions. This roster is deliberately
    /// distinct from `unit_functions`: accepting a structural signature in
    /// the value-less baseline would erase its ABI and ownership transfer.
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

#[cfg(test)]
mod tests;

//! Target-legal operations with explicit legality and semantic provenance.
//!
//! The program root retains ordinary instruction graphs and explicit structural
//! call shapes. Control flow, calls and legality own their fields beneath it.
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
    /// Ordinary ordered scalar and Unit graphs with explicit ABI transport.
    pub scalar_functions: Vec<LegalizedScalarFunction>,
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
pub use legality::LegalizedExactIntegerOperator;
pub mod calls;
pub use calls::LegalizedCallSourceError;
pub use calls::{
    LegalizedBoundarySettlement, LegalizedCallUnitParameter, LegalizedStructuralContract,
    NativeCallOrigin,
};
pub mod control_flow;
pub use control_flow::{
    LegalizedDescriptorOrigin, LegalizedNormalizedForeignCall, LegalizedRuntimeIndexOperand,
    LegalizedScalarArgument, LegalizedScalarBlock, LegalizedScalarCall, LegalizedScalarComparison,
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    LegalizedScalarParameter, LegalizedScalarReturn, LegalizedScalarReturnValue,
    LegalizedScalarSuccessor, LegalizedScalarTerminator, LegalizedStructuralCasePayload,
    LegalizedStructuralCaseSource, LegalizedStructuralCaseSuccessor, LegalizedValueDefinition,
    SaturatingCarrier, SaturatingOperation, TrappingForm, TrappingOperation,
};

pub mod identity;
mod validation;
pub use identity::{
    encode_call_binding, encode_call_plan, encode_fuel, encode_hosted_read_byte_identity,
    encode_integer, encode_locator, encode_machine_function_identity,
    encode_native_callback_argument, encode_placed_call_bindings, encode_placement,
    encode_provider_execution, encode_register, encode_scalar_type, encode_shape,
    legalized_operation_plan_identity,
};

#[cfg(test)]
mod tests;

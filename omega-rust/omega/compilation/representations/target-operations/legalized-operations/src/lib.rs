#![forbid(unsafe_code)]

/*
Target-legal operation plan representation for the Omega native backend (`Psi 06–08 -> Native`).

After target operations (`target-operations`) resolve calling policies, boundary bindings, and
structural storage shapes, legalization lowers abstract operations into target-legal scalar and
Unit instruction graphs (`LegalizedOperationPlan`). Each `LegalizedScalarFunction` carries its
evaluated `CallPlan`, explicit scalar and structural parameter contracts, ordered basic blocks of
target-legal instructions (`LegalizedScalarInstruction`), and explicit control-flow terminators
(`LegalizedScalarTerminator`) while retaining the exact semantic provenance, work settlements,
effect links, and ownership events required for independent stage verification.
*/

/*
Why legalized operations retain both semantic contracts and target placements:
Instruction selection and register allocation need concrete ABI placements, resolved byte offsets,
and explicit runtime index strides, whereas independent legalization replay and access-roster
verification must check that every target operation still honors its Terminal Psi source contract,
accepted obligation facts, and ownership frontier. Keeping the semantic contract (`LegalizedStructuralContract`,
`NativeCallOrigin`, `AcceptedObligationFactIdentity`, `OwnershipFrontierFact`) alongside the
target-legal instruction graph lets downstream verifiers audit the stage boundary without
re-running the legalization solver.
*/

/*
We group the target-legal representation into four subordinate owners beneath
`LegalizedOperationPlan`:
1. `legality` — checked exact arithmetic operators that retain their semantic integer carrier.
2. `calls` — structural function signatures, boundary settlements, and call-origin errors.
3. `control_flow` — saturating and trapping integer forms, scalar instruction graphs, and case
   dispatch edges.
4. `identity` and `validation` — canonical SHA-256 identity digests (`LegalizedOperationPlanIdentity`)
   and representation-owned call/instruction shape checks.
*/

use optimization_core::OptimizationUnitIdentity;
pub(crate) use optimization_core::codec_bytes;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use sha2::{Digest, Sha256};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[doc(hidden)]
pub use crate as legalized_operations;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub target: NativeTarget,
    /// Retained deployment, included in identity independently of native geometry.
    pub profile: Option<target::TargetProfile>,
    pub entry: MachineId,
    pub scalar_functions: Vec<LegalizedScalarFunction>, // Ordinary ordered scalar and Unit graphs with explicit ABI transport.
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

pub mod legality {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LegalizedExactIntegerOperator {
        Add,
        Subtract,
        Divide,
        Multiply,
        Remainder,
    }
}
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
    LegalizedScalarSuccessor, LegalizedScalarTerminator, LegalizedStructuralBinding,
    LegalizedStructuralCasePayload, LegalizedStructuralCaseSource,
    LegalizedStructuralCaseSuccessor, LegalizedValueDefinition, SaturatingCarrier,
    SaturatingOperation, TrappingForm, TrappingOperation,
};

pub mod identity;
mod validation;
pub use identity::{
    encode_access_call_bindings, encode_call_binding, encode_call_plan,
    encode_hosted_read_byte_identity, encode_integer, encode_locator,
    encode_machine_function_identity, encode_native_callback_argument, encode_placement,
    encode_provider_execution, encode_register, encode_scalar_type, encode_shape, encode_work,
    legalized_operation_plan_identity,
};

#[cfg(test)]
mod tests;

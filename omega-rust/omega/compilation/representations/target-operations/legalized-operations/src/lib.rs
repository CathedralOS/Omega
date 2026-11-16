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

pub(crate) use optimization_core::codec_bytes;
use optimization_core::{OptimizationUnitIdentity, OptimizationValidatorIdentity};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use sha2::{Digest, Sha256};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[doc(hidden)]
pub use crate as legalized_operations;

pub mod native_geometry;
pub mod structural_inputs;

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
    pub retained_integers: RetainedIntegerTypes,
    /// Interrupt installations that survived lowering, sorted and unique.
    /// These are what the driver must resolve (callback bodies, handler
    /// stacks) and bind before selection; the plan says which, the bound
    /// authority says with what.
    pub installation_operations: Vec<semantic_vocabulary::OperationId>,
}

impl LegalizedOperationPlan {
    /// The function selection starts from, with the calling plan its callers
    /// and the entry bridge must follow.
    pub fn entry_function(&self) -> Option<&LegalizedScalarFunction> {
        self.scalar_functions
            .iter()
            .find(|function| function.machine == self.entry)
    }
}

/// The execution plan stage 02 publishes for instruction selection.
///
/// A prepared plan says what must execute: concrete types, placements, proof
/// identities at the operations they license, and ownership evidence per
/// function. Selection decides how the target's instructions and registers
/// carry that out. The receipt names the plan by the hash of its contents, so
/// a holder can tell that the plan it selects from is the one that was sealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExecutionPlan {
    plan: LegalizedOperationPlan,
    receipt: PreparationReceipt,
}

impl PreparedExecutionPlan {
    pub fn seal(plan: LegalizedOperationPlan, validator: OptimizationValidatorIdentity) -> Self {
        let receipt = PreparationReceipt {
            identity: legalized_operation_plan_identity(&plan),
            validator,
            optimization_unit: plan.optimization_unit,
            work_schedule: plan.work_schedule,
            target: plan.target,
            function_count: plan.scalar_functions.len(),
        };
        Self { plan, receipt }
    }

    pub const fn plan(&self) -> &LegalizedOperationPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> PreparationReceipt {
        self.receipt
    }

    /// Whether the plan still hashes to the identity it was sealed under.
    pub fn is_intact(&self) -> bool {
        legalized_operation_plan_identity(&self.plan) == self.receipt.identity
            && self.receipt.optimization_unit == self.plan.optimization_unit
            && self.receipt.work_schedule == self.plan.work_schedule
            && self.receipt.target == self.plan.target
            && self.receipt.function_count == self.plan.scalar_functions.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparationReceipt {
    identity: LegalizedOperationPlanIdentity,
    validator: OptimizationValidatorIdentity,
    optimization_unit: OptimizationUnitIdentity,
    work_schedule: WorkScheduleIdentity,
    target: NativeTarget,
    function_count: usize,
}

impl PreparationReceipt {
    pub const fn identity(self) -> LegalizedOperationPlanIdentity {
        self.identity
    }

    pub const fn validator(self) -> OptimizationValidatorIdentity {
        self.validator
    }

    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn work_schedule(self) -> WorkScheduleIdentity {
        self.work_schedule
    }

    pub const fn target(self) -> NativeTarget {
        self.target
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }
}

/// Which integer types the accepted module retains.
///
/// Values, parameters and typed operations carry closed scalar types, so they
/// need no lookup. Structural declarations still state their leaf scalars in
/// the checking form; instruction selection closes one of those only if the
/// module the plan was prepared from retains it. Preparation copies that
/// membership here so selection needs the plan and nothing behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetainedIntegerTypes {
    /// Sorted and deduplicated, so membership is a binary search.
    Module(Vec<semantic_vocabulary::ClosedIntegerType>),
    /// The plan was prepared without an accepted module (unit fixtures and the
    /// module-less audit entry): every host-closable integer type is admitted.
    Unsealed,
}

impl RetainedIntegerTypes {
    pub fn from_module(
        retained: impl Iterator<Item = semantic_vocabulary::ClosedIntegerType>,
    ) -> Self {
        let mut retained = retained.collect::<Vec<_>>();
        retained.sort_unstable();
        retained.dedup();
        Self::Module(retained)
    }

    pub fn integer(
        &self,
        integer: semantic_vocabulary::IntegerType,
    ) -> Option<semantic_vocabulary::ClosedIntegerType> {
        let closed = semantic_vocabulary::ClosedIntegerType::try_from(integer).ok()?;
        match self {
            Self::Module(retained) => retained.binary_search(&closed).ok().map(|_| closed),
            Self::Unsealed => Some(closed),
        }
    }

    pub fn scalar(
        &self,
        scalar: semantic_vocabulary::ScalarType,
    ) -> Option<semantic_vocabulary::ClosedScalarType> {
        use semantic_vocabulary::{ClosedScalarType, ScalarType};
        Some(match scalar {
            ScalarType::Boolean => ClosedScalarType::Boolean,
            ScalarType::Integer(integer) => ClosedScalarType::Integer(self.integer(integer)?),
            ScalarType::IeeeFloat(format) => ClosedScalarType::IeeeFloat(format),
        })
    }
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
    LegalizedCasePointerSlot, LegalizedDescriptorOrigin, LegalizedNormalizedForeignCall,
    LegalizedRuntimeIndexOperand, LegalizedScalarArgument, LegalizedScalarBlock,
    LegalizedScalarCall, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarParameter,
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator, LegalizedStructuralBinding, LegalizedStructuralCasePayload,
    LegalizedStructuralCaseSource, LegalizedStructuralCaseSuccessor, LegalizedValueDefinition,
    SaturatingCarrier, SaturatingOperation, TrappingForm, TrappingOperation,
};

pub mod identity;
mod validation;
pub use identity::{
    encode_access_call_bindings, encode_call_binding, encode_call_plan, encode_external_execution,
    encode_hosted_read_byte_identity, encode_integer, encode_locator,
    encode_machine_function_identity, encode_native_callback_argument, encode_placement,
    encode_provider_execution, encode_register, encode_scalar_type, encode_shape, encode_work,
    legalized_operation_plan_identity,
};

#[cfg(test)]
mod tests;

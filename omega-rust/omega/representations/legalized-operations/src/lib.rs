#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Target-legal program representation.
//!
//! Start at [`legalized_operations::LegalizedOperationPlan`]. Control flow,
//! calls and legality are subordinate representation owners.

pub mod legalized_operations;
pub use legalized_operations::{
    LegalizedBoundarySettlement, LegalizedCallSourceError, LegalizedCallUnitParameter,
    LegalizedDynamicParameterCall, LegalizedDynamicParameterCallShapeError,
    LegalizedExactIntegerOperator, LegalizedNormalizedForeignCall, LegalizedOperationPlan,
    LegalizedOperationPlanIdentity, LegalizedRuntimeIndexOperand, LegalizedScalarArgument,
    LegalizedScalarBlock, LegalizedScalarCall, LegalizedScalarCallShapeError,
    LegalizedScalarComparison, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, LegalizedScalarParameter, LegalizedScalarReturn,
    LegalizedScalarReturnValue, LegalizedScalarSuccessor, LegalizedScalarTerminator,
    LegalizedStructuralCasePayload, LegalizedStructuralCaseSource,
    LegalizedStructuralCaseSuccessor, LegalizedStructuralContract, LegalizedValueDefinition,
    NativeCallOrigin, SaturatingCarrier, SaturatingOperation, calls, control_flow,
    encode_hosted_read_byte_identity, identity, legality, legalized_operation_plan_identity,
};

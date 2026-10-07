#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Target-legal program representation.
//!
//! Start at [`legalized_operations::LegalizedOperationPlan`]. Control flow,
//! calls and legality are subordinate representation owners.

pub mod legalized_operations;
pub use legalized_operations::{
    LegalizedBoundarySettlement, LegalizedCallSourceError, LegalizedCallUnitParameter,
    LegalizedDescriptorOrigin, LegalizedExactIntegerOperator, LegalizedNormalizedForeignCall,
    LegalizedOperationPlan, LegalizedOperationPlanIdentity, LegalizedRuntimeIndexOperand,
    LegalizedScalarArgument, LegalizedScalarBlock, LegalizedScalarCall,
    LegalizedScalarCallShapeError, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarParameter,
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator, LegalizedStructuralCasePayload, LegalizedStructuralCaseSource,
    LegalizedStructuralCaseSuccessor, LegalizedStructuralContract, LegalizedValueDefinition,
    NativeCallOrigin, SaturatingCarrier, SaturatingOperation, TrappingForm, TrappingOperation,
    calls, control_flow, encode_call_binding, encode_call_plan, encode_callback_materialization,
    encode_eightbyte_class, encode_hosted_read_byte_identity, encode_indirect_pointer,
    encode_machine_function_identity, encode_native_place, encode_placement,
    encode_provider_execution, encode_register, encode_scalar_type, encode_shape, identity,
    legality, legalized_operation_plan_identity,
};

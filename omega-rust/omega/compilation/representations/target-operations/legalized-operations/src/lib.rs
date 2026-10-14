#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Target-legal program representation.
//!
//! Start at [`legalized_operations::LegalizedOperationPlan`]. Control flow,
//! calls and legality are subordinate representation owners.

pub(crate) use optimization_core::codec_bytes;

pub mod legalized_operations;
pub(crate) use legalized_operations::LegalizedCallSourceError;
pub use legalized_operations::{
    LegalizedBoundarySettlement, LegalizedCallUnitParameter, LegalizedDescriptorOrigin,
    LegalizedExactIntegerOperator, LegalizedNormalizedForeignCall, LegalizedOperationPlan,
    LegalizedOperationPlanIdentity, LegalizedRuntimeIndexOperand, LegalizedScalarArgument,
    LegalizedScalarBlock, LegalizedScalarCall, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarParameter,
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator, LegalizedStructuralCasePayload, LegalizedStructuralCaseSource,
    LegalizedStructuralCaseSuccessor, LegalizedStructuralContract, LegalizedValueDefinition,
    NativeCallOrigin, SaturatingCarrier, SaturatingOperation, TrappingForm, TrappingOperation,
    calls, control_flow, encode_call_binding, encode_call_plan, encode_fuel,
    encode_hosted_read_byte_identity, encode_integer, encode_locator,
    encode_machine_function_identity, encode_native_callback_argument, encode_placed_call_bindings,
    encode_placement, encode_provider_execution, encode_register, encode_scalar_type, encode_shape,
    identity, legality, legalized_operation_plan_identity,
};

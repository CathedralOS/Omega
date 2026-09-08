//! Integer expression vocabulary and exact proof-bearing arithmetic.

use crate::{ScalarParameterLocation, TargetByteView, TargetCallArgument};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{
    IntegerType, IntegerValue, MachineId, OperationId, PlaceId, StructuralFieldId, ValueId,
};
use terminal_psi::CrashRouteBucket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerExpression {
    /// Read one exact earlier scalar definition without repeating its producer.
    ScalarHome(crate::TargetUnitScalarHomeRequirement),
    StructuralCall {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TargetCallArgument>,
        structural_arguments: Vec<crate::TargetStructuralArgument>,
        call_plan: calling_conventions::CallPlan,
        claim_transfers: Vec<terminal_psi::ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Exact `u8` observation. The view retains its parameter placement or
    /// checked derivation. The index expression retains its Terminal
    /// identity; length and obligation retain proof custody, not a new check.
    ByteSequenceRead {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetByteView>,
        index: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ByteSequenceLength {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetByteView>,
        length_byte_offset: u32,
    },
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TargetCallArgument>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    StructuralField {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        field: StructuralFieldId,
        source_placement: ValuePlacement,
        field_byte_offset: u32,
        integer_type: IntegerType,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    WrappingShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    ExactShiftLeft {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactAdd {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingAdd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingSubtract {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactSubtract {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingSubtract {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingMultiply {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactMultiply {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
}

//! values integer in the assigned operations program.

use crate::AssignedCallArgument;
use crate::AssignedScalarLocation;
use calling_conventions::ValuePlacement;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::StructuralFieldId;
use semantic_vocabulary::ValueId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedIntegerExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: semantic_vocabulary::MachineId,
        arguments: Vec<AssignedCallArgument>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<terminal_psi::CrashRouteBucket>,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
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
        operand: Box<AssignedIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<AssignedIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        source_type: IntegerType,
        operand: Box<AssignedIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    WrappingShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    ExactShiftLeft {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactAdd {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingAdd {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingSubtract {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactSubtract {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingSubtract {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingMultiply {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactMultiply {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
}

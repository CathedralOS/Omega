use omega_calling_conventions::ValuePlacement;
use psi_core::{
    ClaimId, EdgeId, IntegerType, IntegerValue, OperationId, PlaceId, StructuralFieldId, ValueId,
};
use psi_terminal::{CrashCause, CrashPredicateTerm};

use crate::{
    AssignedBooleanExpression, AssignedCallArgument, AssignedScalarLocation, ExpressionFrame,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<AssignedIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedIntegerExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: psi_core::MachineId,
        arguments: Vec<AssignedCallArgument>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<psi_terminal::CrashRouteBucket>,
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
        obligation: psi_core::ObligationId,
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
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
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
        obligation: psi_core::ObligationId,
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
        obligation: psi_core::ObligationId,
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
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
}

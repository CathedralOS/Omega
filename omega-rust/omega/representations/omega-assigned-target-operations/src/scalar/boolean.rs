use omega_calling_conventions::ValuePlacement;
use psi_core::{ClaimId, EdgeId, IntegerType, OperationId, PlaceId, StructuralFieldId, ValueId};
use psi_terminal::{CrashCause, CrashPredicateTerm, CrashRouteBucket};

use crate::{
    AssignedCallArgument, AssignedIntegerExpression, AssignedScalarLocation, ExpressionFrame,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedBooleanExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: psi_core::MachineId,
        arguments: Vec<AssignedCallArgument>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    Immediate {
        source_value: ValueId,
        value: bool,
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
    },
    Not {
        psi_operation: OperationId,
        operand: Box<AssignedBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<AssignedBooleanExpression>,
        right: Box<AssignedBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<AssignedBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedBooleanControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    ReturnImmediate {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    ReturnParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
}

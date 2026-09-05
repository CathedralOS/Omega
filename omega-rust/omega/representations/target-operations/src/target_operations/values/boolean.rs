//! Boolean expressions with their retained semantic operations.

use crate::{ScalarParameterLocation, TargetCallArgument, TargetIntegerExpression};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{
    IntegerType, MachineId, OperationId, PlaceId, StructuralFieldId, ValueId,
};
use terminal_psi::CrashRouteBucket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanExpression {
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
        value: bool,
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
    },
    Not {
        psi_operation: OperationId,
        operand: Box<TargetBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<TargetBooleanExpression>,
        right: Box<TargetBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
}

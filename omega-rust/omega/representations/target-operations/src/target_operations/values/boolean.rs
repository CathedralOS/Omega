//! Boolean expressions with their retained semantic operations.

use crate::{ScalarParameterLocation, TargetIntegerExpression};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{IntegerType, OperationId, PlaceId, StructuralFieldId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanExpression {
    /// Read the exact value supplied on entry to its declaring block.
    BlockParameter(crate::TargetScalarBlockValue),
    /// Read an already executed scalar definition; physical residence is downstream.
    ScalarHome(crate::TargetUnitScalarHomeRequirement),
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

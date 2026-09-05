//! values boolean in the assigned operations program.

use crate::AssignedCallArgument;
use crate::AssignedIntegerExpression;
use crate::AssignedScalarLocation;
use calling_conventions::ValuePlacement;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::StructuralFieldId;
use semantic_vocabulary::ValueId;
use terminal_psi::CrashRouteBucket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedBooleanExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: semantic_vocabulary::MachineId,
        arguments: Vec<AssignedCallArgument>,
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

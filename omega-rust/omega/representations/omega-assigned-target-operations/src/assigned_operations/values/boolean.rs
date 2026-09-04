//! values boolean in the assigned operations program.

use crate::AssignedCallArgument;
use crate::AssignedIntegerExpression;
use crate::AssignedScalarLocation;
use omega_calling_conventions::ValuePlacement;
use psi_core::IntegerType;
use psi_core::OperationId;
use psi_core::PlaceId;
use psi_core::StructuralFieldId;
use psi_core::ValueId;
use psi_terminal::CrashRouteBucket;

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

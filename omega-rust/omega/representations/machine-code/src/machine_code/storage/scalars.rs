//! Durable scalar homes and zero-code value establishment.

use calling_conventions::ValueShape;
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};

/// One durable scalar home in an attached Unit frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitScalarHomeRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitIntegerConstantRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: IntegerType,
    pub value: IntegerValue,
    pub operation_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAffineScalarRecordEstablishmentRecord {
    pub psi_operation: OperationId,
    pub result: terminal_psi::StructuralOperationResult,
    pub field: semantic_vocabulary::StructuralFieldId,
    pub value: IntegerValue,
    pub shape: ValueShape,
    pub operation_ordinal: usize,
}

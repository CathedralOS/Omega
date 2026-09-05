//! Emitted writes and the exact sources, places, and intervals they bind.

use crate::{
    InternalUnitScalarArgumentSourceRecord, UnitScalarHomeRecord, UnitScalarParameterLocationRecord,
};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{
    IeeeFloatValue, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use terminal_psi::StructuralPathSegment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitStructuralScalarFieldStoreRecord {
    pub psi_operation: OperationId,
    pub destination: terminal_psi::StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: semantic_vocabulary::StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub parameter_home_byte_offset: u32,
    pub parameter_home_indirect: bool,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitWriteOnlyPrimitiveStoreSourceRecord {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
        location: UnitScalarParameterLocationRecord,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
        definition_ordinal: usize,
    },
    IeeeFloatImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: IeeeFloatValue,
        definition_ordinal: usize,
    },
    Home(UnitScalarHomeRecord),
}

impl UnitWriteOnlyPrimitiveStoreSourceRecord {
    pub const fn defining_operation(&self) -> Option<OperationId> {
        match self {
            Self::Parameter { .. } => None,
            Self::IntegerImmediate {
                defining_operation, ..
            }
            | Self::BooleanImmediate {
                defining_operation, ..
            }
            | Self::IeeeFloatImmediate {
                defining_operation, ..
            } => Some(*defining_operation),
            Self::Home(home) => Some(home.defining_operation),
        }
    }

    pub const fn source_value(&self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. }
            | Self::IntegerImmediate { source_value, .. }
            | Self::BooleanImmediate { source_value, .. }
            | Self::IeeeFloatImmediate { source_value, .. } => *source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => *scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(*scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::IeeeFloatImmediate { value, .. } => ScalarType::IeeeFloat(value.format()),
            Self::Home(home) => home.scalar_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitWriteOnlyPrimitiveStoreRecord {
    pub psi_operation: OperationId,
    pub destination: terminal_psi::StructuralParameterDeclaration,
    pub destination_type: terminal_psi::StructuralTypeDeclaration,
    pub destination_placement: ValuePlacement,
    pub source: UnitWriteOnlyPrimitiveStoreSourceRecord,
    pub parameter_home_byte_offset: u32,
    pub parameter_home_indirect: bool,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarStructuralScalarFieldStoreRecord {
    pub psi_operation: OperationId,
    pub destination: terminal_psi::StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: semantic_vocabulary::StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub immediate: target_operations::TargetScalarImmediate,
    pub return_operation: OperationId,
    pub return_source_value: ValueId,
    pub return_field: semantic_vocabulary::StructuralFieldId,
    pub return_field_byte_offset: u32,
    pub return_scalar_type: ScalarType,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

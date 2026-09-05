//! Exact scalar and structural argument sources and their physical copies.

use crate::{UnitScalarHomeRecord, UnitScalarParameterLocationRecord};
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{
    IntegerType, IntegerValue, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::StructuralPathSegment;

/// Exact occurrence-specific source and ABI custody for one evaluated foreign-
/// call scalar value. The byte interval names only its register
/// materialization; the unresolved call field remains owned by
/// [`crate::ForeignCallRelocation::offset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallScalarArgumentRecord {
    pub parameter_index: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub placement: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitCallArgumentRecord {
    pub place: PlaceId,
    pub access: terminal_psi::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub source_home_byte_offset: u32,
    pub call_stack_bytes: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
    /// Immutable target bytes that realize this exact source-to-destination copy.
    pub bytes: Vec<u8>,
}

/// Exact semantic and physical source of one attached-Unit scalar argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalUnitScalarArgumentSourceRecord {
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
    Home(UnitScalarHomeRecord),
}

impl InternalUnitScalarArgumentSourceRecord {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. } => source_value,
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::BooleanImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::Home(home) => home.scalar_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitScalarCallArgumentRecord {
    pub parameter_index: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub destination: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
}

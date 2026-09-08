//! Exact scalar and structural argument sources and their physical copies.

use crate::{UnitScalarHomeRecord, UnitScalarParameterLocationRecord};
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{
    IntegerType, IntegerValue, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::StructuralPathSegment;

/// Exact occurrence-specific source and ABI custody for one evaluated foreign-
/// call scalar value. For ordinary calls the byte interval names register
/// materialization; the unresolved call field belongs to
/// [`crate::ForeignCallRelocation::offset`]. A selected boundary source instead
/// names the complete independently decoded builtin instruction sequence.
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
    pub source_location: crate::StructuralSourceLocation,
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
    /// One ordered selected call operand. Its argument span is the actual
    /// call instruction; retained physical replay owns preceding SSA transport.
    /// The enclosing parameter index identifies the exact operand position.
    SelectedCall {
        source_value: ValueId,
        scalar_type: ScalarType,
        instruction: selected_instructions::SelectedInstructionId,
    },
    /// One selected boundary pseudo consumes this SSA value from the recorded
    /// physical register. Its argument span is the whole pseudo, not a legacy
    /// materialization prefix; scratch is an operation-owned frame byte.
    SelectedBoundary {
        source_value: ValueId,
        scalar_type: ScalarType,
        instruction: selected_instructions::SelectedInstructionId,
        scratch_byte_offset: u64,
    },
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
            Self::SelectedBoundary { source_value, .. }
            | Self::SelectedCall { source_value, .. } => source_value,
            Self::Parameter { source_value, .. } => source_value,
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::BooleanImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::SelectedBoundary { scalar_type, .. } | Self::SelectedCall { scalar_type, .. } => {
                scalar_type
            }
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

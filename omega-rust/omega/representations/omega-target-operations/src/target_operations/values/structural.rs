//! Structural places, projected arguments, and field replacement facts.

use crate::TargetScalarImmediate;
use omega_calling_conventions::{ValuePlacement, ValueShape};
use psi_core::{OperationId, PlaceId, StructuralFieldId, StructuralTypeId, ValueId};
use psi_terminal::{StructuralParameterDeclaration, StructuralPathSegment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralParameter {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub access: psi_terminal::StructuralAccess,
    /// Exact authored path qualification roster. Target lowering transports
    /// this authority; it must never infer rows from layout or carrier shape.
    pub projected_qualifications: Vec<psi_terminal::StructuralPathQualification>,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralArgument {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
    /// Exact source-relative semantic projection retained through native and
    /// installed-artifact custody.
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    /// Checked byte offset of this projected value within `source`.
    pub source_byte_offset: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
}

/// One exact immediate scalar replacement performed before a scalar return.
///
/// This carrier is deliberately separate from [`crate::TargetUnitOperation`]: a
/// scalar function borrows its structural receiver through the ordinary scalar
/// ABI and does not materialize an attached-Unit parameter frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetScalarStructuralFieldStore {
    pub psi_operation: OperationId,
    pub destination: StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub immediate: TargetScalarImmediate,
}

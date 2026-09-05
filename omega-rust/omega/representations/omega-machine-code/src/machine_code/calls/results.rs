//! Shared scalar-result homes and structural-return ownership evidence.

use crate::UnitScalarHomeRecord;
use omega_calling_conventions::{ValuePlacement, ValueShape};
use psi_core::{ClaimId, EdgeId, OperationId, PlaceId};
use psi_terminal::{
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration,
};

/// Exact source-free custody for one normalized foreign scalar result.
///
/// The byte interval begins after the native call and any outbound-stack
/// release. It covers only canonical result normalization and the durable-home
/// store, never the mutable import relocation field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallScalarResultRecord {
    pub home: UnitScalarHomeRecord,
    pub source: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralReturnRecord {
    pub psi_edge: EdgeId,
    /// Ordered fixed-integer prefix preceding the structural input roster.
    /// Empty for the established claim-bearing structural-only family.
    pub scalar_parameters: Vec<omega_target_operations::FixedIntegerScalarAbiValue>,
    /// Complete ordered structural input signature. This binds the returned
    /// place and every zero-code cleanup place to its exact type/multiplicity.
    pub parameters: Vec<StructuralParameterDeclaration>,
    /// Structural ABI placements corresponding one-for-one with `parameters`;
    /// scalar-prefix placements live in `scalar_parameters`.
    pub parameter_placements: Vec<ValuePlacement>,
    pub source: StructuralParameterDeclaration,
    pub result: StructuralResultDeclaration,
    pub shape: ValueShape,
    pub source_placement: ValuePlacement,
    pub result_placement: ValuePlacement,
    pub returned_claims: Vec<ClaimId>,
    /// Typed no-ABI local declarations. These never receive a placement and
    /// therefore cannot silently become runtime inputs.
    pub trivial_affine_locals: Vec<(
        OperationId,
        StructuralPlaceDeclaration,
        StructuralTypeDeclaration,
    )>,
    /// Exact verifier-owned reverse-declaration no-code cleanup order.
    pub trivial_affine_discards: Vec<PlaceId>,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Compatibility name for the shared scalar-result custody record. Internal
/// and normalized foreign calls deliberately use one result/home vocabulary.
pub type InternalUnitScalarCallResultRecord = ForeignCallScalarResultRecord;

//! Immutable byte-view observation source contracts.

use std::collections::BTreeMap;

use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::OptimizationUnitValidationError;

pub(super) fn validate_immutable_byte_view_source(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    source: PlaceId,
    source_kind: Option<&StructuralPlaceKind>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let structural_type = match source_kind {
        Some(StructuralPlaceKind::Parameter { .. }) => function
            .structural_parameters
            .iter()
            .find(|parameter| {
                parameter.place == source
                    && parameter.access == StructuralAccess::SharedBorrow
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
            .map(|parameter| parameter.structural_type),
        Some(StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }) => Some(*structural_type),
        _ => None,
    };
    let valid = structural_type
        .and_then(|identity| structural_types.get(&identity))
        .is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
            )
        })
        && function
            .entry_claim_declarations
            .iter()
            .all(|claim| claim.input != source)
        && function
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != source);
    if !valid {
        return Err(OptimizationUnitValidationError::InvalidByteSequenceLength {
            machine: function.machine,
            block,
            node,
        });
    }
    Ok(())
}

//! Immutable byte-view observation source contracts.

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::OptimizationUnitValidationError;

pub(super) fn validate_immutable_byte_view_source(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    operation: &AbstractOperation,
    source_kind: Option<&StructuralPlaceKind>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let (source, length) = match operation {
        AbstractOperation::ByteSequenceLength { source, .. } => (*source, None),
        AbstractOperation::ByteSequenceRead { source, length, .. } => (*source, Some(*length)),
        _ => return Ok(()),
    };
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
    let exact_length = length.is_none_or(|length| {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(&node.operation, AbstractOperation::ByteSequenceLength {
                source: measured, result, ..
            } if *measured == source && result.value == length)
            })
    });
    // Scalar-use validation independently requires the exact length definition
    // to dominate this read. Equal integers and incoming aliases are not witnesses.
    if !valid || !exact_length {
        return Err(if length.is_some() {
            OptimizationUnitValidationError::InvalidByteSequenceRead {
                machine: function.machine,
                block,
                node,
            }
        } else {
            OptimizationUnitValidationError::InvalidByteSequenceLength {
                machine: function.machine,
                block,
                node,
            }
        });
    }
    Ok(())
}

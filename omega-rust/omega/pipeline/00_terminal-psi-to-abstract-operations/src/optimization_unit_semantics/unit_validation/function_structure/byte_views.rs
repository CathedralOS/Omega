//! Exact byte-view observation and mutation source contracts.

use std::collections::BTreeMap;

use crate::abstract_operations::AbstractOperation;
use crate::optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::optimization_unit_semantics::OptimizationUnitValidationError;

/// Resolve the bounded field from the exact writable destination declaration.
/// Replacement source validity and indexed-store freshness are checked separately.
pub(crate) fn byte_field_store_capacity(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<u64> {
    let (destination, path, field) = match operation {
        AbstractOperation::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } if destination != source => (destination, path, field),
        AbstractOperation::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => (destination, path, field),
        _ => return None,
    };
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)?;
    if !matches!(
        parameter.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == *destination)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *destination)
        || !function.structural_places.iter().any(|place| {
            place.id == *destination
                && place.kind
                    == StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    }
        })
    {
        return None;
    }
    let parent = super::super::structural_catalog::resolve_structural_path(
        structural_types,
        parameter.structural_type,
        path,
    )?;
    let StructuralTypeShape::Record { fields } = &structural_types.get(&parent)?.shape else {
        return None;
    };
    let selected = fields
        .iter()
        .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())?;
    match selected.field_type {
        terminal_psi::StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
            capacity,
        }) => Some(capacity),
        _ => None,
    }
}

pub(super) fn validate_byte_view_source(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    operation: &AbstractOperation,
    source_kind: Option<&StructuralPlaceKind>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let (source, length) = match operation {
        AbstractOperation::ByteSequenceWrite {
            destination,
            length,
            ..
        } => (*destination, Some(*length)),
        AbstractOperation::ByteSequenceLength { source, .. } => (*source, None),
        AbstractOperation::StructuralByteSequenceFieldStore { source, length, .. }
        | AbstractOperation::ByteSequenceRead { source, length, .. }
        | AbstractOperation::ByteSequenceSubslice { source, length, .. } => {
            (*source, Some(*length))
        }
        _ => return Ok(()),
    };
    let permitted_access = |access| match operation {
        AbstractOperation::ByteSequenceWrite { .. } => access == StructuralAccess::MutableBorrow,
        AbstractOperation::ByteSequenceLength { .. } => matches!(
            access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        ),
        _ => access == StructuralAccess::SharedBorrow,
    };
    let structural_type = match source_kind {
        Some(StructuralPlaceKind::BlockParameter {
            block: owner,
            position,
        }) => function
            .blocks
            .iter()
            .find(|block| block.id == *owner)
            .and_then(|block| block.structural_parameters.get(*position as usize))
            .filter(|parameter| {
                parameter.place == source
                    && permitted_access(parameter.access)
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
            .map(|parameter| parameter.structural_type),
        Some(StructuralPlaceKind::Parameter { .. }) => function
            .structural_parameters
            .iter()
            .find(|parameter| {
                parameter.place == source
                    && permitted_access(parameter.access)
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
            .map(|parameter| parameter.structural_type),
        Some(StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }) => Some(*structural_type),
        Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }) => function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find_map(|node| match &node.operation {
                AbstractOperation::ByteSequenceSubslice {
                    psi_operation,
                    result,
                    ..
                } if psi_operation == producer
                    && result.place == source
                    && result.structural_type == *structural_type
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty() =>
                {
                    Some(result.structural_type)
                }
                _ => None,
            }),
        _ => None,
    };
    let valid = structural_type
        .and_then(|identity| structural_types.get(&identity))
        .is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
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
    let valid_result = match operation {
        AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            result,
            ..
        } => {
            result.place != source
                && Some(result.structural_type) == structural_type
                && result.multiplicity == StructuralMultiplicity::Unrestricted
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && function.structural_places.iter().any(|place| {
                    place.id == result.place
                        && place.kind
                            == StructuralPlaceKind::OperationResult {
                                producer: *psi_operation,
                                structural_type: result.structural_type,
                            }
                })
                && function
                    .entry_claim_declarations
                    .iter()
                    .all(|claim| claim.input != result.place)
                && function
                    .content_entry_claims
                    .iter()
                    .all(|claim| claim.input.root != result.place)
        }
        _ => true,
    };
    // Scalar-use validation independently requires the exact length definition
    // and endpoints to dominate the operation. Structural availability checks
    // require a subslice producer to dominate every descriptor use.
    let writable = !matches!(operation, AbstractOperation::ByteSequenceWrite { .. })
        || matches!(
            source_kind,
            Some(
                StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. }
            )
        );
    if !valid || !exact_length || !valid_result || !writable {
        return Err(
            if matches!(operation, AbstractOperation::ByteSequenceSubslice { .. }) {
                OptimizationUnitValidationError::InvalidByteSequenceSubslice {
                    machine: function.machine,
                    block,
                    node,
                }
            } else if matches!(operation, AbstractOperation::ByteSequenceWrite { .. }) {
                OptimizationUnitValidationError::InvalidByteSequenceWrite {
                    machine: function.machine,
                    block,
                    node,
                }
            } else if length.is_some() {
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
            },
        );
    }
    Ok(())
}

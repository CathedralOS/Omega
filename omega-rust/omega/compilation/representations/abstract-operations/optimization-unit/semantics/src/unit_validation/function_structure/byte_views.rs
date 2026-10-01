//! Exact byte-view observation and mutation source contracts.

use std::collections::BTreeMap;

use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::OptimizationUnitValidationError;

/// Resolve the bounded field from the exact writable destination declaration.
/// Replacement source validity and indexed-store freshness are checked separately.
pub(crate) fn byte_field_store_capacity(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<u64> {
    if let AbstractOperation::ByteSequenceStore {
        destination,
        source,
        ..
    } = operation
    {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == *destination)?;
        if destination == source
            || !matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == *destination)
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination)
        {
            return None;
        }
        return match structural_types.get(&parameter.structural_type)?.shape {
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) => {
                Some(capacity)
            }
            _ => None,
        };
    }
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
    // A byte index bound proves address safety, not preservation of a whole
    // carrier predicate such as UTF-8. A store into a predicate-bearing field
    // is admitted here by its carrier alone; its accepted goal must carry the
    // stored byte's class legs (the obligation catalog checks that) and a
    // UTF-8 field must be all-ASCII on every path (`byte_predicates`).
    match selected.field_type.byte_sequence_carrier() {
        Some(ByteSequenceCarrier::BoundedOwned { capacity }) => Some(capacity),
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
        AbstractOperation::ByteSequenceStore { source, length, .. }
        | AbstractOperation::StructuralByteSequenceFieldStore { source, length, .. }
        | AbstractOperation::ByteSequenceRead { source, length, .. }
        | AbstractOperation::ByteSequenceSubslice { source, length, .. } => {
            (*source, Some(*length))
        }
        _ => return Ok(()),
    };
    validate_source(
        function,
        block,
        node,
        operation,
        source_kind,
        structural_types,
        source,
        length,
    )
}

pub(super) fn validate_source(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    operation: &AbstractOperation,
    source_kind: Option<&StructuralPlaceKind>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    source: semantic_vocabulary::PlaceId,
    length: Option<semantic_vocabulary::ValueId>,
) -> Result<(), OptimizationUnitValidationError> {
    let permitted_access = |access| match operation {
        AbstractOperation::ByteSequenceWrite { .. } => access == StructuralAccess::MutableBorrow,
        AbstractOperation::ByteSequenceLength { .. } => matches!(
            access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        ),
        _ => access == StructuralAccess::SharedBorrow,
    };
    // A shared bounded-owner loan keeps its owner's plain byte qualifications;
    // it is read through the owner layout, never written.
    let owner_loan = |parameter: &terminal_psi::StructuralParameterDeclaration| {
        !matches!(operation, AbstractOperation::ByteSequenceWrite { .. })
            && abstract_operations::control_flow::owner_loans::is_shared_bounded_owner_loan(
                parameter,
                |identity| structural_types.get(&identity).copied(),
            )
    };
    // A by-value bounded owner supplies readable bytes from its own incoming
    // payload. This does not make it a mutable view or a borrowed descriptor.
    let owned_parameter = |parameter: &terminal_psi::StructuralParameterDeclaration| {
        !matches!(operation, AbstractOperation::ByteSequenceWrite { .. })
            && abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
                parameter,
                |identity| structural_types.get(&identity).copied(),
            )
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
                    && ((permitted_access(parameter.access)
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty())
                        || owner_loan(parameter))
            })
            .map(|parameter| parameter.structural_type),
        Some(StructuralPlaceKind::Parameter { .. }) => function
            .structural_parameters
            .iter()
            .find(|parameter| {
                parameter.place == source
                    && ((permitted_access(parameter.access)
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty())
                        || owner_loan(parameter)
                        || owned_parameter(parameter))
            })
            .map(|parameter| parameter.structural_type),
        Some(StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }) => Some(*structural_type),
        Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }) => owned_byte_result(function, source_kind, source, |identity| {
            structural_types.get(&identity).is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                )
            })
        })
        .or_else(|| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find_map(|node| match &node.operation {
                AbstractOperation::ByteSequenceSubslice {
                    psi_operation,
                    result,
                    ..
                }
                // A `&[u8]` view is established through the element-view
                // family (`element` = `u8`); the byte carrier keeps no
                // element field, so the view kind is read from the result's
                // declared `ByteSequence` shape.
                | AbstractOperation::EstablishElementView {
                    psi_operation,
                    result,
                    ..
                } if psi_operation == producer
                    && result.place == source
                    && result.structural_type == *structural_type
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.qualification_establishments.is_empty()
                    && result.claims.is_empty() =>
                {
                    Some(result.structural_type)
                }
                _ => None,
            })
        }),
        _ => None,
    };
    let valid = structural_type
        .and_then(|identity| structural_types.get(&identity))
        .is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView { .. }
                        | ByteSequenceCarrier::BoundedView { .. }
                )
            ) || (matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
            ) && (owned_byte_result(function, source_kind, source, |identity| {
                structural_types.get(&identity).is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                    )
                })
            })
            .is_some()
                || function
                    .structural_parameters
                    .iter()
                    .chain(
                        function
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .any(|parameter| parameter.place == source && owner_loan(parameter))
                || function
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == source && owned_parameter(parameter))))
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
                && structural_types
                    .get(&result.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BorrowedView { .. }
                            )
                        )
                    })
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
        )
        || matches!(source_kind, Some(StructuralPlaceKind::OperationResult { .. }))
            && function.blocks.iter().flat_map(|block| &block.nodes).any(|node| {
                matches!(&node.operation, AbstractOperation::EstablishByteSequenceConcatenation { result, .. }
                    if result.place == source)
            });
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

/// A bounded owner keeps the `{live length, capacity bytes}` layout whether it
/// was established locally or completed by an ordinary structural call. The
/// place declaration must name that exact producer and type; borrowed and
/// boundary results remain descriptor/storage families owned elsewhere.
pub(super) fn owned_byte_result(
    function: &PsiOptimizationFunction,
    source_kind: Option<&StructuralPlaceKind>,
    source: semantic_vocabulary::PlaceId,
    mut bounded_owned_type: impl FnMut(StructuralTypeId) -> bool,
) -> Option<StructuralTypeId> {
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = source_kind?
    else {
        return None;
    };
    let mut matching = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| {
            let (operation, result) = match &node.operation {
                AbstractOperation::EstablishByteSequenceConcatenation {
                    psi_operation,
                    result,
                    ..
                }
                | AbstractOperation::CallStructural {
                    psi_operation,
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                } => (psi_operation, result),
                _ => return None,
            };
            (*operation == *producer
                && result.place == source
                && result.structural_type == *structural_type
                && result.multiplicity == StructuralMultiplicity::Unrestricted
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && bounded_owned_type(*structural_type))
            .then_some(*structural_type)
        });
    let structural_type = matching.next()?;
    matching.next().is_none().then_some(structural_type)
}

//! Exact byte-view observation and mutation source contracts.
//! Entry and state arrivals share one declaration lookup and access rule.
//! Length observes owner metadata, reads require readable contents, and
//! replacement requires an immutable source. A state edge changes none of
//! those permissions; custody separately checks availability and currentness.

use std::collections::BTreeMap;

use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralDomainDeclaration, StructuralMultiplicity,
    StructuralTypeDeclaration, StructuralTypeShape,
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
        let parameter = function.arrival_parameter(*destination)?;
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
    // A byte index bound proves address safety, not preservation of a whole
    // carrier predicate such as UTF-8. A store into a predicate-bearing field
    // is admitted here by its carrier alone; its accepted goal must carry the
    // stored byte's class legs (the obligation catalog checks that) and a
    // UTF-8 field must be all-ASCII on every path (`byte_predicates`).
    match writable_byte_field_carrier(function, *destination, path, *field, structural_types)? {
        ByteSequenceCarrier::BoundedOwned { capacity } => Some(capacity),
        _ => None,
    }
}

/// Descriptor shape and readable custody are independent of backing lifetime.
/// The current-position descriptor analysis checks the latter; a literal,
/// arrival and completed operation result use the same source contract here.
pub(crate) fn byte_view_field_store_is_valid(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    source_kind: Option<&StructuralPlaceKind>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    let AbstractOperation::StructuralByteViewFieldStore {
        destination,
        path,
        field,
        source,
        ..
    } = operation
    else {
        return false;
    };
    if source_kind.is_none() || destination == source {
        return false;
    }
    let Some(contract) = crate::unit_validation::operation_contracts::structural_source_contract(
        function, *source, true,
    ) else {
        return false;
    };
    matches!(
        contract.access,
        StructuralAccess::SharedBorrow | StructuralAccess::Owned
    ) && contract.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && structural_types
            .get(&contract.structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
        && writable_byte_field_carrier(function, *destination, path, *field, structural_types)
            == Some(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            })
}

/// The declared carrier of one byte field below an exclusive, unqualified,
/// claim-free parameter or an explicit mutable-record `Referent` crossing.
fn writable_byte_field_carrier(
    function: &PsiOptimizationFunction,
    destination: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<ByteSequenceCarrier> {
    let (root_type, storage_path) = if matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        super::super::references::mutable_record_field_destination(
            function,
            structural_types,
            destination,
            path,
        )?
    } else {
        let (root_type, access) =
            super::byte_field_freshness::byte_field_root(function, destination)?;
        let writable = match access {
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => true,
            // Owned payload is writable; owning a loan descriptor does not
            // itself grant authority over the memory its pointer denotes.
            StructuralAccess::Owned => matches!(
                structural_types.get(&root_type)?.shape,
                StructuralTypeShape::Record { .. }
                    | StructuralTypeShape::Mixed { .. }
                    | StructuralTypeShape::FixedArray { .. }
            ),
            StructuralAccess::SharedBorrow => false,
        };
        if !writable {
            return None;
        }
        (root_type, path)
    };
    if !terminal_psi::is_structural_scalar_store_path(storage_path) {
        return None;
    }
    let parent =
        super::byte_field_freshness::field_carrier(structural_types, root_type, storage_path)?;
    let StructuralTypeShape::Record { fields } = &structural_types.get(&parent)?.shape else {
        return None;
    };
    fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?
        .field_type
        .byte_sequence_carrier()
}

pub(super) fn validate_byte_view_source(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    operation: &AbstractOperation,
    source_kind: Option<&StructuralPlaceKind>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
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
        structural_domains,
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
    structural_domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
    source: semantic_vocabulary::PlaceId,
    length: Option<semantic_vocabulary::ValueId>,
) -> Result<(), OptimizationUnitValidationError> {
    let permitted_access = |access| match operation {
        AbstractOperation::ByteSequenceWrite { .. } => access == StructuralAccess::MutableBorrow,
        AbstractOperation::ByteSequenceStore { .. }
        | AbstractOperation::StructuralByteSequenceFieldStore { .. } => {
            access == StructuralAccess::SharedBorrow
        }
        _ => matches!(
            access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::Owned
        ),
    };
    let arrival = function.arrival_parameter(source);
    let structural_type = match source_kind {
        Some(
            StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. },
        ) => {
            arrival.and_then(|parameter| {
                let StructuralTypeShape::ByteSequence(carrier) =
                    structural_types.get(&parameter.structural_type)?.shape
                else {
                    return None;
                };
                let owned = matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. });
                let permitted = match operation {
                    AbstractOperation::ByteSequenceWrite { .. } => {
                        !owned
                            && parameter.access == StructuralAccess::MutableBorrow
                            && parameter.qualifications.is_empty()
                    }
                    AbstractOperation::ByteSequenceLength { .. } => {
                        matches!(
                            parameter.access,
                            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                        ) || owned
                            && matches!(
                                parameter.access,
                                StructuralAccess::Owned | StructuralAccess::WriteOnlyBorrow
                            )
                    }
                    AbstractOperation::ByteSequenceStore { .. }
                    | AbstractOperation::StructuralByteSequenceFieldStore { .. } => {
                        parameter.access == StructuralAccess::SharedBorrow
                            || owned && parameter.access == StructuralAccess::Owned
                    }
                    _ => {
                        matches!(
                            parameter.access,
                            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                        ) || owned && parameter.access == StructuralAccess::Owned
                    }
                };
                // A same-carrier domain is not necessarily a byte predicate.
                // Replay the exact primitive roster before observing contents
                // or owner metadata; this grants no mutation permission.
                let qualifications_permitted = terminal_semantics::byte_parameter_predicates_in(
                    structural_domains.values().copied(),
                    parameter,
                )
                .is_some();
                (permitted
                    && qualifications_permitted
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.projected_qualifications.is_empty())
                .then_some(parameter.structural_type)
            })
        }
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
            let contract = super::super::operation_contracts::structural_source_contract(
                function, source, false,
            );
            let contract = contract?;
            let result = contract.operation_result()?;
            (result.place == source
                && result.structural_type == *structural_type
                && permitted_access(contract.access)
                && contract.is_unqualified()
                && !contract.has_result_evidence()
                && function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .any(|node| node.operation.structural_result() == Some((*producer, result))))
            .then_some(result.structural_type)
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
                || arrival.is_some()))
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
                && super::super::operation_contracts::structural_operation_result_contract(
                    function, operation,
                )
                .is_some_and(|contract| contract.operation_result() == Some(result))
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
        || super::super::operation_contracts::structural_source_contract(function, source, false)
            .is_some_and(|contract| {
                matches!(
                    contract.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::Owned
                )
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

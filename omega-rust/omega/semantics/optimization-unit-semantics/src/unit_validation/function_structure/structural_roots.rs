//! Structural-root uniqueness, availability, observation, and return contracts.
use crate::OptimizationUnitValidationError;
use crate::unit_validation::structural_catalog::structural_root_key;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};

mod availability;
mod primitive_locals;

pub(super) use availability::operation_place_inputs;
pub(crate) use availability::validate_structural_place_availability;

pub(crate) fn validate_structural_root_uniqueness(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut roots = BTreeSet::new();
    for place in &function.structural_places {
        if !roots.insert(structural_root_key(place.kind)) {
            return Err(
                OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                    machine: function.machine,
                    kind: place.kind,
                },
            );
        }
    }
    Ok(())
}

/// Validate the closed root roles of structural observations and structural
/// returns. This is deliberately independent of the later full ownership walk:
/// it establishes which catalog roots may participate and replays every
/// observation invariant still representable after Terminal-to-Omega lowering.
pub(crate) fn validate_structural_root_operations(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let place_kinds = function
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            primitive_locals::validate(
                function,
                block.id,
                node_index,
                &node.operation,
                structural_types,
            )?;
            match &node.operation {
                O::StructuralByteSequenceFieldByteStore { .. } => {
                    if super::byte_views::byte_field_store_capacity(
                        function,
                        &node.operation,
                        structural_types,
                    )
                    .is_none()
                    {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::StructuralByteSequenceFieldStore { source, .. } => {
                    super::byte_views::validate_byte_view_source(
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        place_kinds.get(source),
                        structural_types,
                    )?;
                    if super::byte_views::byte_field_store_capacity(
                        function,
                        &node.operation,
                        structural_types,
                    )
                    .is_none()
                    {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::StructuralByteSequenceFieldLength {
                    source,
                    path,
                    field,
                    ..
                } => {
                    let valid = function.structural_parameters.iter().find(|parameter| parameter.place == *source)
                        .is_some_and(|parameter| {
                            matches!(parameter.access,
                                terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow)
                            && matches!(parameter.multiplicity,
                                terminal_psi::StructuralMultiplicity::Unrestricted
                                | terminal_psi::StructuralMultiplicity::Affine)
                            && parameter.qualifications.is_empty()
                            && parameter.projected_qualifications.is_empty()
                            && terminal_psi::is_bounded_structural_scalar_store_path(path)
                            && function.entry_claim_declarations.iter().all(|claim| claim.input != *source)
                            && function.content_entry_claims.iter().all(|claim| claim.input.root != *source)
                            && matches!(place_kinds.get(source),
                                Some(StructuralPlaceKind::Parameter { position, is_self })
                                if *position == parameter.position && *is_self == parameter.is_self)
                            && super::super::structural_catalog::resolve_structural_path(
                                structural_types, parameter.structural_type, path,
                            ).and_then(|parent| structural_types.get(&parent))
                            .is_some_and(|declaration| match &declaration.shape {
                                terminal_psi::StructuralTypeShape::Record { fields } => fields.iter().any(|candidate|
                                    candidate.id == *field && !candidate.relevance.is_erased()
                                    && matches!(candidate.field_type, terminal_psi::StructuralFieldType::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }))),
                                _ => false,
                            })
                        });
                    if !valid {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::StructuralCaseMembership {
                    source,
                    path,
                    case,
                    result,
                    ..
                } => {
                    let signature =
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function, *source, false,
                        );
                    let valid = result.scalar_type == ScalarType::Boolean
                        && signature.is_some_and(|signature| {
                            signature.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
                                && super::super::structural_catalog::resolve_structural_path(
                                    structural_types,
                                    signature.structural_type,
                                    path,
                                )
                                .and_then(|endpoint| structural_types.get(&endpoint))
                                .is_some_and(|declaration| match &declaration.shape {
                                    terminal_psi::StructuralTypeShape::Sum { cases }
                                    | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => {
                                        cases.iter().any(|candidate| candidate.id == *case)
                                    }
                                    _ => false,
                                })
                        });
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidStructuralCaseDispatch {
                                machine: function.machine,
                                source: *source,
                            },
                        );
                    }
                }
                O::ByteSequenceSubslice { source, .. }
                | O::ByteSequenceLength { source, .. }
                | O::ByteSequenceRead { source, .. } => {
                    super::byte_views::validate_byte_view_source(
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        place_kinds.get(source),
                        structural_types,
                    )?;
                }
                O::ByteSequenceWrite { destination, .. } => {
                    super::byte_views::validate_byte_view_source(
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        place_kinds.get(destination),
                        structural_types,
                    )?;
                }
                O::WriteOnlyPrimitiveStore {
                    destination,
                    path,
                    value,
                    ..
                } => {
                    let valid = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == destination.place)
                        == Some(destination)
                        && matches!(
                            destination.access,
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                        && (destination.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                            || (!path.is_empty()
                                && destination.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Affine))
                        && destination.qualifications.is_empty()
                        && destination.projected_qualifications.is_empty()
                        && function
                            .entry_claim_declarations
                            .iter()
                            .all(|claim| claim.input != destination.place)
                        && function
                            .content_entry_claims
                            .iter()
                            .all(|claim| claim.input.root != destination.place)
                        && matches!(
                            place_kinds.get(&destination.place),
                            Some(StructuralPlaceKind::Parameter { position, is_self })
                                if *position == destination.position
                                    && *is_self == destination.is_self
                        )
                        && terminal_semantics::primitive_place_type(
                            structural_types.values().copied(),
                            destination.structural_type,
                            path,
                        ) == Some(value.scalar_type);
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    value,
                    range_obligation,
                    ..
                } => {
                    let parent = super::super::structural_catalog::resolve_structural_path(
                        structural_types,
                        destination.structural_type,
                        path,
                    );
                    let valid = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == destination.place)
                        == Some(destination)
                        && matches!(
                            destination.multiplicity,
                            terminal_psi::StructuralMultiplicity::Unrestricted
                                | terminal_psi::StructuralMultiplicity::Affine
                        )
                        && matches!(
                            destination.access,
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                        && destination.qualifications.is_empty()
                        && destination.projected_qualifications.is_empty()
                        && terminal_psi::is_bounded_structural_scalar_store_path(path)
                        && function
                            .entry_claim_declarations
                            .iter()
                            .all(|claim| claim.input != destination.place)
                        && function
                            .content_entry_claims
                            .iter()
                            .all(|claim| claim.input.root != destination.place)
                        && matches!(
                            place_kinds.get(&destination.place),
                            Some(StructuralPlaceKind::Parameter { position, is_self })
                                if *position == destination.position
                                    && *is_self == destination.is_self
                        )
                        && parent.is_some_and(|parent| {
                            direct_relevant_scalar_field(
                                structural_types,
                                parent,
                                *field,
                                range_obligation.is_some(),
                            ) == Some(value.scalar_type)
                        });
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidStructuralScalarFieldStore {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::BooleanStructuralField {
                    source,
                    path,
                    field,
                    ..
                } => {
                    // Root catalogs and dominance are checked before this pass;
                    // current ownership separately checks live, whole owned inputs.
                    let valid = readable_field_type(function, *source)
                        .and_then(|root| {
                            terminal_semantics::record_field_carrier(
                                structural_types.values().copied(),
                                root,
                                path,
                            )
                        })
                        .is_some_and(|carrier| {
                            let identity = carrier.structural_type;
                            direct_relevant_scalar_field(structural_types, identity, *field, true)
                                == Some(ScalarType::Boolean)
                        });
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidBooleanStructuralField {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::IntegerStructuralField {
                    result,
                    source,
                    path,
                    field,
                    ..
                } => {
                    let valid = matches!(result.scalar_type, ScalarType::Integer(_))
                        && readable_field_type(function, *source)
                            .and_then(|root| {
                                terminal_semantics::record_field_carrier(
                                    structural_types.values().copied(),
                                    root,
                                    path,
                                )
                            })
                            .is_some_and(|carrier| {
                                let identity = carrier.structural_type;
                                direct_relevant_scalar_field(
                                    structural_types,
                                    identity,
                                    *field,
                                    true,
                                ) == Some(result.scalar_type)
                            });
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidIntegerStructuralField {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::EstablishReference {
                    psi_operation,
                    result,
                    source,
                } => {
                    // Whole-carrier establishment: the result is an affine
                    // `ref mut` over a primitive referent and the source must
                    // name exactly that referent under mutable authority.
                    let expected = crate::unit_validation::references::referent(
                        structural_types,
                        result.structural_type,
                    );
                    let valid = expected.is_some()
                        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                        && matches!(
                            place_kinds.get(&result.place),
                            Some(StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type,
                            }) if *producer == *psi_operation
                                && *structural_type == result.structural_type
                        )
                        && source.access == terminal_psi::StructuralAccess::MutableBorrow
                        && crate::unit_validation::references::reference_source_type(
                            function,
                            structural_types,
                            source,
                        ) == expected;
                    if !valid {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::ReleaseReference { source, .. } => {
                    let valid =
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function, *source, false,
                        )
                        .is_some_and(|signature| {
                            crate::unit_validation::references::referent(
                                structural_types,
                                signature.structural_type,
                            )
                            .is_some()
                        });
                    if !valid {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::MoveStructuralField {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                } => {
                    // Replay the verified static shape: the exact borrowed
                    // parameter row, a declared structural field beneath the
                    // spelled path, and a fresh operation-result place of
                    // exactly that type under clean exact-once custody.
                    let valid = borrowed_window_root(function, source, &place_kinds)
                        && window_field_type(
                            structural_types,
                            source.structural_type,
                            path,
                            *field,
                        ) == Some(result.structural_type)
                        && matches!(
                            result.multiplicity,
                            terminal_psi::StructuralMultiplicity::Unrestricted
                                | terminal_psi::StructuralMultiplicity::Affine
                        )
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                        && matches!(
                            place_kinds.get(&result.place),
                            Some(StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type,
                            }) if *producer == *psi_operation
                                && *structural_type == result.structural_type
                        );
                    if !valid {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    // The repair targets the same borrowed root authority and
                    // consumes one owned whole place of the declared field
                    // type. Whether a window is open there is debt evidence
                    // the Terminal verifier retained; the unit keeps the
                    // static contract honest.
                    let valid = borrowed_window_root(function, destination, &place_kinds)
                        && value.access == terminal_psi::StructuralAccess::Owned
                        && value.path.is_empty()
                        && window_field_type(
                            structural_types,
                            destination.structural_type,
                            path,
                            *field,
                        )
                        .is_some_and(|field_type| {
                            crate::unit_validation::operation_contracts::structural_source_contract(
                                function,
                                value.place,
                                false,
                            )
                            .is_some_and(|signature| {
                                signature.structural_type == field_type
                                    && signature.is_unqualified()
                            })
                        });
                    if !valid {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::StructuralLeafCopy {
                    psi_operation,
                    result,
                    source,
                    path,
                } => {
                    // Replay the verified static shape: a readable source
                    // root, the spelled path resolving to exactly the result
                    // type, and a fresh operation-result place under clean
                    // unrestricted custody.
                    let valid =
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function, *source, false,
                        )
                        .is_some_and(|signature| {
                            signature.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
                            && crate::unit_validation::structural_catalog::resolve_structural_path(
                                structural_types,
                                signature.structural_type,
                                path,
                            ) == Some(result.structural_type)
                        }) && result.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                            && matches!(
                                place_kinds.get(&result.place),
                                Some(StructuralPlaceKind::OperationResult {
                                    producer,
                                    structural_type,
                                }) if *producer == *psi_operation
                                    && *structural_type == result.structural_type
                            );
                    if !valid {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
                O::ReturnStructural { source, .. } => {
                    let Some(signature) = function.result.structural() else {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    };
                    let source_contract =
                        function
                            .structural_parameters
                            .iter()
                            .find(|parameter| {
                                parameter.place == *source
                                    && matches!(
                                        place_kinds.get(source),
                                        Some(StructuralPlaceKind::Parameter { position, is_self })
                                            if *position == parameter.position
                                                && *is_self == parameter.is_self
                                    )
                            })
                            .or_else(|| {
                                let Some(StructuralPlaceKind::BlockParameter { block, position }) =
                                    place_kinds.get(source).copied()
                                else {
                                    return None;
                                };
                                function
                                    .blocks
                                    .iter()
                                    .find(|candidate| candidate.id == block)
                                    .and_then(|block| {
                                        block.structural_parameters.get(position as usize)
                                    })
                                    .filter(|parameter| {
                                        parameter.place == *source
                                            && parameter.position == position
                                            && !parameter.is_self
                                            && parameter.access
                                                == terminal_psi::StructuralAccess::Owned
                                    })
                            })
                            .map(|parameter| {
                                (
                                    parameter.structural_type,
                                    parameter.multiplicity,
                                    parameter.qualifications.as_slice(),
                                    parameter.projected_qualifications.as_slice(),
                                    BTreeSet::new(),
                                )
                            })
                            .or_else(|| {
                                let Some(StructuralPlaceKind::OperationResult {
                                    producer,
                                    structural_type,
                                }) = place_kinds.get(source).copied()
                                else {
                                    return None;
                                };
                                function
                                .blocks
                                .iter()
                                .flat_map(|block| &block.nodes)
                                .find_map(|node| match &node.operation {
                                    O::EstablishScalarArray { psi_operation, result, .. }
                                    | O::EstablishScalarCase {
                                        psi_operation,
                                        result,
                                        ..
                                    }
                                    | O::EstablishRecord {
                                        psi_operation,
                                        result,
                                        ..
                                    }
                                    | O::EstablishReference {
                                        psi_operation,
                                        result,
                                        ..
                                    }
                                    | O::StructuralLeafCopy {
                                        psi_operation,
                                        result,
                                        ..
                                    }
                                    | O::CallStructural {
                                        psi_operation,
                                        result,
                                        ..
                                    }
                                    | O::BoundaryCall {
                                        psi_operation,
                                        result:
                                            abstract_operations::AbstractBoundaryResult::Structural(
                                                result,
                                            ),
                                        ..
                                    } if *psi_operation == producer
                                        && result.place == *source
                                        && result.structural_type == structural_type =>
                                    {
                                        Some((
                                            result.structural_type,
                                            result.multiplicity,
                                            result.qualifications.as_slice(),
                                            result.projected_qualifications.as_slice(),
                                            result
                                                .qualification_establishments
                                                .iter()
                                                .map(|binding| binding.domain)
                                                .collect::<BTreeSet<_>>(),
                                        ))
                                    }
                                    _ => None,
                                })
                            });
                    if source_contract.is_none_or(
                        |(structural_type, multiplicity, qualifications, projected, minted)| {
                            // Mirror the Terminal return-edge rule: domains
                            // minted onto the source by its producer's
                            // authorized qualification establishments shed at
                            // the contract edge, so the remaining source
                            // qualifications must live inside the declared
                            // result roster rather than equal it.
                            structural_type != signature.structural_type
                                || multiplicity != signature.multiplicity
                                || !qualifications
                                    .iter()
                                    .filter(|domain| !minted.contains(domain))
                                    .all(|domain| signature.qualifications.contains(domain))
                                || projected
                                    .iter()
                                    .filter(|projection| !minted.contains(&projection.domain))
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    != signature.projected_qualifications
                        },
                    ) {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn direct_relevant_scalar_field(
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    field: semantic_vocabulary::StructuralFieldId,
    allow_bounded_integer: bool,
) -> Option<ScalarType> {
    let declaration = structural_types.get(&structural_type)?;
    let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(|field_type| match field_type {
                terminal_psi::StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
                terminal_psi::StructuralFieldType::IeeeFloat(format) => {
                    Some(ScalarType::IeeeFloat(*format))
                }
                // Construction obligations remain checked separately. A scalar
                // observation can use this carrier, but an unproved store cannot.
                terminal_psi::StructuralFieldType::BoundedInteger(bounds)
                    if allow_bounded_integer =>
                {
                    Some(ScalarType::Integer(bounds.integer_type()))
                }
                _ => None,
            })
    })
}

fn readable_field_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let signature = crate::unit_validation::operation_contracts::structural_source_contract(
        function, place, false,
    )?;
    if signature.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
        || signature.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !signature.is_unqualified()
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == place)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                let result = match &node.operation {
                    O::EstablishRecord { result, .. }
                    | O::EstablishScalarArray { result, .. }
                    | O::EstablishScalarCase { result, .. }
                    | O::EstablishReference { result, .. }
                    | O::MoveStructuralField { result, .. }
                    | O::CallStructural { result, .. }
                    | O::BoundaryCall {
                        result: abstract_operations::AbstractBoundaryResult::Structural(result),
                        ..
                    } => result,
                    _ => return false,
                };
                result.place == place && !result.claims.is_empty()
            })
    {
        return None;
    }
    Some(signature.structural_type)
}

/// The borrowed machine parameter a restoration-window operation may name,
/// replayed against the unit's retained rows: the exact declaration,
/// mutable-borrow authority, a transferable multiplicity, and no
/// qualifications or entry claims — the same static contract the Terminal
/// verifier's `borrowed_window_root` admitted.
fn borrowed_window_root(
    function: &PsiOptimizationFunction,
    parameter: &terminal_psi::StructuralParameterDeclaration,
    place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
) -> bool {
    function
        .structural_parameters
        .iter()
        .find(|candidate| candidate.place == parameter.place)
        == Some(parameter)
        && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
        && matches!(
            parameter.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
                | terminal_psi::StructuralMultiplicity::Affine
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && function
            .entry_claim_declarations
            .iter()
            .all(|claim| claim.input != parameter.place)
        && function
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != parameter.place)
        && matches!(
            place_kinds.get(&parameter.place),
            Some(StructuralPlaceKind::Parameter { position, is_self })
                if *position == parameter.position && *is_self == parameter.is_self
        )
}

/// The declared `Structural` type of the field a window operation names,
/// resolved beneath the borrowed root through the spelled path. `None` means
/// the place does not host a structural field there.
fn window_field_type(
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<StructuralTypeId> {
    let parent =
        super::super::structural_catalog::resolve_structural_path(structural_types, root, path)?;
    let declaration = structural_types.get(&parent)?;
    let fields = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Record { fields }
        | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields,
        _ => return None,
    };
    match fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?
        .field_type
    {
        terminal_psi::StructuralFieldType::Structural(field_type) => Some(field_type),
        _ => None,
    }
}

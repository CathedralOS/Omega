//! Structural-root uniqueness, availability, observation, and return contracts.

use super::*;

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
                O::StructuralCaseMembership {
                    source,
                    case,
                    result,
                    ..
                } => {
                    let signature = crate::unit_validation::structural_source_contract(
                        function, *source, false,
                    );
                    let valid = result.scalar_type == ScalarType::Boolean
                        && signature.is_some_and(|signature| {
                            signature.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
                                && structural_types
                                    .get(&signature.structural_type)
                                    .is_some_and(|declaration| match &declaration.shape {
                                        terminal_psi::StructuralTypeShape::Sum { cases }
                                        | terminal_psi::StructuralTypeShape::Mixed {
                                            cases, ..
                                        } => cases.iter().any(|candidate| candidate.id == *case),
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
                    destination, value, ..
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
                        && destination.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                        && destination.qualifications.is_empty()
                        && matches!(
                            place_kinds.get(&destination.place),
                            Some(StructuralPlaceKind::Parameter { position, is_self })
                                if *position == destination.position
                                    && *is_self == destination.is_self
                        )
                        && structural_types
                            .get(&destination.structural_type)
                            .is_some_and(|declaration| {
                                matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                        scalar_type
                                    ) if scalar_type == value.scalar_type
                                )
                            });
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
                            direct_relevant_scalar_field(structural_types, parent, *field)
                                == Some(value.scalar_type)
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
                O::BooleanStructuralField { source, field, .. } => {
                    // Root catalogs and dominance are checked before this pass;
                    // current ownership separately checks live, whole owned inputs.
                    let valid = readable_field_type(function, *source).is_some_and(|identity| {
                        direct_relevant_scalar_field(structural_types, identity, *field)
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
                    field,
                    ..
                } => {
                    let valid = matches!(result.scalar_type, ScalarType::Integer(_))
                        && readable_field_type(function, *source).is_some_and(|identity| {
                            direct_relevant_scalar_field(structural_types, identity, *field)
                                == Some(result.scalar_type)
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
                                        ))
                                    }
                                    _ => None,
                                })
                            });
                    if source_contract.is_none_or(
                        |(structural_type, multiplicity, qualifications, projected)| {
                            structural_type != signature.structural_type
                                || multiplicity != signature.multiplicity
                                || qualifications != signature.qualifications.as_slice()
                                || projected != signature.projected_qualifications.as_slice()
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
                _ => None,
            })
    })
}

fn readable_field_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let signature = crate::unit_validation::structural_source_contract(function, place, false)?;
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

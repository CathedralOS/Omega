//! Structural-root uniqueness, availability, observation, and return contracts.

use super::super::derived_metadata::dominators;
use super::*;

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

pub(crate) fn validate_structural_place_availability(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut producers = BTreeMap::<PlaceId, (BlockId, u32)>::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let place = match &node.operation {
                O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
                    Some(result.place)
                }
                O::EstablishByteSequenceLiteral { place, .. }
                | O::EstablishTrivialAffineLocal { place, .. } => Some(place.id),
                _ => None,
            };
            if let Some(place) = place {
                producers.insert(
                    place,
                    (
                        block.id,
                        u32::try_from(node_index).expect("unit node index fits u32"),
                    ),
                );
            }
        }
    }
    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            for place in operation_place_inputs(&node.operation) {
                let Some((producer_block, producer_node)) = producers.get(&place) else {
                    continue;
                };
                let available = (*producer_block == block.id && *producer_node < node_index)
                    || (*producer_block != block.id
                        && dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(producer_block)));
                if !available {
                    return Err(
                        OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                            place,
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn operation_place_inputs(operation: &O) -> Vec<PlaceId> {
    let mut inputs = match operation {
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. } => vec![destination.place],
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect(),
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        } => vec![
            dynamic_dispatch.initial.source.place,
            dynamic_dispatch.rebound.source.place,
        ],
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            vec![*source]
        }
        O::IntegerStructuralField { source, .. } => vec![source.place],
        _ => Vec::new(),
    };
    match operation {
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => inputs.extend(cleanup_actions.iter().map(|cleanup| match cleanup {
            psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
            psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => discard.place,
            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => cleanup.place,
        })),
        O::ReturnStructural {
            trivial_affine_discards,
            ..
        } => inputs.extend(trivial_affine_discards.iter().copied()),
        _ => {}
    }
    inputs
}

/// Validate the closed root roles of structural observations and structural
/// returns. This is deliberately independent of the later full ownership walk:
/// it establishes which catalog roots may participate and replays every
/// observation invariant still representable after Terminal-to-Omega lowering.
pub(crate) fn validate_structural_root_operations(
    function: &PsiOptimizationFunction,
    unit_entry: MachineId,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let place_kinds = function
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    let observations = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match node.operation {
            O::BooleanStructuralField { source, field, .. } => Some((source, field)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let integer_observations = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::IntegerStructuralField { source, field, .. } => Some((source.place, *field)),
            _ => None,
        })
        .collect::<Vec<_>>();

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            match &node.operation {
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
                            psi_terminal::StructuralAccess::MutableBorrow
                                | psi_terminal::StructuralAccess::WriteOnlyBorrow
                        )
                        && destination.multiplicity
                            == psi_terminal::StructuralMultiplicity::Unrestricted
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
                                    psi_terminal::StructuralTypeShape::PrimitiveScalar(
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
                        && destination.multiplicity
                            == psi_terminal::StructuralMultiplicity::Unrestricted
                        && matches!(
                            destination.access,
                            psi_terminal::StructuralAccess::MutableBorrow
                                | psi_terminal::StructuralAccess::WriteOnlyBorrow
                        )
                        && destination.qualifications.is_empty()
                        && destination.projected_qualifications.is_empty()
                        && matches!(
                            path.as_slice(),
                            [psi_terminal::StructuralPathSegment::Field(_)]
                        )
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
                    let parameter = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == *source);
                    let valid = parameter.is_some_and(|parameter| {
                        let affine_entry_observation = function.machine == unit_entry
                            && parameter.multiplicity
                                == psi_terminal::StructuralMultiplicity::Affine
                            && function
                                .parameters
                                .iter()
                                .any(|parameter| parameter.scalar_type == ScalarType::Boolean)
                            && every_scalar_return_nominally_cleans(function, *source);
                        let unrestricted_shared_observation = parameter.multiplicity
                            == psi_terminal::StructuralMultiplicity::Unrestricted
                            && parameter.access == psi_terminal::StructuralAccess::SharedBorrow;

                        (affine_entry_observation || unrestricted_shared_observation)
                            && parameter.qualifications.is_empty()
                            && parameter.access != psi_terminal::StructuralAccess::WriteOnlyBorrow
                            && observations
                                .iter()
                                .all(|candidate| candidate == &(*source, *field))
                            && function.content_entry_claims.is_empty()
                            && function
                                .entry_claim_declarations
                                .iter()
                                .all(|claim| claim.input != *source)
                            && matches!(
                                place_kinds.get(source),
                                Some(StructuralPlaceKind::Parameter { position, is_self })
                                    if *position == parameter.position
                                        && *is_self == parameter.is_self
                            )
                            && structural_types
                                .get(&parameter.structural_type)
                                .is_some_and(|declaration| {
                                    let psi_terminal::StructuralTypeShape::Record { fields } =
                                        &declaration.shape
                                    else {
                                        return false;
                                    };
                                    fields.iter().any(|candidate| {
                                        candidate.id == *field
                                            && !candidate.relevance.is_erased()
                                            && candidate.field_type
                                                == psi_terminal::StructuralFieldType::Scalar(
                                                    ScalarType::Boolean,
                                                )
                                    })
                                })
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
                    let valid = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == source.place)
                        == Some(source)
                        && matches!(
                            source.multiplicity,
                            psi_terminal::StructuralMultiplicity::Unrestricted
                                | psi_terminal::StructuralMultiplicity::Affine
                        )
                        && source.access == psi_terminal::StructuralAccess::SharedBorrow
                        && source.qualifications.is_empty()
                        && source.projected_qualifications.is_empty()
                        && matches!(result.scalar_type, ScalarType::Integer(_))
                        && integer_observations
                            .iter()
                            .all(|candidate| candidate == &(source.place, *field))
                        && function
                            .entry_claim_declarations
                            .iter()
                            .all(|claim| claim.input != source.place)
                        && function
                            .content_entry_claims
                            .iter()
                            .all(|claim| claim.input.root != source.place)
                        && matches!(
                            place_kinds.get(&source.place),
                            Some(StructuralPlaceKind::Parameter { position, is_self })
                                if *position == source.position && *is_self == source.is_self
                        )
                        && direct_relevant_scalar_field(
                            structural_types,
                            source.structural_type,
                            *field,
                        ) == Some(result.scalar_type);
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
                    let source_contract = function
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
                                    O::EstablishPayloadlessCase {
                                        psi_operation,
                                        result,
                                        ..
                                    }
                                    | O::CallStructural {
                                        psi_operation,
                                        result,
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
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    field: psi_core::StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = structural_types.get(&structural_type)?;
    let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(|field_type| match field_type {
                psi_terminal::StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
                _ => None,
            })
    })
}

pub(crate) fn every_scalar_return_nominally_cleans(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> bool {
    let mut saw_return = false;
    for operation in function
        .blocks
        .iter()
        .filter_map(|block| block.nodes.last().map(|node| &node.operation))
    {
        match operation {
            O::Return {
                cleanup_actions, ..
            } => {
                saw_return = true;
                if !cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            if cleanup.place == source
                    )
                }) {
                    return false;
                }
            }
            O::ReturnUnit { .. } | O::ReturnStructural { .. } => return false,
            O::Jump { .. } | O::Conditional { .. } | O::Crash { .. } => {}
            _ => return false,
        }
    }
    saw_return
}

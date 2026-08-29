//! Per-function CFG, fact, structural-root, and provenance validation.

use super::*;

pub(crate) fn validate_function(
    function: &PsiOptimizationFunction,
    unit_entry: MachineId,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    if !valid_service_ceiling(&function.published_service_ceiling, services) {
        return Err(
            OptimizationUnitValidationError::InvalidFunctionServiceCeiling(function.machine),
        );
    }
    let (byte_sequence_literals, trivial_affine_locals) =
        validate_function_structural_catalog(function, structural_types, structural_domains)?;
    validate_provider_attachment_specialization(function, boundary_machines, structural_types)?;
    validate_structural_root_uniqueness(function)?;
    let indexed_entry_claims = function
        .entry_claim_declarations
        .iter()
        .map(|claim| claim.claim)
        .collect::<BTreeSet<_>>();
    if indexed_entry_claims.len() != function.entry_claim_declarations.len()
        || indexed_entry_claims != function.entry_claims
    {
        return Err(OptimizationUnitValidationError::EntryClaimIndexMismatch(
            function.machine,
        ));
    }
    let mut blocks = BTreeMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBlock {
                machine: function.machine,
                block: block.id,
            });
        }
    }
    if !blocks.contains_key(&function.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryBlock {
            machine: function.machine,
            block: function.entry,
        });
    }
    if !blocks[&function.entry].parameters.is_empty() {
        return Err(OptimizationUnitValidationError::EntryBlockHasParameters {
            machine: function.machine,
            block: function.entry,
        });
    }
    validate_parameter_metadata(function)?;

    let mut edge_ids = BTreeSet::new();
    let mut predecessor = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut successors = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        if block.nodes.is_empty() {
            return Err(OptimizationUnitValidationError::EmptyBlock {
                machine: function.machine,
                block: block.id,
            });
        }
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).expect("unit node index was built as u32");
            if !provenance_matches_operation(&node.operation, &node.provenance)
                || node.definitions != expected_definitions(&node.operation, block.id, node_index)
                || node.uses != expected_uses(&node.operation, block.id, node_index)
                || !successors_match_operation(&node.operation, &node.successors)
                || node.ownership != expected_ownership(&node.operation)
            {
                return Err(OptimizationUnitValidationError::OperationMetadataMismatch {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                });
            }
            let terminal = is_terminator(&node.operation);
            if terminal && index + 1 != block.nodes.len() {
                return Err(OptimizationUnitValidationError::TerminatorNotLast {
                    machine: function.machine,
                    block: block.id,
                });
            }
            for edge in &node.successors {
                if !blocks.contains_key(&edge.target) {
                    return Err(OptimizationUnitValidationError::UnknownSuccessor {
                        machine: function.machine,
                        block: block.id,
                        target: edge.target,
                    });
                }
                if !edge_ids.insert(edge.psi_edge) {
                    return Err(OptimizationUnitValidationError::DuplicateEdge(
                        edge.psi_edge,
                    ));
                }
                predecessor
                    .get_mut(&edge.target)
                    .expect("known target")
                    .insert(block.id);
                successors
                    .get_mut(&block.id)
                    .expect("every block has a successor row")
                    .push(edge.target);
            }
        }
        if !is_terminator(&block.nodes.last().expect("nonempty").operation) {
            return Err(OptimizationUnitValidationError::MissingTerminator {
                machine: function.machine,
                block: block.id,
            });
        }
    }

    validate_total_cfg(function, &blocks, &successors)?;

    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        let matches = match (operation, &function.result) {
            (
                omega_abstract_operations::AbstractOperation::Return {
                    result,
                    scalar_type,
                    ..
                },
                omega_abstract_operations::AbstractFunctionResult::Scalar(signature),
            ) => *result == signature.value && *scalar_type == signature.scalar_type,
            (
                omega_abstract_operations::AbstractOperation::ReturnUnit { .. },
                omega_abstract_operations::AbstractFunctionResult::Unit,
            )
            | (
                omega_abstract_operations::AbstractOperation::ReturnStructural { .. },
                omega_abstract_operations::AbstractFunctionResult::Structural(_),
            ) => true,
            (
                omega_abstract_operations::AbstractOperation::Return { .. }
                | omega_abstract_operations::AbstractOperation::ReturnUnit { .. }
                | omega_abstract_operations::AbstractOperation::ReturnStructural { .. },
                _,
            ) => false,
            _ => continue,
        };
        if !matches {
            return Err(OptimizationUnitValidationError::FunctionResultMismatch(
                function.machine,
            ));
        }
    }

    validate_byte_sequence_literal_witnesses(function, &byte_sequence_literals)?;
    validate_trivial_affine_local_witnesses(function, &trivial_affine_locals)?;
    validate_structural_place_availability(function, &blocks, &predecessor)?;
    validate_structural_root_operations(function, unit_entry, structural_types)?;

    validate_provenance_fuel_effects(function)?;
    validate_fact_index(function)?;
    validate_values_and_bindings(
        function,
        &blocks,
        &predecessor,
        functions,
        boundary_machines,
        services,
        structural_types,
        structural_domains,
    )?;
    validate_places_and_claims(function)?;
    current_ownership::validate_current_ownership_frontier(
        function,
        &blocks,
        &successors,
        functions,
        boundary_machines,
        structural_types,
    )?;
    Ok(())
}

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

pub(crate) fn validate_parameter_metadata(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    for (position, parameter) in function.parameters.iter().enumerate() {
        let Ok(position) = u32::try_from(position) else {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        };
        if parameter.site != ValueDefinitionSite::FunctionParameter(position) {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        }
    }
    for block in &function.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            let Ok(position) = u32::try_from(position) else {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            };
            if parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position,
                })
            {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_total_cfg(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors[&block].iter().copied());
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different block counts have an unreachable block");
        return Err(OptimizationUnitValidationError::UnreachableBlock {
            machine: function.machine,
            block,
        });
    }

    let mut indegree = blocks
        .keys()
        .copied()
        .map(|block| (block, 0usize))
        .collect::<BTreeMap<_, _>>();
    for target in successors.values().flatten() {
        *indegree.get_mut(target).expect("successor was validated") += 1;
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut visited = 0usize;
    while let Some(block) = ready.pop_first() {
        visited += 1;
        for target in &successors[&block] {
            let count = indegree.get_mut(target).expect("successor was validated");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if visited != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(OptimizationUnitValidationError::ControlCycle {
            machine: function.machine,
            block,
        });
    }
    Ok(())
}

pub(crate) fn validate_fact_index(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let expected = reconstruct_fact_index(function);
    if expected != function.facts {
        return Err(OptimizationUnitValidationError::FactIndexMismatch(
            function.machine,
        ));
    }
    Ok(())
}

/// Every executable structural root is available only after its current
/// producer. Immutable source-frontier rows do not authorize a root at a
/// rewritten site. Compressed return-tuple locals are metadata-only and have
/// no executable producer, so they are deliberately absent from this walk.
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
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            vec![*source]
        }
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

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            match &node.operation {
                O::BooleanStructuralField { source, field, .. } => {
                    let valid = function.machine == unit_entry
                        && observations
                            .iter()
                            .all(|candidate| candidate == &(*source, *field))
                        && function.content_entry_claims.is_empty()
                        && function
                            .parameters
                            .iter()
                            .any(|parameter| parameter.scalar_type == ScalarType::Boolean)
                        && function
                            .entry_claim_declarations
                            .iter()
                            .all(|claim| claim.input != *source)
                        && matches!(
                            place_kinds.get(source),
                            Some(StructuralPlaceKind::Parameter { .. })
                        )
                        && function
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == *source)
                            .is_some_and(|parameter| {
                                parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                                    && parameter.qualifications.is_empty()
                                    && parameter.access
                                        != psi_terminal::StructuralAccess::WriteOnlyBorrow
                                    && function.structural_places.iter().any(|place| {
                                        place.id == parameter.place
                                            && matches!(
                                                place.kind,
                                                StructuralPlaceKind::Parameter {
                                                    position,
                                                    is_self,
                                                } if position == parameter.position
                                                    && is_self == parameter.is_self
                                            )
                                    })
                                    && structural_types
                                        .get(&parameter.structural_type)
                                        .is_some_and(|declaration| {
                                            let psi_terminal::StructuralTypeShape::Record {
                                                fields,
                                            } = &declaration.shape
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
                            })
                        && every_scalar_return_nominally_cleans(function, *source);
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
                                        ))
                                    }
                                    _ => None,
                                })
                        });
                    if source_contract.is_none_or(
                        |(structural_type, multiplicity, qualifications)| {
                            structural_type != signature.structural_type
                                || multiplicity != signature.multiplicity
                                || qualifications != signature.qualifications.as_slice()
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

pub(crate) fn reconstruct_fact_index(function: &PsiOptimizationFunction) -> Vec<OptimizationFact> {
    use omega_abstract_operations::AbstractOperation as O;

    let mut expected = Vec::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::IntegerExactCast {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerAdd {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            _ => {}
        }
        match operation {
            O::BooleanConstant {
                psi_operation,
                result,
                value,
            } => expected.push(OptimizationFact::BooleanConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            O::IntegerConstant {
                psi_operation,
                result,
                value,
                ..
            } => expected.push(OptimizationFact::IntegerConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            _ => {}
        }
    }
    expected
}

pub(crate) fn validate_provenance_fuel_effects(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut node_provenance = BTreeMap::<PsiProvenance, Vec<(BlockId, bool)>>::new();
    let mut edge_provenance = BTreeMap::<PsiProvenance, BTreeSet<EdgeId>>::new();
    let mut edge_shapes = BTreeMap::<EdgeId, (BlockId, BlockId)>::new();
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let index = u32::try_from(index).expect("unit node index was built as u32");
            if node.provenance.is_empty() && node.successors.is_empty() {
                return Err(OptimizationUnitValidationError::IncompleteProvenance {
                    machine: function.machine,
                    block: block.id,
                    node: index,
                });
            }
            let unique_node_sources = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            if unique_node_sources.len() != node.provenance.len() {
                return Err(OptimizationUnitValidationError::DuplicateProvenance(
                    *node
                        .provenance
                        .first()
                        .expect("duplicated provenance is nonempty"),
                ));
            }
            let is_exact_terminal = node.successors.is_empty()
                && matches!(
                    node.operation,
                    O::Return { .. }
                        | O::ReturnUnit { .. }
                        | O::ReturnStructural { .. }
                        | O::Crash { .. }
                );
            for site in &node.provenance {
                if edge_provenance.contains_key(site) {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(*site));
                }
                node_provenance
                    .entry(*site)
                    .or_default()
                    .push((block.id, is_exact_terminal));
            }
            let source_sites = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            let settled_sites = node
                .fuel
                .iter()
                .map(|settlement| settlement.site)
                .collect::<BTreeSet<_>>();
            if source_sites != settled_sites
                || node.fuel.len() != node.provenance.len()
                || node
                    .fuel
                    .iter()
                    .zip(&node.provenance)
                    .any(|(settlement, source)| settlement.site != *source || settlement.units != 1)
            {
                return Err(
                    OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    },
                );
            }
            for settlement in &node.fuel {
                let _ = settlement;
            }
            for edge in &node.successors {
                edge_shapes.insert(edge.psi_edge, (block.id, edge.target));
                if edge.provenance.is_empty()
                    || edge.provenance.first() != Some(&PsiProvenance::Edge(edge.psi_edge))
                    || edge
                        .provenance
                        .iter()
                        .any(|site| !matches!(site, PsiProvenance::Edge(_)))
                {
                    return Err(OptimizationUnitValidationError::IncompleteProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    });
                }
                let source_sites = edge.provenance.iter().copied().collect::<BTreeSet<_>>();
                if source_sites.len() != edge.provenance.len()
                    || node_provenance
                        .keys()
                        .any(|site| source_sites.contains(site))
                {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(
                        *edge
                            .provenance
                            .first()
                            .expect("edge provenance is nonempty"),
                    ));
                }
                if edge.fuel.len() != edge.provenance.len()
                    || edge
                        .fuel
                        .iter()
                        .zip(&edge.provenance)
                        .any(|(settlement, source)| {
                            settlement.site != *source || settlement.units != 1
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                            machine: function.machine,
                            block: block.id,
                            node: index,
                        },
                    );
                }
                for source in &edge.provenance {
                    edge_provenance
                        .entry(*source)
                        .or_default()
                        .insert(edge.psi_edge);
                }
            }
            if node.effect.input != expected_effect || node.effect.output != expected_effect + 1 {
                return Err(OptimizationUnitValidationError::BrokenEffectChain {
                    machine: function.machine,
                    expected: expected_effect,
                    actual: node.effect.input,
                });
            }
            expected_effect += 1;
        }
    }
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .iter()
                    .flat_map(|node| node.successors.iter().map(|edge| edge.target))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (source, occurrences) in node_provenance {
        if occurrences.len() < 2 {
            continue;
        }
        if !matches!(source, PsiProvenance::Edge(_))
            || occurrences.iter().any(|(_, terminal)| !terminal)
        {
            return Err(OptimizationUnitValidationError::DuplicateProvenance(source));
        }
        for (index, (left, _)) in occurrences.iter().enumerate() {
            for (right, _) in &occurrences[index + 1..] {
                if left == right
                    || block_reaches(&successors, *left, *right)
                    || block_reaches(&successors, *right, *left)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    for (source, occurrences) in edge_provenance {
        let occurrences = occurrences.into_iter().collect::<Vec<_>>();
        for (index, left) in occurrences.iter().enumerate() {
            let (_, left_target) = edge_shapes[left];
            for right in &occurrences[index + 1..] {
                let (right_owner, right_target) = edge_shapes[right];
                let (left_owner, _) = edge_shapes[left];
                if block_reaches(&successors, left_target, right_owner)
                    || block_reaches(&successors, right_target, left_owner)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn block_reaches(
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    start: BlockId,
    target: BlockId,
) -> bool {
    let mut visited = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if visited.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    false
}

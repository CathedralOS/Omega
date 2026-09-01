use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_current_ownership_cfg(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    _successors: &BTreeMap<BlockId, Vec<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<psi_core::BoundaryMachineId, &BoundaryMachineDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    entry: CurrentOwnership,
) -> Result<(), OptimizationUnitValidationError> {
    let mut ready = BTreeSet::from([function.entry]);
    let mut incoming = BTreeMap::<BlockId, CurrentOwnership>::new();
    incoming.insert(function.entry, entry);

    while let Some(block_id) = ready.pop_first() {
        let mut frontier = incoming
            .get(&block_id)
            .expect("reachable block has a current ownership frontier")
            .clone();

        let block = blocks[&block_id];
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index =
                u32::try_from(node_index).expect("optimization-unit node position fits u32");

            if let O::BooleanStructuralField { source, .. } = node.operation
                && function
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == source)
                    .is_none_or(|parameter| {
                        parameter.multiplicity != StructuralMultiplicity::Unrestricted
                            && (!frontier.owned_places.contains_key(&source)
                                || frontier.partial_custody_paths.contains_key(&source))
                    })
            {
                return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                    machine: function.machine,
                    block: block_id,
                    node: node_index,
                    place: source,
                });
            }

            if let O::EstablishTrivialAffineLocal { place, .. } = &node.operation {
                insert_owned_result(
                    function,
                    block_id,
                    node_index,
                    &mut frontier,
                    place.id,
                    StructuralMultiplicity::Affine,
                )?;
            }
            if let O::ReturnStructural {
                trivial_affine_locals,
                ..
            } = &node.operation
            {
                for (_, place, _) in trivial_affine_locals {
                    insert_owned_result(
                        function,
                        block_id,
                        node_index,
                        &mut frontier,
                        place.id,
                        StructuralMultiplicity::Affine,
                    )?;
                }
            }

            let structural_arguments = match &node.operation {
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
                } => structural_arguments.as_slice(),
                O::CallDynamicScalar {
                    dynamic_dispatch, ..
                } => std::slice::from_ref(&dynamic_dispatch.rebound.source),
                _ => &[],
            };
            let parameter_multiplicities = match &node.operation {
                O::CallUnit { callee, .. }
                | O::CallStructuralScalar { callee, .. }
                | O::CallStructural { callee, .. } => functions[callee]
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.multiplicity)
                    .collect::<Vec<_>>(),
                O::CallDynamicScalar {
                    dynamic_dispatch, ..
                } => functions[&dynamic_dispatch.dispatch.realization]
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.multiplicity)
                    .collect::<Vec<_>>(),
                O::BoundaryCall { boundary, .. } => boundary_machines[boundary]
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.multiplicity)
                    .collect(),
                _ => Vec::new(),
            };
            let consumed_places = structural_arguments
                .iter()
                .zip(&parameter_multiplicities)
                .filter_map(|(argument, multiplicity)| {
                    (argument.path.is_empty()
                        && argument.access == StructuralAccess::Owned
                        && *multiplicity != StructuralMultiplicity::Unrestricted)
                        .then_some(argument.place)
                })
                .collect::<Vec<_>>();
            for place in &consumed_places {
                if frontier.partial_custody_paths.contains_key(place) {
                    return Err(
                        OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: *place,
                        },
                    );
                }
            }

            let transferred = match &node.operation {
                O::CallUnit {
                    claim_transfers, ..
                }
                | O::CallStructuralScalar {
                    claim_transfers, ..
                }
                | O::CallStructural {
                    claim_transfers, ..
                } => claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<Vec<_>>(),
                O::BoundaryCall {
                    completion_receipts,
                    ..
                } => completion_receipts
                    .iter()
                    .map(|receipt| receipt.claim)
                    .collect(),
                _ => Vec::new(),
            };
            for claim in transferred {
                if frontier.claims.remove(&claim).is_none() {
                    return Err(OptimizationUnitValidationError::CurrentClaimNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        claim,
                    });
                }
            }
            for (argument, _) in structural_arguments
                .iter()
                .zip(&parameter_multiplicities)
                .filter(|(argument, multiplicity)| {
                    argument.access == StructuralAccess::Owned
                        && **multiplicity != StructuralMultiplicity::Unrestricted
                })
            {
                if !frontier.owned_places.contains_key(&argument.place) {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place: argument.place,
                    });
                }
                if argument.path.is_empty()
                    && frontier.partial_custody_paths.contains_key(&argument.place)
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: argument.place,
                        },
                    );
                }
                if !argument.path.is_empty()
                    && frontier
                        .partial_custody_paths
                        .get(&argument.place)
                        .is_some_and(|moved| {
                            moved.iter().any(|existing| {
                                existing.starts_with(&argument.path)
                                    || argument.path.starts_with(existing)
                            })
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentProjectedMoveOverlap {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: argument.place,
                        },
                    );
                }
            }
            for place in consumed_places {
                if frontier.owned_places.remove(&place).is_none() {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place,
                    });
                }
            }

            for argument in structural_arguments.iter().filter(|argument| {
                !argument.path.is_empty() && argument.access == StructuralAccess::Owned
            }) {
                if !frontier.owned_places.contains_key(&argument.place) {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place: argument.place,
                    });
                }
                let moved = frontier
                    .partial_custody_paths
                    .entry(argument.place)
                    .or_default();
                if moved.iter().any(|existing| {
                    existing.starts_with(&argument.path) || argument.path.starts_with(existing)
                }) || !moved.insert(argument.path.clone())
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentProjectedMoveOverlap {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: argument.place,
                        },
                    );
                }
                if projected_fixed_array_root_is_fully_consumed(
                    function,
                    structural_types,
                    &frontier,
                    argument.place,
                ) {
                    frontier.owned_places.remove(&argument.place);
                    frontier.partial_custody_paths.remove(&argument.place);
                }
            }

            let structural_result = match &node.operation {
                O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
                    Some(result)
                }
                _ => None,
            };
            if let Some(result) = structural_result {
                insert_owned_result(
                    function,
                    block_id,
                    node_index,
                    &mut frontier,
                    result.place,
                    result.multiplicity,
                )?;
                for binding in &result.claims {
                    let claim = LiveClaim {
                        input: Some(result.place),
                        path: binding.path.clone(),
                        multiplicity: Some(if binding.path.is_empty() {
                            result.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    };
                    if frontier.claims.insert(binding.claim, claim).is_some() {
                        return Err(OptimizationUnitValidationError::CurrentClaimAlreadyLive {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            claim: binding.claim,
                        });
                    }
                }
            }

            match &node.operation {
                O::Return {
                    cleanup_actions, ..
                } => {
                    reject_live_linear_claim(function, block_id, &frontier)?;
                    validate_scalar_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        block_id,
                        &frontier,
                        cleanup_actions,
                    )?;
                }
                O::ReturnUnit {
                    cleanup_actions, ..
                } => {
                    reject_live_linear_claim(function, block_id, &frontier)?;
                    validate_unit_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        block_id,
                        &frontier,
                        cleanup_actions,
                    )?;
                }
                O::ReturnStructural {
                    source,
                    returned_claims,
                    trivial_affine_discards,
                    ..
                } => {
                    if frontier.partial_custody_paths.contains_key(source) {
                        return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnSourcePartiallyMoved {
                                machine: function.machine,
                                block: block_id,
                                place: *source,
                            },
                        );
                    }
                    if frontier.owned_places.remove(source).is_none() {
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: *source,
                        });
                    }
                    let expected = frontier
                        .claims
                        .iter()
                        .filter_map(|(claim, live)| (live.input == Some(*source)).then_some(*claim))
                        .collect::<Vec<_>>();
                    if returned_claims != &expected {
                        return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
                    for claim in returned_claims {
                        frontier.claims.remove(claim);
                    }
                    if trivial_affine_discards
                        != &expected_trivial_affine_discards(function, &frontier)
                    {
                        return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                            machine: function.machine,
                            block: block_id,
                        });
                    }
                    if let Some(claim) = frontier.claims.keys().next().copied() {
                        return Err(
                            OptimizationUnitValidationError::CurrentClaimLiveAfterStructuralReturn {
                                machine: function.machine,
                                block: block_id,
                                claim,
                            },
                        );
                    }
                }
                O::Crash {
                    frontier_lower_bound,
                    ..
                } => {
                    if frontier_lower_bound != &frontier.claims.keys().copied().collect::<Vec<_>>()
                    {
                        return Err(
                            OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        for edge in &block
            .nodes
            .last()
            .expect("validated block is nonempty")
            .successors
        {
            let mut outgoing = frontier.clone();
            apply_edge_trivial_affine_discards(
                function,
                block_id,
                &mut outgoing,
                &edge.trivial_affine_discards,
            )?;
            if let Some(existing) = incoming.get(&edge.target) {
                if existing.claims != outgoing.claims {
                    return Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch {
                        machine: function.machine,
                        block: edge.target,
                    });
                }
                if existing.owned_places != outgoing.owned_places
                    || existing.partial_custody_paths != outgoing.partial_custody_paths
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentOwnedPlaceJoinMismatch {
                            machine: function.machine,
                            block: edge.target,
                        },
                    );
                }
            } else {
                incoming.insert(edge.target, outgoing);
                ready.insert(edge.target);
            }
        }
    }

    if let Some(block) = blocks.keys().find(|block| !incoming.contains_key(block)) {
        return Err(OptimizationUnitValidationError::UnreachableBlock {
            machine: function.machine,
            block: *block,
        });
    }

    Ok(())
}

//! Bounded final internal structural-result call lowering.

use super::*;

pub(super) fn lower_structural_call_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralCallReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let target_plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .for_machine(plan.call.target_machine)
        .ok_or(LoweringError::Unsupported(
            "structural call target has no exact structural-return plan",
        ))?;
    if target_plan.state != plan.call.target_state
        || target_plan.structural_parameters.len() != 1
        || !target_plan.trivial_affine_locals.is_empty()
        || !target_plan.trivial_affine_local_discard_ordinals.is_empty()
        || !target_plan.trivial_affine_discards.is_empty()
        || plan.structural_parameters.len() != 1
        || plan.call.structural_arguments.len() != 1
        || plan.call.claim_transfers.len() != 1
        || plan.call.structural_arguments[0].source_parameter_index != 0
        || !plan.call.structural_arguments[0].path.is_empty()
        || plan.call.structural_arguments[0]
            .byte_sequence_literal
            .is_some()
        || plan.call.claim_transfers[0].argument_index != 0
        || plan.call.claim_transfers[0].claim_identity != plan.returned_claim
        || plan.entry_claim.claim_identity != plan.returned_claim
        || plan.call.callee_returned_claim != target_plan.transferred_claim
        || plan.result.multiplicity != Multiplicity::Linear
        || target_plan.result != plan.result
    {
        return unsupported(
            "structural call is not one final exact whole-root linear identity transfer",
        );
    }

    // Reuse the already-closed callee producer to obtain one canonical shared
    // structural type/domain catalog. The bounded caller is then assembled
    // into that module; two independently lowered modules are never spliced.
    let mut lowered = lower_structural_return_machine(checked, target_plan)?;
    let module = &mut lowered.semantic_module;
    let [target] = module.machines.as_mut_slice() else {
        return unsupported("structural call target did not lower to one machine");
    };
    if target.blocks.len() != 1 || !target.blocks[0].operations.is_empty() {
        return unsupported("structural call target is not the bounded identity body");
    }
    target.id = machine_id(2);
    target.entry = block_id(2);
    target.blocks[0].id = block_id(2);
    match &mut target.blocks[0].terminator {
        Terminator::ReturnStructural { edge, .. } => *edge = edge_id(2),
        _ => return unsupported("structural call target has no structural return"),
    }
    target.contract.id = contract_id(2);
    let target_machine = target.id;
    let target_claim = target.entry_claims[0].claim;

    let type_ids = module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let attachment = lookup_type_id(&type_ids, &plan.attachment_type_identity)?;
    let result_type = lookup_type_id(&type_ids, &plan.result.type_identity)?;
    let domain_ids = plan
        .result
        .qualifications
        .iter()
        .map(|source| {
            module
                .structural_domains
                .iter()
                .find_map(|domain| {
                    (domain.semantic_domain.get() == u64::from(source.0)).then_some(domain.id)
                })
                .ok_or(LoweringError::Unsupported(
                    "structural call result references an unlowered domain",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if domain_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return unsupported("structural call result domains are not canonical");
    }

    // Structural place identities are module-wide. The one-machine callee
    // owns the historical input/result ids, so the caller starts after its
    // dense input id and chooses fresh ordinary ids for both of its results.
    let mut next_place = 2_u64;
    let parameters = lower_unit_parameters(
        &plan.structural_parameters,
        &type_ids,
        &plan
            .result
            .qualifications
            .iter()
            .copied()
            .zip(domain_ids.iter().copied())
            .collect::<Vec<_>>(),
        &mut next_place,
    )?;
    let [input] = parameters.as_slice() else {
        return unsupported("structural call caller has no exact input");
    };
    let input_place = input.place;
    let input_is_self = input.is_self;
    let operation = operation_id(1);
    let operation_result_place = place_id(allocate_dense(&mut next_place)?);
    let machine_result_place = place_id(allocate_dense(&mut next_place)?);
    let claim = claim_id(1);

    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == plan.machine && fact.state_symbol == plan.state)
        .cloned()
        .collect::<Vec<_>>();
    let mut identity = lower_content_identity_reshuffles(&identity_facts)?;
    for place in &mut identity.structural_places {
        if place.id == place_id(1) {
            place.id = input_place;
        } else if place.id == place_id(RESULT_STRUCTURAL_PLACE_ID) {
            place.id = machine_result_place;
        }
    }
    for entry in &mut identity.entry_claims {
        if entry.input.root == place_id(1) {
            entry.input.root = input_place;
        }
    }
    for reshuffle in &mut identity.reshuffles {
        if reshuffle.input.root == place_id(1) {
            reshuffle.input.root = input_place;
        }
        if reshuffle.output.root == place_id(RESULT_STRUCTURAL_PLACE_ID) {
            reshuffle.output.root = machine_result_place;
        }
    }
    let [(source_claim, content_claim)] = identity.source_claims.as_slice() else {
        return unsupported("structural call requires one exact identity reshuffle claim");
    };
    let [content_entry] = identity.entry_claims.as_slice() else {
        return unsupported("structural call requires one content entry binding");
    };
    let [reshuffle] = identity.reshuffles.as_slice() else {
        return unsupported("structural call requires one content identity reshuffle");
    };
    if *source_claim != plan.returned_claim
        || *content_claim != claim
        || content_entry.claim != claim
        || reshuffle.claim != claim
        || content_entry.input.root != input_place
        || reshuffle.input.root != input_place
        || reshuffle.output.root != machine_result_place
        || !content_entry.input.segments.is_empty()
        || !reshuffle.input.segments.is_empty()
        || !reshuffle.output.segments.is_empty()
    {
        return unsupported("structural call claim/content identities do not unify exactly");
    }

    let content_places = BTreeMap::from([
        (
            input_place,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: input_is_self,
            },
        ),
        (machine_result_place, StructuralPlaceKind::Result),
    ]);
    if identity
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>()
        != content_places
    {
        return unsupported("structural call content roots do not match the checked signature");
    }

    let structural_arguments =
        lower_structural_arguments(&plan.call.structural_arguments, &parameters, &[])?;
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: Some(attachment),
        parameters: Vec::new(),
        structural_parameters: parameters,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: machine_result_place,
            structural_type: result_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: domain_ids.clone(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: input_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: input_is_self,
                },
            },
            StructuralPlaceDeclaration {
                id: operation_result_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: result_type,
                },
            },
            StructuralPlaceDeclaration {
                id: machine_result_place,
                kind: StructuralPlaceKind::Result,
            },
        ],
        entry_claims: vec![EntryClaim {
            claim,
            input: input_place,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: identity.entry_claims,
        content_identity_reshuffles: identity.reshuffles,
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation,
                result: psi_terminal::OperationResult::Structural(
                    psi_terminal::StructuralOperationResult {
                        place: operation_result_place,
                        structural_type: result_type,
                        multiplicity: StructuralMultiplicity::Linear,
                        qualifications: domain_ids,
                        claims: vec![psi_terminal::StructuralResultClaimBinding {
                            claim,
                            path: Vec::new(),
                        }],
                    },
                ),
                kind: OperationKind::CallStructural {
                    callee: target_machine,
                    structural_arguments,
                    claim_transfers: vec![ClaimTransfer {
                        claim,
                        argument_index: 0,
                    }],
                    returned_claim_transfers: vec![psi_terminal::StructuralResultClaimTransfer {
                        callee_claim: target_claim,
                        caller_claim: claim,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: operation_result_place,
                returned_claims: vec![claim],
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };
    module.entry = caller.id;
    module.machines.insert(0, caller);
    Ok(lowered)
}

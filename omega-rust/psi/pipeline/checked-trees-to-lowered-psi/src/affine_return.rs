//! Whole owned-affine identity returns, shared by ordinary and selected machines.

use super::*;
use crate::attached_unit::lower_unit_parameters;

pub(super) fn lower_affine_return_machine(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
) -> Result<LoweredPsi, LoweringError> {
    let plans = &checked.facts.flow.terminal_structural_returns;
    let terminal_machine = machine_id(1);
    let mut semantic_module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: terminal_machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: Vec::new(),
    };
    let plan =
        plans
            .claim_free_affine_for_machine(source_machine)
            .ok_or(LoweringError::Unsupported(
                "affine identity has no checked return plan",
            ))?;
    retain_additional_structural_types(
        &mut semantic_module,
        &plans.structural_types,
        plan.attachment_type_identity.iter().cloned().chain([
            plan.structural_parameter.type_identity.clone(),
            plan.result.type_identity.clone(),
        ]),
    )?;
    let type_ids = semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    semantic_module.machines = lower_claim_free_affine_return_machines(
        checked,
        &[source_machine],
        &semantic_module.structural_types,
        &type_ids,
        &[(source_machine, terminal_machine)],
        0,
    )?;
    Ok(LoweredPsi {
        semantic_module,
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

pub(super) fn lower_claim_free_affine_return_machines(
    checked: &CheckedTrees,
    roots: &[symbols::SymbolHandle],
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(symbols::SymbolHandle, MachineId)],
    machine_index_base: usize,
) -> Result<Vec<TerminalMachine>, LoweringError> {
    let plans = &checked.facts.flow.terminal_structural_returns;
    let mut machines = Vec::with_capacity(roots.len());
    for (index, source_machine) in roots.iter().enumerate() {
        let realizations = plans
            .claim_free_affine_machines
            .iter()
            .filter(|plan| plan.machine == *source_machine)
            .collect::<Vec<_>>();
        let [realization] = realizations.as_slice() else {
            return unsupported(
                "selected structural-result closure does not contain one exact checked realization",
            );
        };
        validate_parameter_partition(checked, realization)?;
        let mut parameter_positions = realization
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>();
        parameter_positions.push(realization.structural_parameter.position);
        parameter_positions.sort_unstable();
        if !realization.machine.is_valid()
            || !realization.state.is_valid()
            || parameter_positions
                .iter()
                .enumerate()
                .any(|(position, source)| u32::try_from(position).ok() != Some(*source))
            || realization.structural_parameter.is_self
            || realization.structural_parameter.multiplicity != Multiplicity::Affine
            || realization.structural_parameter.access
                != checked_trees::CheckedStructuralAccess::Owned
            || !realization.structural_parameter.qualifications.is_empty()
            || realization
                .structural_parameter
                .fused_service_erasure
                .is_some()
            || realization
                .scalar_parameters
                .windows(2)
                .any(|parameters| parameters[0].source_position >= parameters[1].source_position)
            || realization.result.multiplicity != Multiplicity::Affine
            || !realization.result.qualifications.is_empty()
            || realization.result.type_identity != realization.structural_parameter.type_identity
            || realization.return_statement_ordinal != 0
        {
            return unsupported(
                "affine identity return has an invalid signature or result transfer",
            );
        }
        let machine_index =
            machine_index_base
                .checked_add(index)
                .ok_or(LoweringError::Unsupported(
                    "selected structural-result machine count overflows usize",
                ))?;
        let identity_base = u64::try_from(machine_index)
            .map_err(|_| {
                LoweringError::Unsupported("selected structural-result machine count exceeds u64")
            })?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "selected structural-result identity range overflows",
            ))?;
        let terminal_machine = lookup_machine_id(machine_ids, *source_machine)?;
        let mut next_value = identity_base
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "selected structural-result value identity range overflows",
            ))?;
        let scalar_parameters = realization
            .scalar_parameters
            .iter()
            .map(|parameter| {
                Ok(ValueDeclaration {
                    id: value_id(allocate_dense(&mut next_value)?),
                    scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let mut next_place = identity_base
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "selected structural-result place identity range overflows",
            ))?;
        let structural_parameters = lower_unit_parameters(
            std::slice::from_ref(&realization.structural_parameter),
            type_ids,
            &[],
            &mut next_place,
        )?;
        let [structural_parameter] = structural_parameters.as_slice() else {
            unreachable!("one checked structural parameter lowers to one Terminal parameter")
        };
        let structural_parameter_place = structural_parameter.place;
        let structural_parameter_position = structural_parameter.position;
        let result_place = place_id(allocate_dense(&mut next_place)?);
        let result_type = lookup_type_id(type_ids, &realization.result.type_identity)?;
        if structural_parameter.structural_type != result_type
            || structural_types
                .iter()
                .filter(|declaration| declaration.id == result_type)
                .count()
                != 1
        {
            return unsupported(
                "selected structural-result realization lost its exact structural type",
            );
        }
        let block = block_id(
            identity_base
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "selected structural-result block identity range overflows",
                ))?,
        );
        machines.push(TerminalMachine {
            id: terminal_machine,
            attachment: realization
                .attachment_type_identity
                .as_deref()
                .map(|identity| lookup_type_id(type_ids, identity))
                .transpose()?,
            parameters: scalar_parameters,
            structural_parameters,
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: result_place,
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: structural_parameter_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: structural_parameter_position,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnStructural {
                    edge: edge_id(identity_base.checked_add(1).ok_or(
                        LoweringError::Unsupported(
                            "selected structural-result edge identity range overflows",
                        ),
                    )?),
                    source: structural_parameter_place,
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(terminal_machine.get()),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        });
    }
    Ok(machines)
}

/// Rejoin the authored partition before Terminal assigns separate dense scalar
/// and structural parameter positions. A coordinated position swap is not a
/// different spelling of the same signature.
fn validate_parameter_partition(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedClaimFreeAffineStructuralReturnMachinePlan,
) -> Result<(), LoweringError> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .ok_or(LoweringError::Unsupported(
            "affine identity has no exact typed machine",
        ))?;
    let state = checked
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == plan.state)
        .ok_or(LoweringError::Unsupported(
            "affine identity has no exact typed state",
        ))?;
    let parameters = checked.state_parameters(state);
    if parameters.len() != plan.scalar_parameters.len() + 1 {
        return unsupported(
            "affine identity parameter partition does not cover its source signature",
        );
    }
    let source = parameters
        .get(plan.structural_parameter.position as usize)
        .ok_or(LoweringError::Unsupported(
            "affine identity source position is out of range",
        ))?;
    let [checked_trees::statement::StatementNode::Expression(expression)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        return unsupported("affine identity source body is not one return expression");
    };
    let checked_trees::expression::ExpressionNode::Name(path) =
        checked.expression_table.expression(*expression)
    else {
        return unsupported("affine identity source does not return its owned parameter");
    };
    if checked.machine_states(machine).len() != 1
        || !checked.machine_contracts(machine).is_empty()
        || !checked.state_contracts(state).is_empty()
        || path.symbol != source.symbol
        || path.head_symbol != source.symbol
        || checked
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || checked
            .typed
            .normalized_type_identity(source.type_reference)
            .into_string()
            != plan.structural_parameter.type_identity
        || checked
            .typed
            .normalized_type_identity(state.return_type)
            .into_string()
            != plan.result.type_identity
    {
        return unsupported("affine identity return disagrees with its authored source");
    }
    let flows = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, flow)| flow.machine_symbol == plan.machine && flow.state_symbol == plan.state)
        .map(|(_, flow)| flow)
        .collect::<Vec<_>>();
    let [flow] = flows.as_slice() else {
        return unsupported("affine identity has no exact checked source flow");
    };
    if !checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .is_empty()
        || !checked
            .facts
            .service_reaches
            .rows
            .services(flow.service_reach.direct)
            .is_empty()
        || !checked
            .facts
            .service_reaches
            .rows
            .services(flow.service_reach.transitive)
            .is_empty()
    {
        return unsupported("affine identity source has calls or service effects");
    }
    for (position, source) in parameters.iter().enumerate() {
        let primitive = checked.primitive_type_reference(source.type_reference);
        if position == plan.structural_parameter.position as usize {
            if primitive.is_some() || source.is_self || source.is_const {
                return unsupported(
                    "affine identity structural position is not an owned source value",
                );
            }
        } else {
            let matches = plan
                .scalar_parameters
                .iter()
                .filter(|parameter| parameter.source_position as usize == position)
                .collect::<Vec<_>>();
            if source.is_self
                || source.is_const
                || source.is_mutable
                || !matches!(matches.as_slice(), [parameter] if Some(parameter.primitive_type) == primitive)
            {
                return unsupported(
                    "affine identity scalar parameters do not rejoin the source partition",
                );
            }
        }
    }
    Ok(())
}

//! Reachable scalar-call closure discovery and terminal module assembly.

use super::*;

pub(crate) mod callee;
pub(crate) mod embedded;

pub(super) fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let mut closure = vec![entry];
    let mut authorized_static_scalar_realizations = Vec::new();
    let mut next = 0usize;
    while let Some(machine) = closure.get(next).copied() {
        next += 1;
        let selection = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.machine == machine)
            .ok_or(LoweringError::Unsupported(
                "direct scalar call target has no checked terminal selection",
            ))?;
        if selection.signature != CheckedTerminalSignatureEligibility::Eligible
            && !(selection.signature == CheckedTerminalSignatureEligibility::Attached
                && authorized_static_scalar_realizations.contains(&machine))
        {
            return unsupported("direct scalar call target has an unsupported terminal signature");
        }
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct scalar call target has no checked scalar graph",
            ))?;
        let computation_targets =
            scalar_graph_lowering::checked_scalar_computation_call_targets(checked, machine)?;
        for (target, authorized_static_scalar) in graph
            .states
            .iter()
            .flat_map(|state| {
                state.bindings.iter().filter_map(|binding| {
                    let CheckedScalarBindingValue::DirectCall { target_machine, .. } =
                        &binding.value
                    else {
                        return None;
                    };
                    Some((
                        *target_machine,
                        bounded_static_scalar_dispatch_edge(checked, machine, state.state, binding),
                    ))
                })
            })
            .chain(
                computation_targets
                    .into_iter()
                    .map(|target| (target, false)),
            )
        {
            let target_selection = checked
                .facts
                .flow
                .terminal_machines
                .machines
                .iter()
                .find(|selection| selection.machine == target)
                .ok_or(LoweringError::Unsupported(
                    "direct scalar call target has no checked terminal selection",
                ))?;
            if target_selection.signature == CheckedTerminalSignatureEligibility::Attached
                && !authorized_static_scalar
            {
                return unsupported(
                    "attached scalar call target is not an exact bounded static scalar realization",
                );
            }
            if authorized_static_scalar && !authorized_static_scalar_realizations.contains(&target)
            {
                authorized_static_scalar_realizations.push(target);
            }
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

/// The bounded scalar named-witness rung is the sole exception that may pull
/// an attached static realization into a free scalar caller's closure. Rejoin
/// the exact proof-output call coordinate here so an unrelated proof row cannot
/// grant scalar eligibility to another attached machine.
fn bounded_static_scalar_dispatch_edge(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    binding: &checked_trees::CheckedScalarBinding,
) -> bool {
    let CheckedScalarBindingValue::DirectCall {
        target_machine,
        target_state,
        call_ordinal,
        argument_count,
    } = &binding.value
    else {
        return false;
    };
    if *argument_count != 0 {
        return false;
    }
    let Some(runtime_statement) = usize::try_from(binding.statement_ordinal).ok() else {
        return false;
    };
    let Some(runtime_call_ordinal) = usize::try_from(*call_ordinal).ok() else {
        return false;
    };
    let mut invocations =
        checked
            .facts
            .proof
            .proof_output_calls
            .iter()
            .filter_map(|(_, invocation)| {
                let runtime_call = invocation.runtime_call?;
                let dispatch = invocation.static_requirement_dispatch?;
                (invocation.caller_machine_symbol == caller_machine
                    && invocation.caller_state_symbol == caller_state
                    && runtime_call.statement_index == runtime_statement
                    && runtime_call.call_ordinal == runtime_call_ordinal
                    && invocation.target_machine_symbol == *target_machine
                    && invocation.target_state_symbol == *target_state
                    && dispatch.realization_machine == *target_machine
                    && dispatch.realization_state == *target_state)
                    .then_some((invocation, dispatch))
            });
    let Some((_invocation, dispatch)) = invocations.next() else {
        return false;
    };
    if invocations.next().is_some() {
        return false;
    }

    let caller_is_free = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller_machine)
        .is_some_and(|machine| machine.attached_data.is_none());
    let realization_is_attached_checked_body = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == *target_machine)
        .is_some_and(|machine| {
            machine.attached_data.is_some()
                && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && checked.typed.machine_states(machine).len() == 1
        });
    let realization_result = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(*target_machine)
        .and_then(|graph| {
            let [state] = graph.states.as_slice() else {
                return None;
            };
            (state.state == *target_state && state.parameter_types.is_empty())
                .then_some(state.result_type)
        })
        .filter(|result| matches!(result, PrimitiveType::I32 | PrimitiveType::Bool));
    let requirement_result = checked
        .typed
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == dispatch.declaring_trait)
        .and_then(|trait_definition| {
            checked
                .typed
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|requirement| requirement.symbol == dispatch.requirement)
        })
        .and_then(|requirement| {
            checked
                .typed
                .state_signature_parameters(requirement)
                .is_empty()
                .then(|| {
                    checked
                        .typed
                        .primitive_type_reference(requirement.return_type)
                })
                .flatten()
        })
        .filter(|result| matches!(result, PrimitiveType::I32 | PrimitiveType::Bool));

    caller_is_free
        && realization_is_attached_checked_body
        && realization_result.is_some()
        && realization_result == requirement_result
}

pub(super) fn lower_scalar_call_closure(
    checked: &CheckedTrees,
    closure: &[symbols::SymbolHandle],
) -> Result<LoweredPsi, LoweringError> {
    let prepared = closure
        .iter()
        .map(|machine| {
            let graph = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(*machine)
                .ok_or(LoweringError::Unsupported(
                    "terminal call-closure machine has no checked scalar graph",
                ))?;
            prepare_scalar_graph_machine(checked, *machine, graph)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if prepared.iter().any(|machine| {
        !machine.identity_reshuffles.structural_places.is_empty()
            || !machine.identity_reshuffles.entry_claims.is_empty()
            || !machine.identity_reshuffles.reshuffles.is_empty()
            || !machine.partition_compositions.structural_places.is_empty()
            || !machine.partition_compositions.compositions.is_empty()
    }) {
        return unsupported(
            "structural/content call effects require the terminal content-call slice",
        );
    }
    let machine_ids = prepared
        .iter()
        .enumerate()
        .map(|(index, machine)| {
            Ok((
                machine.source_machine,
                machine_id(
                    u64::try_from(index)
                        .map_err(|_| {
                            LoweringError::Unsupported("terminal call closure exceeds u64")
                        })?
                        .checked_add(1)
                        .expect("terminal machine identities are one-based"),
                ),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let requirement_counts = prepared
        .iter()
        .map(|machine| (machine.source_machine, machine.contract.requirement_count()))
        .collect::<Vec<_>>();
    let mut machines = Vec::with_capacity(prepared.len());
    let mut evidence = Vec::new();
    let mut source_call_occurrences = Vec::new();
    let mut selected_ieee_float_fma_occurrences = Vec::new();
    for (index, machine) in prepared.into_iter().enumerate() {
        let terminal_machine = machine_ids[index].1;
        let identity_base = u64::try_from(index)
            .map_err(|_| LoweringError::Unsupported("terminal call closure exceeds u64"))?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "terminal call closure identity range overflows",
            ))?;
        let mut lowered = build_scalar_graph_module(
            &machine.states,
            machine.result_type,
            machine.contract,
            machine.crash_routes,
            machine.identity_reshuffles,
            machine.partition_compositions,
            terminal_machine,
            identity_base,
            &machine_ids,
            &requirement_counts,
        )?;
        let [terminal_machine] = lowered.semantic_module.machines.as_slice() else {
            unreachable!("one prepared scalar graph emits one terminal machine")
        };
        machines.push(terminal_machine.clone());
        evidence.append(&mut lowered.proof_bundle.evidence);
        source_call_occurrences.append(&mut lowered.source_call_occurrences);
        selected_ieee_float_fma_occurrences
            .append(&mut lowered.selected_ieee_float_fma_occurrences);
    }
    let mut lowered = LoweredPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine_id(1),
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
            machines,
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

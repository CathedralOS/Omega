//! Reachable scalar-call closure discovery and terminal module assembly.

use super::{
    CheckedScalarBindingValue, CheckedTerminalSignatureEligibility, CheckedTrees, LoweredPsi,
    LoweringError, PrimitiveType, ProofBundle, TERMINAL_MACHINE_IDENTITY_STRIDE, TerminalModule,
    build_scalar_graph_module, machine_id, prepare_scalar_graph_machine, prepare_scalar_graph_root,
    scalar_graph_lowering, unsupported,
};
pub(crate) mod callee;
pub(crate) mod embedded;

/// Scalar bodies share the ordinary catalog whenever their closure needs places
/// or services. A selected callback's published reach is a contract contribution
/// even when its body only returns a scalar; the pure assembler has no service
/// namespace and must not erase that contribution.
pub(crate) fn requires_shared_catalog(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<bool, LoweringError> {
    let mut pending = vec![entry];
    let mut visited = Vec::new();
    while let Some(machine) = pending.pop() {
        if visited.contains(&machine) {
            continue;
        }
        visited.push(machine);
        let mut reaches = checked
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|reach| reach.machine == machine);
        let reach = reaches.next().ok_or(LoweringError::Unsupported(
            "scalar call closure lost its checked service contract",
        ))?;
        if reaches.next().is_some() {
            return unsupported("scalar call closure has ambiguous service contracts");
        }
        if !checked
            .facts
            .service_reaches
            .rows
            .services(reach.effective)
            .is_empty()
            || !reach.unresolved_installation_reaches.is_empty()
        {
            return Ok(true);
        }
        let Some(graph) = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
        else {
            // A graph-less scalar helper may own an ordered operation body.
            // Route the complete closure to the shared assembler before the
            // graph-only catalog can discard that body's structural effects.
            if matches!(
                callee::CheckedScalarCallee::find_for_unit_call(checked, machine),
                Ok(callee::CheckedScalarCallee::Operations(_))
            ) {
                return Ok(true);
            }
            continue;
        };
        if graph.states.iter().any(|state| {
            !state.structural_parameters.is_empty()
                || !state.primitive_locals.is_empty()
                || !state.unit_operations.is_empty()
        }) || !crate::expression_preparation::computation_graph::structural_call_targets(
            checked, machine,
        )?
        .is_empty()
            || !crate::scalar_graph::scalar_computations::cases::type_roots(checked, machine)?
                .is_empty()
        {
            // Computation-owned constructors require the shared namespace even
            // when neither this machine nor its callers have structural formals.
            return Ok(true);
        }
        pending.extend(
            scalar_graph_lowering::checked_scalar_computation_call_targets(checked, machine)?,
        );
        pending.extend(
            graph
                .states
                .iter()
                .flat_map(|state| &state.bindings)
                .filter_map(|binding| {
                    if let CheckedScalarBindingValue::DirectCall { target_machine, .. } =
                        binding.value
                    {
                        Some(target_machine)
                    } else {
                        None
                    }
                }),
        );
    }
    Ok(false)
}

pub(crate) fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let mut closure = vec![entry];
    let mut authorized_static_scalar_callees = Vec::new();
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
                && authorized_static_scalar_callees.contains(&machine))
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
                    .map(|target| (target, receiver_free_checked_body(checked, target))),
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
                    "attached scalar call target has no checked receiver-free call edge",
                );
            }
            if authorized_static_scalar && !authorized_static_scalar_callees.contains(&target) {
                authorized_static_scalar_callees.push(target);
            }
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

/// Attachment names a declaration owner, not necessarily a runtime receiver.
/// Ordinary computed calls use the same scalar graph and independent source-call
/// custody as free helpers. That path rejects named proof-output calls, which
/// still need the exact dispatch coordinate checked below. Keep the declaration's
/// Attached classification: structural consumers also use its nominal owner.
fn receiver_free_checked_body(checked: &CheckedTrees, target: symbols::SymbolHandle) -> bool {
    checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == target)
        .is_some_and(|machine| {
            machine.supply_mode.is_checked_body()
                && machine.body_is_present
                && !machine.structural_type_equations_pending
                && machine.type_parameters.is_empty()
                && machine.owned_data.is_empty()
                && !machine.suspends
                && !machine.blocks
                && machine.invokes.is_empty()
                && checked.typed.machine_states(machine).iter().all(|state| {
                    checked
                        .typed
                        .state_parameters(state)
                        .iter()
                        .all(|parameter| !parameter.is_self)
                })
        })
}

/// An attached named-witness realization needs more than body eligibility. Rejoin
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

/// Lower the selected root's scalar call closure (`checked_scalar_call_closure`,
/// root first) into one Terminal module. This is the one route for a
/// graph-only scalar selection. A root that calls no other machine is a
/// closure of one, not a separate family.
pub(crate) fn lower_scalar_call_closure(
    checked: &CheckedTrees,
    closure: &[symbols::SymbolHandle],
) -> Result<LoweredPsi, LoweringError> {
    let qualifications =
        crate::expression_preparation::qualifications::PreparedScalarQualifications::prepare(
            checked, closure,
        )?;
    let prepared = closure
        .iter()
        .enumerate()
        .map(|(index, machine)| {
            let graph = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(*machine)
                .ok_or(LoweringError::Unsupported(
                    "terminal call-closure machine has no checked scalar graph",
                ))?;
            if index == 0 {
                prepare_scalar_graph_root(checked, &qualifications, *machine, graph)
            } else {
                prepare_scalar_graph_machine(checked, &qualifications, *machine, graph)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    // A call between two closure members carries no content-transfer rows in
    // this assembler, so a member's content effects are admitted only while
    // no such call exists. Every member after the root entered the closure
    // through a call from another member, so that holds exactly for a
    // closure of one.
    if closure.len() > 1
        && prepared.iter().any(|machine| {
            !machine.identity_reshuffles.structural_places.is_empty()
                || !machine.identity_reshuffles.entry_claims.is_empty()
                || !machine.identity_reshuffles.reshuffles.is_empty()
                || !machine.partition_compositions.structural_places.is_empty()
                || !machine.partition_compositions.compositions.is_empty()
        })
    {
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
    let mut selected_ieee_float_comparison_occurrences = Vec::new();
    let mut selected_integer_comparison_occurrences = Vec::new();
    let mut scalar_qualifications = qualifications.catalog().clone();
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
            &machine.state_symbols,
            machine.result_type,
            &machine.scalar_qualifications,
            machine.contract,
            machine.crash_routes,
            machine.identity_reshuffles,
            machine.partition_compositions,
            terminal_machine,
            identity_base,
            &machine_ids,
            &requirement_counts,
            machine.loop_plan.as_ref(),
        )?;
        if lowered.semantic_module.scalar_qualifications.domains != scalar_qualifications.domains
            || lowered.semantic_module.scalar_qualifications.sets != scalar_qualifications.sets
        {
            return unsupported("scalar call closure changed its shared qualification namespace");
        }
        scalar_qualifications
            .coercions
            .append(&mut lowered.semantic_module.scalar_qualifications.coercions);
        // Floating and integer entry ranges are machine-local rows keyed by
        // the emitted machine and parameter identities: they merge without
        // sharing the domain/set namespace.
        scalar_qualifications.float_entry_ranges.append(
            &mut lowered
                .semantic_module
                .scalar_qualifications
                .float_entry_ranges,
        );
        scalar_qualifications.integer_entry_ranges.append(
            &mut lowered
                .semantic_module
                .scalar_qualifications
                .integer_entry_ranges,
        );
        let [terminal_machine] = lowered.semantic_module.machines.as_slice() else {
            unreachable!("one prepared scalar graph emits one terminal machine")
        };
        machines.push(terminal_machine.clone());
        evidence.append(&mut lowered.proof_bundle.evidence);
        source_call_occurrences.append(&mut lowered.source_call_occurrences);
        selected_ieee_float_fma_occurrences
            .append(&mut lowered.selected_ieee_float_fma_occurrences);
        selected_ieee_float_comparison_occurrences
            .append(&mut lowered.selected_ieee_float_comparison_occurrences);
        selected_integer_comparison_occurrences
            .append(&mut lowered.selected_integer_comparison_occurrences);
    }
    scalar_qualifications
        .float_entry_ranges
        .sort_by_key(|range| (range.machine, range.parameter));
    scalar_qualifications
        .integer_entry_ranges
        .sort_by_key(|range| (range.machine, range.parameter));
    let lowered = LoweredPsi {
        semantic_module: TerminalModule {
            scalar_qualifications,
            machines,
            ..TerminalModule::for_entry(machine_id(1))
        },
        proof_bundle: ProofBundle {
            crash_obligations: Vec::new(),
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    };
    // Final proof metadata and invariant identities belong to the assembled
    // root module, not this provisional scalar closure.
    Ok(lowered)
}

//! Reachable scalar-call closure discovery and terminal module assembly.

use super::*;

pub(super) fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
) -> Result<Vec<psi_symbols::SymbolHandle>, LoweringError> {
    let mut closure = vec![entry];
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
        if selection.signature != CheckedTerminalSignatureEligibility::Eligible {
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
        for target in graph.states.iter().flat_map(|state| {
            state.bindings.iter().filter_map(|binding| {
                let CheckedScalarBindingValue::DirectCall { target_machine, .. } = &binding.value
                else {
                    return None;
                };
                Some(*target_machine)
            })
        }) {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

pub(super) fn lower_scalar_call_closure(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
) -> Result<LoweredTerminalPsi, LoweringError> {
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
        .map(|machine| {
            (
                machine.source_machine,
                usize::from(machine.contract_value.is_some()),
            )
        })
        .collect::<Vec<_>>();
    let mut machines = Vec::with_capacity(prepared.len());
    let mut evidence = Vec::new();
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
            machine.contract_value,
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
    }
    let mut lowered = LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine_id(1),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle {
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

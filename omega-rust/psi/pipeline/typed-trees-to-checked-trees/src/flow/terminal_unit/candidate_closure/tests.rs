use super::*;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

fn symbol(ordinal: usize) -> SymbolHandle {
    SymbolHandle::from_arena_index(u32::try_from(ordinal + 1).unwrap())
}

fn compare_reference(
    checked: &checked_trees::CheckedTrees,
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed: &mut Vec<CheckedComposedUnitControlMachinePlan>,
) {
    let boundaries = checked
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .map(|plan| plan.machine)
        .collect::<Vec<_>>();
    let mut expected_candidates = candidates.clone();
    let mut expected_composed = composed.clone();
    retain_available_reference(
        &checked.typed,
        &checked.facts,
        &boundaries,
        &mut expected_candidates,
        &mut expected_composed,
    );
    retain_available(
        &checked.typed,
        &checked.facts,
        &boundaries,
        candidates,
        composed,
    );
    assert_eq!(
        *candidates, expected_candidates,
        "ordinary bodies and order"
    );
    assert_eq!(*composed, expected_composed, "composed bodies and order");
}

fn composed(plan: &CheckedUnitEffectMachinePlan) -> CheckedComposedUnitControlMachinePlan {
    CheckedComposedUnitControlMachinePlan {
        machine: plan.machine,
        result: checked_trees::CheckedControlResultPlan::Unit,
        natural_ranks: Vec::new(),
        attachment_type_identity: plan.attachment_type_identity.clone(),
        provider_attachment_requirements: plan.provider_attachment_requirements.clone(),
        body_qualifications: plan.body_qualifications.clone(),
        contract_report_fingerprint: plan.contract_report_fingerprint,
        contract_commitment: plan.contract_commitment,
        contract_service_reach: plan.contract_service_reach,
        service_reach: plan.service_reach,
        states: vec![CheckedComposedUnitControlStatePlan {
            state: plan.state,
            structural_parameters: Vec::new(),
            scalar_parameters: Vec::new(),
            entry_claims: Vec::new(),
            bindings: Vec::new(),
            binding_initializers: Vec::new(),
            operations: plan.operations.clone(),
            terminator: CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
        }],
    }
}

#[test]
fn complete_unit_body_owns_overlap_before_dependency_closure() {
    let checked = checked(
        "boundary trait Sink { machine record(value: u64); }
         data Packet { value: u64; }
         machine caller() reaches Sink {
             let packet: Packet = Packet { value: 7 };
             Sink::record(packet.value);
         }",
    );
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .unwrap();
    let effects = &checked.facts.flow.terminal_unit_effects;
    let mut shapes = ShapeCollector::new(&checked.typed);
    let graphs = build_checked_composed_unit_control_machines(
        &checked.typed,
        &checked.facts,
        &mut shapes,
        &effects.boundary_machines,
    );
    assert!(
        graphs.iter().any(|plan| plan.machine == caller.symbol),
        "normal-return cleanup admits the overlapping graph"
    );
    assert_eq!(
        effects
            .machines
            .iter()
            .filter(|plan| plan.machine == caller.symbol)
            .count(),
        1,
        "the ordinary complete body remains available"
    );
    assert!(
        !effects
            .composed_machines
            .iter()
            .any(|plan| plan.machine == caller.symbol),
        "builder overlap is resolved before the duplicate-entry guard"
    );
    let mut candidates = effects.machines.clone();
    let ordinary = candidates
        .iter()
        .find(|plan| plan.machine == caller.symbol)
        .unwrap()
        .clone();
    let mut duplicate_graph = vec![composed(&ordinary)];
    compare_reference(&checked, &mut candidates, &mut duplicate_graph);
    assert!(!candidates.iter().any(|plan| plan.machine == caller.symbol));
    assert!(
        duplicate_graph.is_empty(),
        "forged competing catalog entries still reject"
    );
}

#[test]
fn deep_invalid_chain_and_cycles_close_without_recursive_or_round_replay() {
    const BODY_COUNT: usize = 4096;
    let mut closure =
        CandidateClosure::new((0..BODY_COUNT).map(|ordinal| (symbol(ordinal), symbol(ordinal))));
    for caller in 0..BODY_COUNT - 1 {
        closure.require_entry(caller, symbol(caller + 1), symbol(caller + 1));
    }
    closure.require_entry(
        BODY_COUNT - 1,
        SymbolHandle::invalid(),
        SymbolHandle::invalid(),
    );
    assert!(closure.close().iter().all(|retained| !retained));

    let mut cycle =
        CandidateClosure::new((0..BODY_COUNT).map(|ordinal| (symbol(ordinal), symbol(ordinal))));
    for caller in 0..BODY_COUNT {
        let target = (caller + 1) % BODY_COUNT;
        cycle.require_entry(caller, symbol(target), symbol(target));
    }
    assert!(cycle.close().iter().all(|retained| *retained));
}

#[test]
fn roster_preserves_generations_and_rejects_all_competing_entries() {
    let old_machine = SymbolHandle::from_parts(10, 1);
    let new_machine = SymbolHandle::from_parts(10, 2);
    let state = symbol(20);
    let mut closure = CandidateClosure::new(
        [
            (old_machine, state),
            (new_machine, state),
            (symbol(30), state),
            (symbol(40), state),
            (symbol(40), SymbolHandle::invalid()),
        ]
        .into_iter(),
    );
    closure.require_entry(0, old_machine, state);
    closure.require_entry(1, new_machine, state);
    closure.require_entry(2, SymbolHandle::from_parts(10, 3), state);
    assert_eq!(closure.close(), [true, true, false, false, false]);
}

#[test]
fn structural_fallback_remains_ordinary_only() {
    let checked = checked(
        "data Packet { value: u64; }
         machine identity(value: Packet) -> Packet { value }
         machine caller(value: Packet) { let result: Packet = identity(value); }",
    );
    let effects = &checked.facts.flow.terminal_unit_effects;
    let caller = effects
        .machines
        .iter()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                )
            })
        })
        .unwrap();
    let call = caller
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralCall { .. }
            )
        })
        .unwrap();
    let CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. } = call else {
        unreachable!()
    };
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_for_machine(*target_machine)
            .is_some()
    );
    let mut ordinary = vec![caller.clone()];
    let mut graph = composed(caller);
    graph.machine = symbol(10000);
    graph.states[0].state = symbol(10001);
    graph.states[0].operations = vec![call.clone()];
    let mut graphs = vec![graph];
    compare_reference(&checked, &mut ordinary, &mut graphs);
    assert_eq!(ordinary.as_slice(), std::slice::from_ref(caller));
    assert!(graphs.is_empty());
}

#[test]
fn mixed_catalog_chain_cycles_duplicates_and_order_match_original() {
    let checked = checked("machine leaf() {} machine root() { leaf(); }");
    let plans = &checked.facts.flow.terminal_unit_effects.machines;
    let template = plans
        .iter()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
            })
        })
        .unwrap();
    let call = template
        .operations
        .iter()
        .find(|operation| matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. }))
        .unwrap();
    for scenario in [
        "valid chain",
        "invalid leaf",
        "cycle",
        "broken cycle",
        "duplicate",
        "invalid entry",
        "wrong state",
        "composed duplicate",
        "empty graph",
    ] {
        let mut ordinary = Vec::new();
        let mut graphs = Vec::new();
        for ordinal in 0..48 {
            let mut plan = template.clone();
            plan.machine = symbol(ordinal + 100);
            plan.state = symbol(ordinal + 200);
            plan.operations.clear();
            if ordinal != 47 || matches!(scenario, "cycle" | "broken cycle") {
                let target = (ordinal + 1) % 48;
                let mut call = call.clone();
                if let CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    target_state,
                    ..
                } = &mut call
                {
                    *target_machine = symbol(target + 100);
                    *target_state = symbol(target + 200);
                    if ordinal == 23 && matches!(scenario, "broken cycle" | "wrong state") {
                        *target_state = SymbolHandle::invalid();
                    }
                }
                plan.operations.push(call);
            }
            if ordinal % 2 == 0 {
                ordinary.push(plan);
            } else {
                graphs.push(composed(&plan));
            }
        }
        match scenario {
            "invalid leaf" => {
                graphs.pop();
            }
            "duplicate" => ordinary.push(ordinary[3].clone()),
            "invalid entry" => ordinary[3].state = SymbolHandle::invalid(),
            "composed duplicate" => graphs.push(composed(&ordinary[3])),
            "empty graph" => graphs[3].states.clear(),
            _ => {}
        }
        if matches!(scenario, "cycle" | "valid chain") {
            compare_reference(&checked, &mut ordinary, &mut graphs);
            assert_eq!(ordinary.len() + graphs.len(), 48, "{scenario}");
        } else {
            // Deliberately reverse both input catalogs; publication retains
            // each input's order rather than queue/topological order.
            ordinary.reverse();
            graphs.reverse();
            compare_reference(&checked, &mut ordinary, &mut graphs);
            assert!(ordinary.len() + graphs.len() < 48, "{scenario}");
        }
    }
}

#[test]
fn scalar_fallback_dependency_is_not_a_registered_producer_dependency() {
    let checked = checked(
        "machine touch() {}
         machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine leaf(value: u8) -> u8 {
             touch();
             let row: [u8; 2] = [7u8, 9u8];
             answer(row, value)
         }
         machine caller(value: u8) { let result: u8 = leaf(value); }",
    );
    let machine = |name: &str| {
        checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap()
            .symbol
    };
    for scenario in [
        "valid",
        "missing touch",
        "duplicate leaf",
        "composed collision",
        "invalid scalar signature",
    ] {
        let mut ordinary = checked.facts.flow.terminal_unit_effects.machines.clone();
        let mut graphs = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .clone();
        let leaf = ordinary
            .iter()
            .position(|plan| plan.machine == machine("leaf"))
            .unwrap();
        let caller = ordinary
            .iter()
            .find(|plan| plan.machine == machine("caller"))
            .unwrap();
        let call = caller
            .operations
            .iter()
            .find(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. })
            })
            .unwrap();
        assert!(
            matches!(scalar_targets::available_target(&checked.typed, &checked.facts, &ordinary, caller, call),
            Some(scalar_targets::AvailableScalarTarget::OrdinaryBody(target)) if target == leaf)
        );
        match scenario {
            "missing touch" => ordinary.retain(|plan| plan.machine != machine("touch")),
            "duplicate leaf" => ordinary.push(ordinary[leaf].clone()),
            "composed collision" => graphs.push(composed(&ordinary[leaf])),
            "invalid scalar signature" => ordinary[leaf].scalar_parameters.clear(),
            _ => {}
        }
        compare_reference(&checked, &mut ordinary, &mut graphs);
        assert_eq!(
            ordinary
                .iter()
                .any(|plan| plan.machine == machine("caller")),
            scenario == "valid",
            "{scenario}"
        );
    }
}

#[test]
fn registered_scalar_call_survives_unavailable_ordinary_body() {
    let checked = checked(
        "machine scalar(value: u8) -> u8 { value }
         machine caller(value: u8) { let result: u8 = scalar(value); }",
    );
    let effects = &checked.facts.flow.terminal_unit_effects;
    let caller = effects
        .machines
        .iter()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. })
            })
        })
        .unwrap();
    let call = caller
        .operations
        .iter()
        .find(|operation| matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. }))
        .unwrap();
    let CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } = call else {
        unreachable!()
    };
    assert!(matches!(
        scalar_targets::available_target(
            &checked.typed,
            &checked.facts,
            &effects.machines,
            caller,
            call
        ),
        Some(scalar_targets::AvailableScalarTarget::Registered)
    ));
    let mut rejected = caller.clone();
    rejected.machine = *target_machine;
    rejected.state = SymbolHandle::invalid();
    let mut ordinary = vec![rejected, caller.clone()];
    let mut graphs = Vec::new();
    compare_reference(&checked, &mut ordinary, &mut graphs);
    assert_eq!(ordinary.as_slice(), std::slice::from_ref(caller));
}

fn retain_available_reference(
    program: &TypedTrees,
    facts: &CheckFacts,
    boundary_symbols: &[SymbolHandle],
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed_machines: &mut Vec<CheckedComposedUnitControlMachinePlan>,
) {
    // Both catalogs contain complete admitted bodies. Resolve ordinary calls
    // against their joint entry roster, then prune both sides to a fixed point:
    // an invalid composed leaf must also retire its ordinary upstream callers.
    loop {
        let entries = candidates
            .iter()
            .map(|plan| (plan.machine, plan.state))
            .chain(composed_machines.iter().map(|plan| {
                (
                    plan.machine,
                    plan.states
                        .first()
                        .map_or(SymbolHandle::invalid(), |state| state.state),
                )
            }))
            .collect::<Vec<_>>();
        let unique_entries = entries
            .iter()
            .filter(|(machine, state)| {
                machine.is_valid()
                    && state.is_valid()
                    && entries
                        .iter()
                        .filter(|(candidate, _)| candidate == machine)
                        .count()
                        == 1
            })
            .copied()
            .collect::<Vec<_>>();
        let old_lengths = (candidates.len(), composed_machines.len());
        // Every scalar call observes the same candidate roster for this pass.
        // Mutating it during availability checks would make transitive pruning
        // depend on declaration order; no body copies are needed to retain it.
        let retained_candidates = candidates
            .iter()
            .map(|plan| {
                if !unique_entries.contains(&(plan.machine, plan.state)) {
                    return false;
                }
                plan.operations
                    .iter()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .all(|operation| {
                        match operation {
                    CheckedUnitEffectOperationPlan::CallUnit {
                        target_machine,
                        target_state,
                        ..
                    } => unique_entries.contains(&(*target_machine, *target_state)),
                    CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                        boundary_symbols.contains(target_machine)
                    }
                    CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                        target_machine, ..
                    } => boundary_symbols.contains(target_machine),
                    CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                        target_machine,
                        ..
                    } => boundary_symbols.contains(target_machine),
                    CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
                        scalar_targets::available_target(program, facts, candidates, plan, operation).is_some()
                    }
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        target_machine,
                        target_state,
                        ..
                    } => {
                        unique_entries.contains(&(*target_machine, *target_state))
                            || facts
                                .flow
                                .terminal_structural_returns
                                .claim_free_affine_for_machine(*target_machine)
                                .is_some()
                    }
                    // Exact realization custody was already joined by selected
                    // execution before this plan was minted.
                    CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                        ..
                    } => true,
                    CheckedUnitEffectOperationPlan::PortWrite { .. }
                    | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                    | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                    | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                    | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                    | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                    | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                    | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                    | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                    | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                    | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                    | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                    | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                    | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                    | CheckedUnitEffectOperationPlan::Complete { .. } => true,
                }
                    })
            })
            .collect::<Vec<_>>();
        let mut retained_candidates = retained_candidates.into_iter();
        candidates.retain(|_| retained_candidates.next().unwrap_or(false));
        composed_machines.retain(|plan| {
            unique_entries
                .iter()
                .any(|(machine, _)| *machine == plan.machine)
                && plan
                    .states
                    .iter()
                    .flat_map(|state| &state.operations)
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .all(|operation| match operation {
                        CheckedUnitEffectOperationPlan::CallUnit {
                            target_machine,
                            target_state,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::StructuralCall {
                            target_machine,
                            target_state,
                            ..
                        } => unique_entries.contains(&(*target_machine, *target_state)),
                        CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                            boundary_symbols.contains(target_machine)
                        }
                        CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            target_machine,
                            ..
                        } => boundary_symbols.contains(target_machine),
                        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                        | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                        | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                        | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                        | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. } => true,
                        _ => false,
                    })
        });
        if (candidates.len(), composed_machines.len()) == old_lengths {
            break;
        }
    }
}

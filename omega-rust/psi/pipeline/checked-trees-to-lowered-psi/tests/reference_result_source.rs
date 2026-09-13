//! A returned reference retains its caller's referent across call completion.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, Terminator};

fn artifact(prefix: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let checked = checked(prefix);
    terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("ordinary sequencing must retain the returned reference and its source loan")
}

fn checked(prefix: &str) -> checked_trees::CheckedTrees {
    let source = format!(
        "machine notify() {{}}
         machine mark(value: &mut i32) {{ value = 11; }}
         machine relay(value: &mut i32) -> &mut i32 {{ {prefix} value }}
         machine replace(value: &mut i32) {{ value = 29; }}
         machine exercise(value: &mut i32) -> i32 {{
             let held: &mut i32 = relay(value);
             replace(held);
             value
         }}"
    );
    typed_trees_to_checked_trees::lower_typed_trees(typed(&source))
        .unwrap_or_else(|diagnostics| panic!("reference-result checking: {diagnostics:#?}"))
}

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("reference-result tokens");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("reference-result syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("reference-result resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("reference-result typing")
}

fn signed(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32 carrier"),
        value: IntegerValue::Signed(value),
    }
}

#[test]
fn reference_result_preserves_original_storage_through_fuel_resumption() {
    execute(&artifact(""), 1, 1);
}

#[test]
fn reference_result_composes_with_an_ordinary_call_before_return() {
    execute(&artifact("mark(value);"), 2, 1);
}

fn local_record_checked(prefix: &str) -> checked_trees::CheckedTrees {
    let source = format!(
        "data View {{ body: &mut i32; }}
        machine replace(value: &mut i32) {{ value = 29; }}
        machine exercise(value: &mut i32) -> i32 {{
            {prefix}
            let held: View = View {{ body: value }};
            replace(held.body);
            value
        }}"
    );
    typed_trees_to_checked_trees::lower_typed_trees(typed(&source))
        .unwrap_or_else(|diagnostics| panic!("local reference checking: {diagnostics:#?}"))
}

#[test]
fn local_reference_record_preserves_original_storage() {
    let checked = local_record_checked("");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("local reference record has exact leaf custody");
    execute(&artifact, 1, 0);
}

#[test]
fn local_reference_record_composes_with_scalar_computation() {
    let checked = local_record_checked("let offset: i32 = 1 + 2;");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("ordinary scalar computation preserves reference construction");
    execute(&artifact, 1, 0);
}

#[test]
fn local_reference_record_rejects_changed_source_custody() {
    let original = local_record_checked("");
    let loan = original
        .facts
        .borrow
        .loans
        .iter()
        .next()
        .expect("stored leaf loan")
        .0;
    for mutation in 0..6 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed.facts.borrow.loans.get_mut(loan).root_symbol =
                    symbols::SymbolHandle::invalid()
            }
            1 => changed.facts.borrow.loans.get_mut(loan).owner_path = arena::HandleSpan::empty(),
            2 => {
                let handle = changed
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .iter()
                    .find_map(|(handle, weakening)| (weakening.loan == loan).then_some(handle))
                    .unwrap();
                changed
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .get_mut(handle)
                    .reason = checked_trees::FlowBorrowWeakeningReason::LocalReassigned;
            }
            3 => {
                let machine = original
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "exercise")
                    .unwrap();
                let state = &original.machine_states(machine)[0];
                let flow = changed
                    .facts
                    .flow
                    .control
                    .states
                    .iter()
                    .find_map(|(_, flow)| (flow.state_symbol == state.symbol).then_some(flow))
                    .unwrap();
                let (offset, entry) = changed
                    .facts
                    .flow
                    .control
                    .statements
                    .span(flow.statements)
                    .unwrap()
                    .iter()
                    .enumerate()
                    .find(|(_, entry)| entry.statement_index == 1)
                    .unwrap();
                let handle = arena::Handle::from_parts(
                    flow.statements.start().arena_index() + u32::try_from(offset).unwrap(),
                    flow.statements.start().generation(),
                );
                let constraints = changed
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .span(entry.entry_constraints)
                    .unwrap();
                let retained = constraints.iter().copied().filter(|constraint|
                    !matches!(constraint.kind, checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == loan))
                    .collect::<Vec<_>>();
                assert_eq!(
                    constraints.len(),
                    retained.len() + 1,
                    "one active leaf premise"
                );
                let replacement = changed
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .insert_many(retained);
                changed
                    .facts
                    .flow
                    .control
                    .statements
                    .get_mut(handle)
                    .entry_constraints = replacement;
                assert!(
                    validation::reference_result_custody::local_record_loans(
                        &original.typed,
                        &changed.facts,
                        machine.symbol,
                        state,
                        0
                    )
                    .is_some(),
                    "formation and weakening remain valid"
                );
                let checked_trees::statement::StatementNode::Call(call) =
                    &original.statement_table.statements(state.statement_nodes)[1]
                else {
                    unreachable!()
                };
                let argument = original.statement_table.expression_handles(call.arguments)[0];
                let binding = original.facts.flow.terminal_unit_effects.machines.iter()
                    .find(|plan| plan.state == state.symbol).unwrap().operations.iter()
                    .find_map(|operation| match operation {
                        checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => Some(result),
                        _ => None,
                    }).unwrap();
                assert!(
                    validation::reference_result_custody::record_argument(
                        &original.typed,
                        &changed.facts,
                        machine.symbol,
                        state,
                        1,
                        argument,
                        binding,
                        original.state_parameters(state)[0].type_reference
                    )
                    .is_none(),
                    "the consumer cannot replace its missing active loan premise"
                );
            }
            4 => {
                let handle = changed
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .iter()
                    .find_map(|(handle, node)| {
                        matches!(
                            node.kind,
                            checked_trees::CheckedStructuralValueKind::Reference { .. }
                        )
                        .then_some(handle)
                    })
                    .unwrap();
                let checked_trees::CheckedStructuralValueKind::Reference { source } = &mut changed
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get_mut(handle)
                    .kind
                else {
                    unreachable!()
                };
                source.source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: 1,
                };
            }
            5 => {
                let state = changed
                    .facts
                    .borrow
                    .states
                    .iter()
                    .find_map(|(_, state)| {
                        changed
                            .facts
                            .borrow
                            .state_owns_loan(state, loan)
                            .then_some(state.clone())
                    })
                    .unwrap();
                changed.facts.borrow.states.append(state);
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "exercise")
                .produce_artifact()
                .is_err(),
            "mutation {mutation} must not fabricate carrier authority"
        );
    }
}

#[test]
fn local_reference_record_rejects_conflicting_original_access() {
    let source = "data View { body: &mut i32; }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: View = View { body: value };
            replace(value);
            replace(held.body);
            value
        }";
    assert!(typed_trees_to_checked_trees::lower_typed_trees(typed(source)).is_err());
}

const STORED_REFERENCE_SOURCE: &str = "data View { body: &mut i32; }
        machine make_view(value: &mut i32) -> View { View { body: value } }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: View = make_view(value);
            replace(held.body);
            value
        }";

const OWNED_REFERENCE_RECORD_SOURCE: &str = "data View { body: &mut i32; }
        machine forward(value: View) -> View { value }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let input: View = View { body: value };
            let held: View = forward(input);
            replace(held.body);
            value
        }";

#[test]
fn owned_reference_record_argument_preserves_original_storage() {
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed(OWNED_REFERENCE_RECORD_SOURCE))
            .expect("owned reference record arguments reach checked trees");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("owned reference record arguments preserve complete leaf custody");
    execute(&artifact, 1, 1);
}

#[test]
fn owned_reference_record_argument_composes_with_ordinary_work() {
    let source = OWNED_REFERENCE_RECORD_SOURCE
        .replace("machine forward(value: View) -> View { value }", "machine notify() {} machine forward(marker: i32, value: View) -> View { notify(); value }")
        .replace("let input: View", "let marker: i32 = 4 + 5; let input: View")
        .replace("forward(input)", "forward(marker, input)");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed(&source)).unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("owned ingress composes with scalar formals and ordinary effects");
    execute(&artifact, 1, 1);
}

#[test]
fn owned_reference_record_argument_rejects_changed_prior_custody() {
    let original =
        typed_trees_to_checked_trees::lower_typed_trees(typed(OWNED_REFERENCE_RECORD_SOURCE))
            .unwrap();
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let prior = validation::reference_result_custody::local_record_loans(
        &original.typed,
        &original.facts,
        machine.symbol,
        state,
        0,
    )
    .unwrap()[0]
        .1;
    let returned = validation::reference_result_custody::local_record_loans(
        &original.typed,
        &original.facts,
        machine.symbol,
        state,
        1,
    )
    .unwrap()[0]
        .1;
    for mutation in 0..4 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed
                    .facts
                    .borrow
                    .loans
                    .get_mut(returned)
                    .source_owner_symbol = symbols::SymbolHandle::invalid()
            }
            1 => changed.facts.borrow.loans.get_mut(prior).owner_path = arena::HandleSpan::empty(),
            2 => {
                changed.facts.borrow.loans.get_mut(prior).root_symbol =
                    changed.facts.borrow.loans.get(returned).owner_symbol
            }
            3 => {
                let plan = changed
                    .facts
                    .flow
                    .terminal_unit_effects
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine != machine.symbol && plan.structural_result.is_some())
                    .unwrap();
                plan.structural_result.as_mut().unwrap().reference_sources[0]
                    .source
                    .path
                    .clear();
            }
            _ => unreachable!(),
        }
        if mutation < 3 {
            assert!(
                validation::reference_result_custody::local_record_loans(
                    &changed.typed,
                    &changed.facts,
                    machine.symbol,
                    state,
                    1
                )
                .is_none(),
                "changed prior custody {mutation}"
            );
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "exercise")
                .produce_artifact()
                .is_err(),
            "changed transferred custody {mutation}"
        );
    }
}

#[test]
fn reference_record_origin_roster_rejects_exponential_type_dags() {
    let mut source = String::from("data Leaf { body: &mut i32; }");
    let mut previous = "Leaf".to_owned();
    for depth in 0..13 {
        let name = format!("Level{depth}");
        source.push_str(&format!(
            "data {name} {{ left: {previous}; right: {previous}; }}"
        ));
        previous = name;
    }
    source.push_str(&format!(
        "machine forward(value: {previous}) -> {previous} {{ value }}"
    ));
    let program = typed(&source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    assert!(!validation::reference_result_custody::is_reference_record(
        &program,
        state.return_type
    ));
    assert!(
        validation::reference_result_custody::returned_record_sources(&program, state).is_none()
    );
}

#[test]
fn stored_reference_result_preserves_original_storage() {
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed(STORED_REFERENCE_SOURCE))
        .unwrap_or_else(|diagnostics| panic!("stored-reference checking: {diagnostics:#?}"));
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("stored result carries exact returned leaf origins");
    execute(&artifact, 1, 1);
}

#[test]
fn stored_reference_result_composes_with_an_ordinary_effect() {
    let source = "data View { body: &mut i32; }
        machine mark(value: &mut i32) { value = 11; }
        machine make_view(value: &mut i32) -> View { mark(value); View { body: value } }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: View = make_view(value);
            replace(held.body);
            value
        }";
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed(source)).unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("ordinary effect precedes reference record completion");
    execute(&artifact, 2, 1);
}

#[test]
fn stored_reference_result_rejoins_full_formal_positions() {
    let source = "data View { body: &mut i32; }
        machine make_view(ignored: i32, value: &mut i32) -> View { View { body: value } }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: View = make_view(17, value);
            replace(held.body);
            value
        }";
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed(source)).unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("structural ordinal rejoins its full authored argument position");
    execute(&artifact, 1, 1);
}

#[test]
fn stored_reference_result_rejects_changed_return_and_actual_origins() {
    use checked_trees::{
        CheckedStructuralValueKind as Value, CheckedUnitEffectOperationPlan as Operation,
        CheckedUnitStructuralArgumentSourcePlan as Source,
    };
    let source = format!(
        "{STORED_REFERENCE_SOURCE}
        machine alternate(value: &mut i32) -> View {{ View {{ body: value }} }}"
    );
    let original = typed_trees_to_checked_trees::lower_typed_trees(typed(&source)).unwrap();
    let _ = terminal_production::TerminalProductionRequest::new(&original, "exercise")
        .produce_artifact()
        .expect("unmodified same-typed helper roster");
    let helper = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "make_view")
        .unwrap();
    let helper_state = &original.machine_states(helper)[0];
    let alternate = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "alternate")
        .unwrap();
    let alternate_state = original.machine_states(alternate)[0].symbol;
    let alternate_contract = original
        .facts
        .contract_plans
        .for_machine(alternate.symbol)
        .unwrap();
    let loan = original.facts.borrow.loans.iter().next().unwrap().0;
    let values = &original.facts.values.structural_values;
    let root = values.root_at(helper_state.symbol, 0).unwrap();
    let Value::Record { fields, .. } = values.nodes.get(root.root).kind else {
        unreachable!()
    };
    let checked_trees::CheckedStructuralRecordFieldValue::Structural(leaf) =
        values.record_fields.span(fields).unwrap()[0].value
    else {
        unreachable!()
    };
    for mutation in 0..7 {
        let mut changed = original.clone();
        match mutation {
            0 | 1 | 5 | 6 => {
                let returned = changed
                    .facts
                    .flow
                    .terminal_unit_effects
                    .machines
                    .iter_mut()
                    .find(|machine| machine.state == helper_state.symbol)
                    .unwrap()
                    .structural_result
                    .as_mut()
                    .unwrap();
                match mutation {
                    0 => returned.reference_sources[0].path.clear(),
                    1 | 6 => {
                        returned.reference_sources[0].source.source =
                            Source::Parameter { parameter_index: 1 }
                    }
                    5 => {
                        returned.source = Source::StructuralResult {
                            binding_ordinal: 99,
                        }
                    }
                    _ => unreachable!(),
                }
                if mutation == 6 {
                    let Value::Reference { source } = &mut changed
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get_mut(leaf)
                        .kind
                    else {
                        unreachable!()
                    };
                    source.source = Source::Parameter { parameter_index: 1 };
                }
            }
            2 => {
                let Value::Reference { source } = &mut changed
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get_mut(leaf)
                    .kind
                else {
                    unreachable!()
                };
                source.source = Source::Parameter { parameter_index: 1 };
            }
            3 => {
                changed.facts.borrow.loans.get_mut(loan).root_symbol =
                    original.state_parameters(helper_state)[0].symbol
            }
            4 => {
                let Operation::StructuralCall { target_machine, target_state, target_contract_report_fingerprint, target_contract_commitment, .. } =
                    changed.facts.flow.terminal_unit_effects.machines.iter_mut().flat_map(|machine| &mut machine.operations)
                        .find(|operation| matches!(operation, Operation::StructuralCall { target_state, .. } if *target_state == helper_state.symbol)).unwrap() else { unreachable!() };
                *target_machine = alternate.symbol;
                *target_state = alternate_state;
                *target_contract_report_fingerprint = alternate_contract.report_fingerprint;
                *target_contract_commitment = alternate_contract.commitment;
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "exercise")
                .produce_artifact()
                .is_err(),
            "mutation {mutation} cannot replace exact returned or caller origins"
        );
    }
}

#[test]
fn reference_release_processing_preserves_empty_helpers() {
    let program = typed("machine empty() {} machine exercise() { empty(); }");
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(program).expect("check empty helper");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_artifact()
        .expect("empty helper has no last-statement release boundary");
    drop(checked);
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &Default::default(),
        &[],
    )
    .expect("reload empty helper closure");
    let mut meter = TerminalFuelMeter::with_allowance(8);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn reference_result_composes_with_an_empty_unit_call_before_return() {
    execute(&artifact("notify();"), 1, 1);
}

#[test]
fn reference_result_rejects_conflicting_access_before_last_use() {
    let program = typed(
        "machine relay(value: &mut i32) -> &mut i32 { value }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: &mut i32 = relay(value);
            replace(value);
            replace(held);
            value
        }",
    );
    assert!(
        typed_trees_to_checked_trees::lower_typed_trees(program).is_err(),
        "a live returned loan excludes direct access to its original referent"
    );
}

#[test]
fn reference_result_rejects_changed_source_loan_and_weakening() {
    use checked_trees::{
        CheckedUnitEffectOperationPlan as Operation,
        CheckedUnitStructuralArgumentSourcePlan as Source,
    };
    let original = checked("");
    let _ = terminal_production::TerminalProductionRequest::new(&original, "exercise")
        .produce_artifact()
        .expect("untampered reference custody");
    for mutation in 0..7 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_unit_effects.machines;
        match mutation {
            0 => {
                let machine = plans
                    .iter_mut()
                    .find(|machine| machine.structural_result.is_some())
                    .expect("reference-returning helper");
                let result = machine.structural_result.as_mut().unwrap();
                result.reference_sources[0].source.source =
                    Source::Parameter { parameter_index: 1 };
                let Operation::EstablishReference { source, .. } = machine
                    .operations
                    .iter_mut()
                    .find(|operation| matches!(operation, Operation::EstablishReference { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                source.source = Source::Parameter { parameter_index: 1 };
            }
            1 => {
                let Operation::StructuralCall { custody, .. } = plans.iter_mut().flat_map(|machine| &mut machine.operations)
                    .find(|operation| matches!(operation, Operation::StructuralCall { custody, .. } if custody.reference_loan.is_valid())).unwrap() else { unreachable!() };
                custody.reference_loan = arena::Handle::invalid();
            }
            2 => {
                let Operation::ReleaseReference {
                    statement_index, ..
                } = plans
                    .iter_mut()
                    .flat_map(|machine| &mut machine.operations)
                    .find(|operation| matches!(operation, Operation::ReleaseReference { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                *statement_index -= 1;
            }
            3 => {
                let machine = plans
                    .iter_mut()
                    .find(|machine| {
                        machine.operations.iter().any(|operation| {
                            matches!(operation, Operation::ReleaseReference { .. })
                        })
                    })
                    .unwrap();
                machine
                    .operations
                    .retain(|operation| !matches!(operation, Operation::ReleaseReference { .. }));
            }
            4 => {
                let loan = plans
                    .iter()
                    .flat_map(|machine| &machine.operations)
                    .find_map(|operation| match operation {
                        Operation::StructuralCall { custody, .. }
                            if custody.reference_loan.is_valid() =>
                        {
                            Some(custody.reference_loan)
                        }
                        _ => None,
                    })
                    .unwrap();
                changed.facts.borrow.loans.get_mut(loan).root_symbol =
                    symbols::SymbolHandle::invalid();
            }
            5 => {
                let loan = plans
                    .iter()
                    .flat_map(|machine| &machine.operations)
                    .find_map(|operation| match operation {
                        Operation::StructuralCall { custody, .. }
                            if custody.reference_loan.is_valid() =>
                        {
                            Some(custody.reference_loan)
                        }
                        _ => None,
                    })
                    .unwrap();
                let weakening = changed
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .iter()
                    .find_map(|(handle, weakening)| (weakening.loan == loan).then_some(handle))
                    .unwrap();
                changed
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .get_mut(weakening)
                    .reason = checked_trees::FlowBorrowWeakeningReason::LocalReassigned;
            }
            6 => {
                let machine = plans
                    .iter_mut()
                    .find(|machine| {
                        machine.operations.iter().any(|operation| {
                            matches!(operation, Operation::ReleaseReference { .. })
                        })
                    })
                    .unwrap();
                let release_index = machine
                    .operations
                    .iter()
                    .position(|operation| matches!(operation, Operation::ReleaseReference { .. }))
                    .unwrap();
                let release = machine.operations.remove(release_index);
                machine.operations.insert(release_index - 1, release);
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "exercise")
                .produce_artifact()
                .is_err(),
            "source-custody mutation {mutation} must reject"
        );
    }
}

fn execute(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    expected_stores: usize,
    expected_returns: usize,
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("decode reference-result module");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes())
        .expect("decode reference-result proof");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("independently verify decoded reference custody");
    assert_eq!(
        terminal_codec::encode_module(&module).expect("canonical reference-result encoding"),
        artifact.semantic_bytes()
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("caller entry");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("one original primitive referent");
    };
    let consumer_calls = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    let [consumer_call] = consumer_calls.as_slice() else {
        panic!("one ordinary caller-side write through the returned reference");
    };
    let reference_returns = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .filter_map(|block| match block.terminator {
            Terminator::ReturnStructural { edge, .. } => Some(edge),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reference_returns.len(),
        expected_returns,
        "retain each structural-result return"
    );
    let stores = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), expected_stores, "retain every authored store");

    let initial = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: signed(7),
    };
    let written = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: signed(29),
    };
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 91,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[initial],
        )
        .expect("install original initialized referent");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).expect("initial suspension"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    let mut complete = false;
    for _ in 0..256 {
        meter.replenish(1).expect("one more execution unit");
        let status = execution.resume(&mut meter).expect("resume reference call");
        if meter
            .usage()
            .at(FuelChargeSite::Operation(*consumer_call))
            .is_some()
        {
            assert!(
                reference_returns.iter().all(|reference_return| meter
                    .usage()
                    .at(FuelChargeSite::Edge(*reference_return))
                    .is_some()),
                "the returned reference is unavailable to its consumer before successful return"
            );
        }
        let executions = stores
            .iter()
            .map(|store| {
                meter
                    .usage()
                    .at(FuelChargeSite::Operation(*store))
                    .map_or(0, |attribution| attribution.executions())
            })
            .collect::<Vec<_>>();
        assert!(
            executions.iter().all(|count| *count <= 1),
            "no replayed store"
        );
        if executions.iter().all(|count| *count == 1) {
            assert!(
                reference_returns.iter().all(|reference_return| meter
                    .usage()
                    .at(FuelChargeSite::Edge(*reference_return))
                    .is_some()),
                "the result consumer cannot execute before relay returns"
            );
            assert_eq!(execution.structural_primitive_values(), vec![written]);
        }
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Scalar(signed(29)));
                assert!(executions.iter().all(|count| *count == 1));
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let usage = meter.usage().clone();
                let storage = execution.structural_primitive_values();
                assert!(matches!(
                    execution
                        .resume(&mut meter)
                        .expect("retry exhausted execution"),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(meter.usage(), &usage, "an unpaid retry performs no work");
                assert_eq!(execution.structural_primitive_values(), storage);
            }
            other => panic!("unexpected reference-result execution status: {other:?}"),
        }
    }
    assert!(complete, "finite reference-result program must complete");
    assert_eq!(execution.structural_primitive_values(), vec![written]);
    let usage = meter.usage().clone();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("completed execution is stable"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(signed(29)))
    );
    assert_eq!(meter.usage(), &usage);
    assert_eq!(execution.structural_primitive_values(), vec![written]);
}

//! Composed Unit calls execute their scalar operands and their observable bodies.

use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

const HELPERS: &str = r#"
    machine identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
    data Scalar {}
    machine Scalar::identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
    machine identity16(input: u16) -> u16
    requires 0u16 == 0u16
    ensures 0u16 == 0u16
    { input }
"#;

fn artifact(checked: &checked_trees::CheckedTrees, state_count: usize) -> (Vec<u8>, Vec<u8>) {
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let states = checked.typed.machine_states(root);
    assert_eq!(
        states.len(),
        state_count,
        "no synthetic authored control states"
    );
    for state in states {
        assert!(
            checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .all(|statement| !matches!(statement, StatementNode::LocalData(_))),
            "computed operands do not manufacture authored temporaries"
        );
    }
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, plan)| {
                plan.machine == root.symbol
                    && matches!(
                        plan.role,
                        CheckedScalarExpressionRole::UnitCallArgument { .. }
                    )
            }),
        "root retains ordinary Unit argument computations"
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("composed internal Unit operand and body lowering");
    if state_count == 4 {
        let helper = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "identity16")
            .unwrap();
        let mut callers = Vec::new();
        let mut targets = std::collections::BTreeSet::new();
        for occurrence in &lowered.source_call_occurrences {
            // Scalar emission records CheckedScalarCallPlan::target_machine,
            // not the authored entry-state target used by ordinary Unit calls.
            if occurrence.source_target != helper.symbol {
                continue;
            }
            if !callers.contains(&occurrence.source_state) {
                callers.push(occurrence.source_state);
            }
            let operation = lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == occurrence.terminal_operation)
                .unwrap();
            let terminal_psi::OperationKind::Call { callee, .. } = operation.kind else {
                panic!("identity16 source joins identify scalar calls");
            };
            targets.insert(callee);
        }
        assert!(
            callers.len() >= 2,
            "shared helper is called from root prefix and Unit bodies"
        );
        assert_eq!(
            targets.len(),
            1,
            "one source helper has one Terminal machine identity across the closure"
        );
        assert_eq!(
            lowered
                .semantic_module
                .machines
                .iter()
                .filter(|machine| targets.contains(&machine.id))
                .count(),
            1
        );
    }
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::CallUnit { .. }
            )),
        "ordinary Unit calls remain actual terminal calls"
    );
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    (semantic, evidence)
}

fn ordinary_source(qualified: bool, prefix: bool) -> (String, usize) {
    ordinary_source_spelling(qualified, prefix, false)
}

fn ordinary_source_spelling(qualified: bool, prefix: bool, trailing: bool) -> (String, usize) {
    let terminator = if trailing { "" } else { ";" };
    let owner = if qualified { "Relay::" } else { "" };
    let control = if prefix {
        format!(
            r#"
            {owner}relay(identity16(41u16), identity16(43u16));
            transition first {{ true -> dispatch(second) _ -> no() }}
            state dispatch(second: bool) {{ transition second {{ true -> yes() _ -> no() }} }}
        "#
        )
    } else {
        "transition first { true -> yes() _ -> no() }".into()
    };
    let parameters = if prefix {
        "first: bool, second: bool"
    } else {
        "first: bool"
    };
    (
        format!(
            r#"
        {HELPERS}
        boundary trait Sink {{ machine finish(first: u16, second: u16); }}
        data Relay {{}}
        machine {owner}forward(first: u16, second: u16) reaches Sink {{
            Sink::finish(identity16(first), identity16(second));
        }}
        machine {owner}relay(first: u16, second: u16) reaches Sink {{
            Sink::finish(first, second);
            {owner}forward(identity16(second), identity16(first));
        }}
        machine {owner}other(first: u16, second: u16) reaches Sink {{ Sink::finish(second, first); }}
        data Main {{}}
        machine Main::main({parameters}) {{
            {control}
            state yes() {{
                {owner}relay((Scalar::identity(identity(255u8)) as u16) + 1u16, identity(7u8) as u16){terminator}
            }}
            state no() {{
                {owner}relay(Scalar::identity(identity(3u8)) as u16, identity(19u8) as u16){terminator}
            }}
        }}
    "#
        ),
        if prefix { 4 } else { 3 },
    )
}

#[test]
fn outer_and_transitive_unit_calls_reject_target_arity_and_operand_source_drift() {
    let (source, states) = ordinary_source(true, true);
    let checked = crate::front_end::checked_program(&source);
    artifact(&checked, states);
    let replacement = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Relay::other")
        .unwrap();
    let replacement_state = checked.typed.machine_states(replacement)[0].symbol;
    let mut covered = Vec::new();
    for machine in checked.typed.machines() {
        if !matches!(
            machine.name.as_str(),
            "Main::main" | "Relay::relay" | "Relay::forward"
        ) {
            continue;
        }
        for state in checked.typed.machine_states(machine) {
            for (index, statement) in checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::Call(call) = statement else {
                    continue;
                };
                covered.push(machine.name.as_str());
                for mutation in 0..3 {
                    let mut changed = checked.clone();
                    let StatementNode::Call(changed_call) = &mut changed
                        .typed
                        .statement_table
                        .statements_mut(state.statement_nodes)[index]
                    else {
                        unreachable!();
                    };
                    match mutation {
                        0 => changed_call.target_symbol = replacement_state,
                        1 => changed_call.arguments = arena::HandleSpan::empty(),
                        2 => {
                            let arguments = checked
                                .typed
                                .statement_table
                                .expression_handles(call.arguments);
                            changed
                                .typed
                                .statement_table
                                .set_expression_handle_at_offset(call.arguments, 0, arguments[1]);
                        }
                        _ => unreachable!(),
                    }
                    assert!(
                        checked_trees_to_lowered_psi::lower_machine(
                            &changed,
                            TerminalMachineSelection::Name("Main::main")
                        )
                        .is_err(),
                        "{}: outer/target call mutation={mutation}",
                        machine.name.as_str()
                    );
                }
            }
        }
    }
    for owner in ["Main::main", "Relay::relay", "Relay::forward"] {
        assert!(covered.contains(&owner));
    }
    let plans = &checked.facts.values.scalar_computations;
    for (_, node) in plans.nodes.iter() {
        let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
            continue;
        };
        let authored = checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .authored_expression;
        let mut changed = checked.clone();
        changed
            .facts
            .flow
            .control
            .calls
            .get_mut(source_call)
            .authored_expression = arena::Handle::invalid();
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err()
        );
        let ExpressionNode::Call(call) = checked.typed.expression_table.expression(authored) else {
            unreachable!();
        };
        if call.receiver.is_valid() {
            let mut changed = checked.clone();
            let ExpressionNode::Name(path) =
                changed.typed.expression_table.expression_mut(call.receiver)
            else {
                unreachable!();
            };
            path.symbol = symbols::SymbolHandle::invalid();
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("Main::main")
                )
                .is_err()
            );
        }
    }
}

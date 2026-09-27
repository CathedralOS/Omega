use super::super::{CheckedScalarComputationHandle, ExpressionHandle, SymbolHandle};
use super::{
    CheckedScalarComputationKind, CheckedScalarExpressionRole, ExpressionNode, StatementNode,
};
use crate::values::build_checked_scalar_computation_plans;
use crate::values::scalar::computations::tests::checked_source;

fn local_fixture(borrow_first: bool) -> checked_trees::CheckedTrees {
    let arguments = if borrow_first {
        "stamp(&mut scratch, 7), scratch"
    } else {
        "scratch, stamp(&mut scratch, 7)"
    };
    checked_source(
        &format!(
            r#"
        machine stamp(value: &mut u64, number: u64) -> u64 {{ value = number; number }}
        machine second(first: u64, second: u64) -> u64 {{ second }}
        machine consume(output: &mut u64, value: u64) {{ output = value; }}
        machine enter(output: &mut u64) {{
            let mut scratch: u64 = 91;
            consume(&mut output, second({arguments}));
            output = scratch;
        }}
    "#
        ),
        false,
    )
}

fn entry(checked: &checked_trees::CheckedTrees) -> &typed_trees::state::State {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    assert_eq!(
        checked.machine_states(machine).len(),
        1,
        "no generated states"
    );
    &checked.machine_states(machine)[0]
}

fn root(checked: &checked_trees::CheckedTrees) -> CheckedScalarComputationHandle {
    checked
        .facts
        .values
        .scalar_computations
        .root_at(
            entry(checked).symbol,
            1,
            CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal: 0,
                argument_ordinal: 0,
            },
        )
        .expect("outer Unit call operand computation")
        .root
}

#[test]
fn borrowed_computation_arguments_reject_missing_duplicate_or_substituted_custody() {
    let checked = local_fixture(true);
    let state = entry(&checked).symbol;
    let plans = &checked.facts.values.scalar_computations;
    let source = plans
        .nodes
        .iter()
        .find_map(|(_, node)| match node.kind {
            CheckedScalarComputationKind::Call {
                structural_arguments,
                source_call,
                ..
            } if !structural_arguments.is_empty() => {
                Some(checked.facts.flow.control.calls.get(source_call))
            }
            _ => None,
        })
        .unwrap();
    let borrow_call = checked
        .facts
        .borrow
        .calls
        .iter()
        .find(|(_, call)| {
            call.statement_index == source.statement_index
                && call.call_ordinal == source.call_ordinal
                && call.target_symbol == source.target_symbol
        })
        .unwrap()
        .0;
    for mutation in 0..6 {
        let mut borrow = checked.facts.borrow.clone();
        let access = borrow.calls.get(borrow_call).accesses.start();
        match mutation {
            0 => borrow.calls.get_mut(borrow_call).accesses = arena::HandleSpan::empty(),
            1 => {
                borrow.argument_accesses.get_mut(access).kind =
                    checked_trees::BorrowAccessKind::Read
            }
            2 => borrow.argument_accesses.get_mut(access).root_symbol = SymbolHandle::invalid(),
            3 => borrow.calls.get_mut(borrow_call).call_ordinal += 1,
            4 => borrow.calls.get_mut(borrow_call).target_symbol = SymbolHandle::invalid(),
            _ => {
                let (handle, row) = borrow
                    .states
                    .iter()
                    .find(|(_, row)| row.state_symbol == state)
                    .unwrap();
                let mut calls = borrow.calls.span(row.calls).unwrap().to_vec();
                calls.push(borrow.calls.get(borrow_call).clone());
                let calls = borrow.calls.insert_many(calls);
                borrow.states.get_mut(handle).calls = calls;
            }
        }
        let plans = build_checked_scalar_computation_plans(
            &checked.typed,
            &checked.facts.operators,
            &checked.facts.flow,
            &borrow,
            &checked.facts.proof,
            &checked.facts.values.scalar_expressions,
            &[],
        );
        assert!(
            plans
                .root_at(
                    state,
                    1,
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: 0,
                        argument_ordinal: 0
                    }
                )
                .is_none(),
            "custody mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_computation_arguments_reject_unestablished_or_non_named_storage() {
    let checked = local_fixture(true);
    let state = entry(&checked);
    let plans = &checked.facts.values.scalar_computations;
    let call = plans
        .nodes
        .iter()
        .find_map(|(_, node)| match node.kind {
            CheckedScalarComputationKind::Call {
                structural_arguments,
                source_call,
                ..
            } if !structural_arguments.is_empty() => {
                Some(checked.facts.flow.control.calls.get(source_call))
            }
            _ => None,
        })
        .unwrap();
    let ExpressionNode::Call(authored) = checked
        .expression_table
        .expression(call.authored_expression)
    else {
        panic!("stamp call");
    };
    let arguments = checked
        .expression_table
        .expression_handles(authored.arguments);
    let borrow_expression = arguments[0];
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(borrow_expression)
    else {
        panic!("local borrow");
    };
    for mutation in 0..6 {
        let mut program = checked.typed.clone();
        match mutation {
            0 | 1 => {
                let StatementNode::LocalData(local) = &mut program
                    .statement_table
                    .statements_mut(state.statement_nodes)[0]
                else {
                    panic!("storage declaration");
                };
                if mutation == 0 {
                    local.is_mutable = false;
                } else {
                    local.initial_value = ExpressionHandle::invalid();
                }
            }
            2 => {
                let ExpressionNode::Borrow(changed) =
                    program.expression_table.expression_mut(borrow_expression)
                else {
                    panic!("borrow");
                };
                changed.target = arguments[1];
            }
            3 => {
                let ExpressionNode::Name(changed) =
                    program.expression_table.expression_mut(borrow.target)
                else {
                    panic!("name");
                };
                changed.head_symbol = SymbolHandle::invalid();
            }
            4 => {
                let target =
                    crate::semantic::calls::find_state(&program, call.target_symbol).unwrap();
                let parameter = program.state_parameters(target)[0].symbol;
                let handle = program
                    .state_parameters
                    .iter()
                    .find(|(_, row)| row.symbol == parameter)
                    .unwrap()
                    .0;
                program.state_parameters.get_mut(handle).is_const = true;
            }
            _ => {
                let ExpressionNode::Borrow(changed) =
                    program.expression_table.expression_mut(borrow_expression)
                else {
                    panic!("borrow");
                };
                changed.access = language_semantics::ReferenceAccess::Shared;
            }
        }
        let rebuilt = build_checked_scalar_computation_plans(
            &program,
            &checked.facts.operators,
            &checked.facts.flow,
            &checked.facts.borrow,
            &checked.facts.proof,
            &checked.facts.values.scalar_expressions,
            &[],
        );
        assert!(
            rebuilt
                .root_at(
                    state.symbol,
                    1,
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: 0,
                        argument_ordinal: 0
                    }
                )
                .is_none(),
            "shape mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_computation_arguments_do_not_admit_projections_or_owned_structures() {
    for (parameter, argument) in [("&mut u64", "&mut record.value"), ("Record", "record")] {
        let checked = checked_source(
            &format!(
                r#"
            data Record {{ value: u64; }}
            machine sample(value: {parameter}) -> u64 {{ 0 }}
            machine consume(output: &mut u64, value: u64) {{ output = value; }}
            machine enter(output: &mut u64, mut record: Record) {{
                let saved: u64 = 0;
                consume(&mut output, sample({argument}));
            }}
        "#
            ),
            false,
        );
        assert!(
            checked
                .facts
                .values
                .scalar_computations
                .root_at(
                    entry(&checked).symbol,
                    1,
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: 0,
                        argument_ordinal: 0
                    }
                )
                .is_none(),
            "{parameter}: {argument}"
        );
    }
}

fn shared_occurrence_fixture(arguments: &str) -> checked_trees::CheckedTrees {
    checked_source(
        &format!(
            r#"
        machine hold(first: &u64, number: u64, second: &u64) -> u64 {{ number }}
        machine identity(value: u64) -> u64 {{ value }}
        machine consume(output: &mut u64, value: u64) {{ output = value; }}
        machine enter(output: &mut u64, other: &u64) {{
            let mut scratch: u64 = 91;
            consume(&mut output, hold({arguments}));
        }}
    "#
        ),
        false,
    )
}

#[test]
fn borrowed_computation_shared_occurrences_reject_altered_full_access_rosters() {
    for arguments in ["&scratch, scratch, &scratch", "&scratch, scratch, other"] {
        let checked = shared_occurrence_fixture(arguments);
        let CheckedScalarComputationKind::Call { source_call, .. } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(root(&checked))
            .kind
        else {
            panic!("hold call");
        };
        let source = checked.facts.flow.control.calls.get(source_call);
        let borrow_call = checked
            .facts
            .borrow
            .calls
            .iter()
            .find(|(_, call)| {
                call.target_symbol == source.target_symbol
                    && call.statement_index == source.statement_index
                    && call.call_ordinal == source.call_ordinal
            })
            .unwrap()
            .0;
        for mutation in [
            "extra",
            "missing borrow",
            "missing scalar read",
            "exclusive",
            "substituted",
            "reordered",
        ] {
            if mutation == "reordered" && arguments.ends_with("&scratch") {
                continue;
            }
            let mut borrow = checked.facts.borrow.clone();
            let mut flow = checked.facts.flow.clone();
            let mut rows = borrow
                .argument_accesses
                .span(source.accesses)
                .unwrap()
                .to_vec();
            assert_eq!(rows.len(), 3);
            match mutation {
                "extra" => rows.push(rows[0].clone()),
                "missing borrow" => {
                    rows.remove(2);
                }
                "missing scalar read" => {
                    rows.remove(1);
                }
                "exclusive" => rows[2].kind = checked_trees::BorrowAccessKind::Mutable,
                "substituted" => rows[1].root_symbol = SymbolHandle::invalid(),
                "reordered" => rows.swap(0, 2),
                _ => unreachable!(),
            }
            let span = borrow.argument_accesses.insert_many(rows);
            borrow.calls.get_mut(borrow_call).accesses = span;
            flow.control.calls.get_mut(source_call).accesses = span;
            let rebuilt = build_checked_scalar_computation_plans(
                &checked.typed,
                &checked.facts.operators,
                &flow,
                &borrow,
                &checked.facts.proof,
                &checked.facts.values.scalar_expressions,
                &[],
            );
            assert!(
                rebuilt
                    .root_at(
                        entry(&checked).symbol,
                        1,
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: 0,
                            argument_ordinal: 0
                        }
                    )
                    .is_none(),
                "arguments={arguments}, synchronized roster mutation={mutation}"
            );
        }
    }
}

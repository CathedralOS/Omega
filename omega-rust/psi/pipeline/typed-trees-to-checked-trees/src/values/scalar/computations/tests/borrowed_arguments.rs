use super::*;
use checked_trees::{
    CheckedCallScalarArgument, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

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
fn borrowed_computation_arguments_reach_outer_unit_with_source_order() {
    for borrow_first in [false, true] {
        let checked = local_fixture(borrow_first);
        let state = entry(&checked);
        let statements = checked.statement_table.statements(state.statement_nodes);
        assert_eq!(statements.len(), 3, "no hoisted declarations");
        let StatementNode::LocalData(local) = &statements[0] else {
            panic!("authored storage");
        };
        let plans = &checked.facts.values.scalar_computations;
        let root = root(&checked);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| {
                checked
                    .machine_states(machine)
                    .iter()
                    .any(|candidate| candidate.symbol == state.symbol)
            })
            .unwrap();
        let unit = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .expect("complete outer Unit plan");
        assert!(
            unit.operations.iter().any(|operation| matches!(operation,
            CheckedUnitEffectOperationPlan::CallUnit { scalar_arguments, .. }
            if scalar_arguments == &[CheckedCallScalarArgument::Computation(root)])),
            "{:#?}",
            unit.operations
        );
        let CheckedScalarComputationKind::Call {
            arguments,
            structural_arguments,
            ..
        } = plans.nodes.get(root).kind
        else {
            panic!("second invocation");
        };
        assert!(
            structural_arguments.is_empty(),
            "scalar-only call stays scalar-only"
        );
        let operands = plans.operands.span(arguments).unwrap();
        assert_eq!(operands.len(), 2);
        let borrowed_position = usize::from(!borrow_first);
        assert_eq!(
            plans.nodes.get(operands[1 - borrowed_position]).kind,
            CheckedScalarComputationKind::Value(CheckedScalarExpression::StorageRead {
                symbol: local.symbol,
                primitive_type: PrimitiveType::U64
            })
        );
        let CheckedScalarComputationKind::Call {
            arguments,
            structural_arguments,
            source_call,
            target_state,
            call_ordinal,
            ..
        } = plans.nodes.get(operands[borrowed_position]).kind
        else {
            panic!("nested stamp invocation");
        };
        let fact = checked.facts.flow.control.calls.get(source_call);
        assert_eq!(fact.statement_index, 1);
        assert_eq!(fact.call_ordinal, call_ordinal as usize);
        assert_eq!(fact.target_symbol, target_state);
        let ExpressionNode::Call(authored) = checked
            .expression_table
            .expression(fact.authored_expression)
        else {
            panic!("exact authored call");
        };
        let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(
            checked
                .expression_table
                .expression_handles(authored.arguments)[0],
        ) else {
            panic!("actual authored borrow");
        };
        assert!(
            matches!(checked.expression_table.expression(borrow.target), ExpressionNode::Name(name) if name.symbol == local.symbol)
        );
        assert_eq!(plans.operands.span(arguments).unwrap().len(), 1);
        let [argument] = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("one exact structural actual");
        };
        assert_eq!(
            argument.as_place().unwrap().source,
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                symbol: local.symbol
            }
        );
        assert_eq!(
            argument.as_place().unwrap().access,
            CheckedStructuralAccess::MutableBorrow
        );
        assert!(argument.as_place().unwrap().path.is_empty());
        assert!(!argument.as_place().unwrap().type_identity.is_empty());
    }
}

#[test]
fn borrowed_computation_arguments_keep_dynamic_mutation_in_selected_operand() {
    let checked = checked_source(
        r#"
        machine stamp(value: &mut u64) -> bool { value = 7; true }
        machine consume(value: bool) { }
        machine enter(enabled: bool, output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(enabled && stamp(&mut scratch));
            output = scratch;
        }
    "#,
        false,
    );
    let state = entry(&checked);
    let statements = checked.statement_table.statements(state.statement_nodes);
    assert_eq!(statements.len(), 3);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("authored storage");
    };
    let plans = &checked.facts.values.scalar_computations;
    let CheckedScalarComputationKind::Select {
        condition,
        when_true,
        when_false,
        ..
    } = plans.nodes.get(root(&checked)).kind
    else {
        panic!("dynamic short-circuit selection");
    };
    assert_eq!(
        plans.nodes.get(condition).kind,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::Parameter { position: 0 }
        )))
    );
    assert_eq!(
        plans.nodes.get(when_false).kind,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::Constant(false)
        )))
    );
    let CheckedScalarComputationKind::Call {
        source_call,
        structural_arguments,
        ..
    } = plans.nodes.get(when_true).kind
    else {
        panic!("mutator exists only in selected RHS");
    };
    let call = checked.facts.flow.control.calls.get(source_call);
    assert_eq!(call.statement_index, 1);
    assert_eq!(call.call_ordinal, 1);
    assert_eq!(
        plans.nodes.get(when_true).primitive_type,
        PrimitiveType::Bool
    );
    let [argument] = plans
        .structural_arguments
        .span(structural_arguments)
        .unwrap()
    else {
        panic!("exact conditional borrow");
    };
    assert_eq!(
        argument.as_place().unwrap().source,
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
            symbol: local.symbol
        }
    );
    assert_eq!(
        argument.as_place().unwrap().access,
        CheckedStructuralAccess::MutableBorrow
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let unit = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("conditional operand remains in the Unit plan");
    assert!(unit.operations.iter().any(|operation| matches!(operation,
        CheckedUnitEffectOperationPlan::CallUnit { scalar_arguments, .. }
        if scalar_arguments == &[CheckedCallScalarArgument::Computation(root(&checked))])));
}

#[test]
fn borrowed_computation_arguments_keep_sibling_occurrences_on_the_same_local_distinct() {
    let checked = checked_source(
        r#"
        machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
        machine first(first: u64, second: u64) -> u64 { first }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(&mut output, first(stamp(&mut scratch, 7), stamp(&mut scratch, 11)));
            output = scratch;
        }
    "#,
        false,
    );
    let state = entry(&checked);
    let statements = checked.statement_table.statements(state.statement_nodes);
    assert_eq!(statements.len(), 3);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("one authored local");
    };
    let plans = &checked.facts.values.scalar_computations;
    let CheckedScalarComputationKind::Call { arguments, .. } = plans.nodes.get(root(&checked)).kind
    else {
        panic!("enclosing scalar invocation");
    };
    let operands = plans.operands.span(arguments).unwrap();
    assert_eq!(operands.len(), 2);
    assert_ne!(operands[0], operands[1]);
    let mut occurrences = Vec::new();
    for (operand, expected) in operands.iter().zip([7, 11]) {
        let CheckedScalarComputationKind::Call {
            source_call,
            target_machine,
            arguments,
            structural_arguments,
            ..
        } = plans.nodes.get(*operand).kind
        else {
            panic!("sibling mutator");
        };
        let call = checked.facts.flow.control.calls.get(source_call);
        assert_eq!(call.statement_index, 1);
        let [argument] = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("one sibling borrow");
        };
        assert_eq!(
            argument.as_place().unwrap().source,
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                symbol: local.symbol
            }
        );
        assert_eq!(
            argument.as_place().unwrap().access,
            CheckedStructuralAccess::MutableBorrow
        );
        let [value] = plans.operands.span(arguments).unwrap() else {
            panic!("one scalar actual");
        };
        let CheckedScalarComputationKind::Value(value) = &plans.nodes.get(*value).kind else {
            panic!("authored scalar literal");
        };
        assert_eq!(
            crate::values::evaluate_checked_scalar(value, &mut |_| None),
            Some(facts::ScalarValue::Integer(
                numerics::bignum::BigInt::from_i64(expected)
            ))
        );
        occurrences.push((
            source_call,
            target_machine,
            call.authored_expression,
            call.call_ordinal,
        ));
    }
    assert_ne!(occurrences[0].0, occurrences[1].0);
    assert_eq!(occurrences[0].1, occurrences[1].1, "one shared callee");
    assert_ne!(
        occurrences[0].2, occurrences[1].2,
        "distinct authored expressions"
    );
    assert!(
        occurrences[0].3 < occurrences[1].3,
        "authored sibling order"
    );
}

#[test]
fn borrowed_computation_arguments_retain_parameter_access_and_dense_positions() {
    for (source_access, target_access, actual, expected) in [
        ("&", "&", "value", CheckedStructuralAccess::SharedBorrow),
        (
            "&mut",
            "&mut",
            "&mut value",
            CheckedStructuralAccess::MutableBorrow,
        ),
        (
            "&mut",
            "&mut",
            "value",
            CheckedStructuralAccess::MutableBorrow,
        ),
        ("&mut", "&", "&value", CheckedStructuralAccess::SharedBorrow),
        (
            "&mut",
            "&write",
            "&write value",
            CheckedStructuralAccess::WriteOnlyBorrow,
        ),
        (
            "&write",
            "&write",
            "&write value",
            CheckedStructuralAccess::WriteOnlyBorrow,
        ),
    ] {
        let checked = checked_source(
            &format!(
                r#"
            machine sample(before: u64, value: {target_access} u64, after: u64) -> u64 {{ after }}
            machine consume(output: &mut u64, value: u64) {{ output = value; }}
            machine enter(seed: u64, output: &mut u64, value: {source_access} u64) {{
                let saved: u64 = seed;
                consume(&mut output, sample(saved, {actual}, seed));
            }}
        "#
            ),
            false,
        );
        let plans = &checked.facts.values.scalar_computations;
        let CheckedScalarComputationKind::Call {
            arguments,
            structural_arguments,
            source_call,
            ..
        } = plans.nodes.get(root(&checked)).kind
        else {
            panic!("mixed computation");
        };
        let operands = plans.operands.span(arguments).unwrap();
        assert_eq!(operands.len(), 2);
        assert_eq!(
            plans.nodes.get(operands[0]).kind,
            CheckedScalarComputationKind::Value(CheckedScalarExpression::Local {
                position: 1,
                primitive_type: PrimitiveType::U64
            })
        );
        assert_eq!(
            plans.nodes.get(operands[1]).kind,
            CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U64
            })
        );
        let [argument] = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("borrowed actual");
        };
        assert_eq!(
            argument.as_place().unwrap().source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
        );
        assert_eq!(argument.as_place().unwrap().access, expected);
        let call = checked.facts.flow.control.calls.get(source_call);
        let parameters = crate::call_target_parameters(&checked.typed, call.target_symbol).unwrap();
        assert_eq!(parameters.len(), 3);
        assert!(
            checked
                .primitive_type_reference(parameters[1].type_reference)
                .is_none()
        );
    }
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
                let target = crate::find_state(&program, call.target_symbol).unwrap();
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
fn borrowed_computation_arguments_admit_each_local_borrow_access() {
    for (access, expected) in [
        ("&", CheckedStructuralAccess::SharedBorrow),
        ("&mut", CheckedStructuralAccess::MutableBorrow),
        ("&write", CheckedStructuralAccess::WriteOnlyBorrow),
    ] {
        let checked = checked_source(
            &format!(
                r#"
            machine sample(value: {access} u64) -> u64 {{ 0 }}
            machine consume(output: &mut u64, value: u64) {{ output = value; }}
            machine enter(output: &mut u64) {{
                let mut scratch: u64 = 91;
                consume(&mut output, sample({access} scratch));
            }}
        "#
            ),
            false,
        );
        let plans = &checked.facts.values.scalar_computations;
        let CheckedScalarComputationKind::Call {
            structural_arguments,
            arguments,
            ..
        } = plans.nodes.get(root(&checked)).kind
        else {
            panic!("borrowed computation");
        };
        assert!(arguments.is_empty());
        let [argument] = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("whole primitive local");
        };
        assert!(
            matches!(argument.as_place().unwrap().source, CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } if symbol.is_valid())
        );
        assert_eq!(argument.as_place().unwrap().access, expected);
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
fn borrowed_computation_shared_occurrences_keep_scalar_reads_between_same_root_borrows() {
    for arguments in [
        "&scratch, scratch, &scratch",
        "&scratch, identity(scratch), &scratch",
    ] {
        let checked = shared_occurrence_fixture(arguments);
        let plans = &checked.facts.values.scalar_computations;
        let CheckedScalarComputationKind::Call {
            source_call,
            structural_arguments,
            arguments,
            ..
        } = plans.nodes.get(root(&checked)).kind
        else {
            panic!("hold call");
        };
        let structural = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap();
        assert_eq!(structural.len(), 2);
        assert_eq!(
            structural[0].as_place().unwrap().source,
            structural[1].as_place().unwrap().source
        );
        assert!(
            structural
                .iter()
                .all(|argument| argument.as_place().unwrap().access
                    == CheckedStructuralAccess::SharedBorrow)
        );
        assert_eq!(plans.operands.span(arguments).unwrap().len(), 1);
        let CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } =
            structural[0].as_place().unwrap().source
        else {
            panic!("local shared root");
        };
        let call = checked.facts.flow.control.calls.get(source_call);
        let rows = checked
            .facts
            .borrow
            .argument_accesses
            .span(call.accesses)
            .unwrap();
        assert_eq!(
            rows.len(),
            3,
            "scalar observation retains its intervening row"
        );
        assert!(rows.iter().all(|row| row.root_symbol == symbol
            && row.kind == checked_trees::BorrowAccessKind::Read
            && row.segments.is_empty()));
    }
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

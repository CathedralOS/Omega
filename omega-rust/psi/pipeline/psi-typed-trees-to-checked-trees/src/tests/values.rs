use super::*;
use psi_checked_trees::{CheckedScalarBindingValue, CheckedValueStatementRole};

#[test]
fn scalar_transition_argument_custody_keeps_exact_targets_and_source_bindings() {
    use psi_checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    };
    use psi_typed_trees::statement::TransitionTargetNode;

    for (declarations, signature, arguments, target_parameters) in [
        (
            "data Packet [copy] { value: u64; }",
            "machine transfer(packet: Packet, input: u8)",
            "packet, ((current as u8 in Wrapping) + 1) as u8, !flag, saved",
            "packet: Packet, next: u8, chosen: bool, prior: u8",
        ),
        (
            "data Root {}",
            "machine Root::transfer(&self, input: u8)",
            "((current as u8 in Wrapping) + 1) as u8, !flag, saved",
            "&self, next: u8, chosen: bool, prior: u8",
        ),
    ] {
        let source = format!(
            "{declarations} {signature} -> u8 {{ let mut current: u8 = 3; let mut flag: bool = false; let saved: u8 = current; transition {{ _ -> finish({arguments}) }} state finish({target_parameters}) -> u8 {{ prior }} }}"
        );
        let checked = lower_typed_trees(typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let states = checked.machine_states(&checked.machines()[0]);
        let state = &states[0];
        let target = &states[1];
        let parameters = checked.state_parameters(state);
        let target_parameters = checked.state_parameters(target);
        let statements = checked.statement_table.statements(state.statement_nodes);
        let [
            StatementNode::LocalData(current),
            StatementNode::LocalData(flag),
            StatementNode::LocalData(saved),
            StatementNode::Transition(transition),
        ] = statements
        else {
            panic!("three locals precede one explicit jump")
        };
        let TransitionTargetNode::Named { arguments, .. } =
            checked.statement_table.transition_target(transition.target)
        else {
            panic!("named target")
        };
        let arguments = checked.statement_table.expression_handles(*arguments);
        let scalar_arguments = &arguments[arguments.len() - 3..];
        let plans = &checked.facts.values.scalar_expressions;
        let rows: Vec<_> = plans
            .source_bindings
            .iter()
            .map(|(_, row)| row)
            .filter(|row| {
                matches!(
                    row.role,
                    CheckedScalarExpressionRole::TransitionArgument { .. }
                )
            })
            .collect();
        assert_eq!(
            rows.len(),
            3,
            "self and structural parameters are not scalar arguments"
        );
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.state, state.symbol);
            assert_eq!(row.statement_ordinal, 3);
            assert_eq!(row.expression, scalar_arguments[index]);
            assert_eq!(row.destination, target_parameters[index + 1].symbol);
            assert_eq!(
                row.role,
                CheckedScalarExpressionRole::TransitionArgument {
                    argument_ordinal: u32::try_from(index + 1).unwrap(),
                },
                "the target declaration position survives self/structural filtering"
            );
            assert_eq!(
                plans.binding_symbols.span_or_empty(row.symbols),
                &[parameters[1].symbol, saved.symbol],
                "mutable storage and structural parameters cannot shift immutable operands"
            );
        }
        assert!(matches!(
            plans.expression_at(rows[0].state, rows[0].statement_ordinal, rows[0].role),
            Some(CheckedScalarExpression::IntegerBinary { left, .. })
                if matches!(left.as_ref(), CheckedScalarExpression::StorageRead { symbol, .. }
                    if *symbol == current.symbol)
        ));
        assert!(matches!(
            plans.expression_at(rows[1].state, rows[1].statement_ordinal, rows[1].role),
            Some(CheckedScalarExpression::Boolean(expression))
                if matches!(expression.as_ref(), CheckedBooleanExpression::Not(operand)
                    if matches!(operand.as_ref(), CheckedBooleanExpression::StorageRead { symbol }
                        if *symbol == flag.symbol))
        ));
        assert!(matches!(
            plans.expression_at(rows[2].state, rows[2].statement_ordinal, rows[2].role),
            Some(CheckedScalarExpression::Local { position: 1, .. })
        ));
    }
}

#[test]
fn scalar_transition_continuation_has_independent_argument_custody() {
    use psi_checked_trees::CheckedScalarExpressionRole;
    use psi_typed_trees::statement::TransitionTargetNode;

    let mut program = typed_trees(
        r#"
        machine choose(flag: bool) -> u8 {
            let seed: u8 = 2;
            let mut current: u8 = 3;
            transition flag {
                true -> first(seed + 1)
                false -> second(current + 4)
            }
            state first(value: u8) -> u8 { value }
            state second(value: u8) -> u8 { value }
        }
        "#,
    );
    // Authored arms are separate statements. Exercise the same combined
    // primary/continuation representation used by retained transition plans.
    let machine = program.machines()[0].clone();
    let nodes = program.machine_states(&machine)[0].statement_nodes;
    let transitions: Vec<_> = program
        .statement_table
        .statements(nodes)
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            if let StatementNode::Transition(transition) = statement {
                Some((index, transition.target))
            } else {
                None
            }
        })
        .collect();
    let [
        (statement_index, primary),
        (continuation_index, continuation),
    ] = transitions.as_slice()
    else {
        panic!("two authored arms")
    };
    assert_eq!(*continuation_index, *statement_index + 1);
    assert_eq!(*continuation_index + 1, nodes.count() as usize);
    let StatementNode::Transition(transition) =
        &mut program.statement_table.statements_mut(nodes)[*statement_index]
    else {
        unreachable!()
    };
    transition.continuation = *continuation;
    program.machine_states_mut(&machine)[0].statement_nodes =
        psi_arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);

    let checked = lower_typed_trees(program).expect("both combined arms remain checked");
    let state = &checked.machine_states(&checked.machines()[0])[0];
    let plans = &checked.facts.values.scalar_expressions;
    for (target, role) in [
        (
            *primary,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0,
            },
        ),
        (
            *continuation,
            CheckedScalarExpressionRole::TransitionContinuationArgument {
                argument_ordinal: 0,
            },
        ),
    ] {
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = checked.statement_table.transition_target(target)
        else {
            panic!("named target")
        };
        let destination = checked
            .machine_states(&checked.machines()[0])
            .iter()
            .find(|candidate| candidate.symbol == path.symbol)
            .unwrap();
        let rows: Vec<_> = plans
            .source_bindings
            .iter()
            .map(|(_, row)| row)
            .filter(|row| row.state == state.symbol && row.role == role)
            .collect();
        assert_eq!(rows.len(), 1);
        let row = rows[0];
        assert_eq!(row.statement_ordinal as usize, *statement_index);
        assert_eq!(
            row.expression,
            checked.statement_table.expression_handles(*arguments)[0]
        );
        assert_eq!(
            row.destination,
            checked.state_parameters(destination)[0].symbol
        );
        assert!(
            plans
                .expression_at(state.symbol, row.statement_ordinal, role)
                .is_some()
        );
    }
}

#[test]
fn mutable_scalar_reads_require_consistent_exact_resolved_name_handles() {
    use psi_checked_trees::{
        CheckedBooleanExpression, CheckedOperatorFacts, CheckedScalarExpression,
        CheckedScalarExpressionRole,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::{expression::ExpressionNode, types::PrimitiveType};

    for (scalar_type, initial_value, primitive_type) in [
        ("u8", "7", PrimitiveType::U8),
        ("bool", "true", PrimitiveType::Bool),
    ] {
        let source = format!(
            "machine value(input: {scalar_type}) -> {scalar_type} {{ let mut current: {scalar_type} = {initial_value}; current }}"
        );
        let program = typed_trees(&source);
        let state = &program.machine_states(&program.machines()[0])[0];
        let state_symbol = state.symbol;
        let parameter = program.state_parameters(state)[0].symbol;
        let [
            StatementNode::LocalData(local),
            StatementNode::Expression(returned),
        ] = program.statement_table.statements(state.statement_nodes)
        else {
            panic!("one mutable declaration and its returned read")
        };
        let symbol = local.symbol;
        let returned = *returned;
        let expected = if primitive_type == PrimitiveType::Bool {
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::StorageRead {
                symbol,
            }))
        } else {
            CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            }
        };
        let plans = crate::values::build_checked_scalar_expression_plans(
            &program,
            &CheckedOperatorFacts::default(),
            &[],
        );
        assert_eq!(
            plans.expression_at(state_symbol, 1, CheckedScalarExpressionRole::Return),
            Some(&expected)
        );
        let stale = SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1);
        let missing = SymbolHandle::invalid();
        for (target, head) in [
            (stale, stale),
            (stale, symbol),
            (symbol, stale),
            (parameter, symbol),
            (symbol, parameter),
            (missing, missing),
            (missing, symbol),
            (symbol, missing),
        ] {
            let mut changed = program.clone();
            let ExpressionNode::Name(path) = changed.expression_table.expression_mut(returned)
            else {
                panic!("returned scalar has a resolved name")
            };
            path.symbol = target;
            path.head_symbol = head;
            let plans = crate::values::build_checked_scalar_expression_plans(
                &changed,
                &CheckedOperatorFacts::default(),
                &[],
            );
            assert_eq!(
                plans.expression_at(state_symbol, 1, CheckedScalarExpressionRole::Return),
                None,
                "{scalar_type}: target {target:?}, head {head:?} cannot recover storage from spelling"
            );
        }
    }
}

#[test]
fn scalar_return_custody_retains_filtered_parameters_and_dense_prior_locals() {
    use psi_checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    };
    use psi_typed_trees::types::PrimitiveType;

    for (result_type, returned, expected) in [
        (
            "u8",
            "count",
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U8,
            },
        ),
        (
            "bool",
            "flag",
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Parameter {
                position: 1,
            })),
        ),
        (
            "bool",
            "chosen",
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Local {
                position: 2,
            })),
        ),
        (
            "u8",
            "second",
            CheckedScalarExpression::Local {
                position: 4,
                primitive_type: PrimitiveType::U8,
            },
        ),
    ] {
        let source = format!(
            "data Packet [copy] {{ value: u64; }} machine mixed(packet: Packet, count: u8, bytes: [u8; 2], flag: bool) -> {result_type} {{ let owned: Packet = packet; let mut scratch: u8 = 9; let chosen: bool = flag; let first: u8 = count; let second: u8 = first; {returned} }}"
        );
        let checked = lower_typed_trees(typed_trees(&source)).expect("mixed return source checks");
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "mixed")
            .unwrap();
        let state = &checked.machine_states(machine)[0];
        let parameters = checked.state_parameters(state);
        let statements = checked.statement_table.statements(state.statement_nodes);
        let locals: Vec<_> = statements
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) => Some(local),
                _ => None,
            })
            .collect();
        let StatementNode::Expression(source_expression) = statements.last().unwrap() else {
            panic!("fixture must retain an ordinary terminal expression")
        };
        let plans = &checked.facts.values.scalar_expressions;
        let bindings: Vec<_> = plans
            .source_bindings
            .iter()
            .map(|(_, binding)| binding)
            .filter(|binding| binding.role == CheckedScalarExpressionRole::Return)
            .collect();
        let [binding] = bindings.as_slice() else {
            panic!("one selected return must have one source binding row: {bindings:?}")
        };
        assert_eq!(binding.state, state.symbol);
        assert_eq!(binding.statement_ordinal, 5);
        assert_eq!(binding.role, CheckedScalarExpressionRole::Return);
        assert_eq!(binding.expression, *source_expression);
        assert_eq!(
            plans.binding_symbols.span_or_empty(binding.symbols),
            &[
                parameters[1].symbol,
                parameters[3].symbol,
                locals[2].symbol,
                locals[3].symbol,
                locals[4].symbol,
            ],
            "structural parameters/locals and mutable scalar locals cannot shift the selected dense namespace"
        );
        assert_eq!(
            plans.expression_at(binding.state, binding.statement_ordinal, binding.role),
            Some(&expected)
        );
        let initializers: Vec<_> = plans
            .source_bindings
            .iter()
            .map(|(_, row)| row)
            .filter(|row| {
                matches!(
                    row.role,
                    CheckedScalarExpressionRole::LocalInitializer { .. }
                )
            })
            .collect();
        assert_eq!(initializers.len(), 3);
        let storage: Vec<_> = plans
            .source_bindings
            .iter()
            .map(|(_, row)| row)
            .filter(|row| row.role == CheckedScalarExpressionRole::StorageInitializer)
            .collect();
        assert_eq!(storage.len(), 1);
        assert_eq!(storage[0].expression, locals[1].initial_value);
        assert_eq!(storage[0].statement_ordinal, 1);
        assert_eq!(
            plans.binding_symbols.span_or_empty(storage[0].symbols),
            &[parameters[1].symbol, parameters[3].symbol]
        );
        for (position, row) in initializers.iter().enumerate() {
            assert_eq!(row.state, state.symbol);
            assert_eq!(row.statement_ordinal as usize, position + 2);
            assert_eq!(row.expression, locals[position + 2].initial_value);
            assert_eq!(
                row.role,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: position as u32,
                }
            );
            assert_eq!(
                plans.binding_symbols.span_or_empty(row.symbols),
                &plans.binding_symbols.span_or_empty(binding.symbols)[..position + 2],
                "each initializer sees only parameters and earlier immutable scalar locals"
            );
        }
    }
}

#[test]
fn scalar_return_custody_keeps_same_spelling_state_bindings_and_source_occurrences_distinct() {
    use psi_checked_trees::{CheckedScalarExpression, CheckedScalarExpressionRole};
    use psi_typed_trees::{statement::TransitionTargetNode, types::PrimitiveType};

    let checked = lower_typed_trees(typed_trees(
        r#"
        data Packet [copy] { value: u64; }
        machine choose(packet: Packet, input: u8, flag: bool) -> u8 {
            transition flag {
                true -> left(packet, input)
                false -> right(packet, input)
            }
            state left(packet: Packet, current: u8) -> u8 {
                let owned: Packet = packet;
                let saved: u8 = current;
                transition { _ -> saved }
            }
            state right(packet: Packet, current: u8) -> u8 {
                let owned: Packet = packet;
                let saved: u8 = current;
                transition { _ -> saved }
            }
        }
    "#,
    ))
    .expect("explicit state argument source checks");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .unwrap();
    let states = checked.machine_states(machine);
    let plans = &checked.facts.values.scalar_expressions;
    assert_eq!(
        plans
            .source_bindings
            .iter()
            .filter(|(_, binding)| binding.role == CheckedScalarExpressionRole::Return)
            .count(),
        2
    );
    assert!(
        !plans
            .source_bindings
            .iter()
            .any(|(_, binding)| binding.state == states[0].symbol
                && binding.role == CheckedScalarExpressionRole::Return),
        "named transitions are not value-return source bindings"
    );
    let mut prior = None;
    for name in ["left", "right"] {
        let state = states
            .iter()
            .find(|state| state.name.as_str() == name)
            .unwrap();
        let statements = checked.statement_table.statements(state.statement_nodes);
        let StatementNode::LocalData(local) = &statements[1] else {
            panic!("scalar local")
        };
        let StatementNode::Transition(transition) = &statements[2] else {
            panic!("explicit value transition")
        };
        let TransitionTargetNode::Value(expression) =
            checked.statement_table.transition_target(transition.target)
        else {
            panic!("value target")
        };
        let rows: Vec<_> = plans
            .source_bindings
            .iter()
            .map(|(_, binding)| binding)
            .filter(|binding| {
                binding.state == state.symbol && binding.role == CheckedScalarExpressionRole::Return
            })
            .collect();
        let [binding] = rows.as_slice() else {
            panic!("one exact row per return state")
        };
        let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
        assert_eq!(binding.expression, *expression);
        assert_eq!(binding.statement_ordinal, 2);
        assert_eq!(binding.role, CheckedScalarExpressionRole::Return);
        assert_eq!(
            symbols,
            &[checked.state_parameters(state)[1].symbol, local.symbol]
        );
        assert_eq!(
            plans.expression_at(state.symbol, 2, CheckedScalarExpressionRole::Return),
            Some(&CheckedScalarExpression::Local {
                position: 1,
                primitive_type: PrimitiveType::U8
            })
        );
        if let Some((previous_state, previous_expression, previous_local)) = prior {
            assert_ne!(binding.state, previous_state);
            assert_ne!(binding.expression, previous_expression);
            assert_ne!(symbols[1], previous_local);
        }
        prior = Some((binding.state, binding.expression, symbols[1]));
    }
}

#[test]
fn transition_scalar_facts_skip_implicit_self_but_retain_target_position() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self, first: bool, second: bool) {
            transition first { true -> dispatch(second) _ -> done() }

            state dispatch(&mut self, flag: bool) {
                transition flag { true -> done() _ -> done() }
            }

            state done(&mut self) {}
        }
    "#;

    let checked = lower_typed_trees(typed_trees(source))
        .expect("implicit-self transition arguments should reach checked lowering");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let entry = &checked.machine_states(machine)[0];
    assert_eq!(
        checked.facts.values.scalar_expressions.expression_at(
            entry.symbol,
            0,
            psi_checked_trees::CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 1,
            },
        ),
        Some(&psi_checked_trees::CheckedScalarExpression::Boolean(
            Box::new(psi_checked_trees::CheckedBooleanExpression::Parameter { position: 1 })
        )),
        "the dense scalar expression must retain the target parameter's raw position",
    );
    assert_eq!(
        checked.facts.values.scalar_expressions.expression_at(
            entry.symbol,
            0,
            psi_checked_trees::CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0,
            },
        ),
        None,
        "implicit self must not masquerade as an authored scalar argument",
    );
}

#[test]
fn checked_scalar_graph_retains_direct_call_bindings_and_arguments() {
    let source = r#"
        machine identity(value: bool) -> bool { value }

        machine caller(flag: bool) -> bool {
            let forwarded: bool = identity(flag);
            forwarded
        }
    "#;

    let checked = lower_typed_trees(typed_trees(source))
        .expect("the direct scalar call should reach checked lowering");
    let caller = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "caller")
        .expect("caller terminal selection");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(caller.machine)
        .expect("caller scalar graph");
    let [state] = graph.states.as_slice() else {
        panic!("caller should retain one scalar state")
    };
    let [binding] = state.bindings.as_slice() else {
        panic!("caller should retain one scalar binding")
    };
    let CheckedScalarBindingValue::DirectCall {
        target_machine,
        target_state,
        call_ordinal,
        argument_count,
    } = binding.value
    else {
        panic!("the call-valued local must not masquerade as a pure expression")
    };
    assert_eq!(call_ordinal, 0);
    assert_eq!(argument_count, 1);
    assert_ne!(target_machine, caller.machine);
    assert!(target_state.is_valid());
    let checked_call = checked
        .facts
        .contract_plans
        .for_machine(caller.machine)
        .and_then(|plan| {
            plan.crash
                .checked_call_at(state.state, binding.statement_ordinal, call_ordinal)
        })
        .expect("the binding coordinate should join to checked call refinement");
    assert_eq!(checked_call.target_machine(), target_machine);
    assert_eq!(checked_call.target_state(), target_state);
    assert_eq!(
        checked.facts.values.scalar_expressions.expression_at(
            state.state,
            binding.statement_ordinal,
            psi_checked_trees::CheckedScalarExpressionRole::CallArgument {
                binding_ordinal: 0,
                argument_ordinal: 0,
            },
        ),
        Some(&psi_checked_trees::CheckedScalarExpression::Boolean(
            Box::new(psi_checked_trees::CheckedBooleanExpression::Parameter { position: 0 },)
        )),
        "the argument must be expressed in the caller's checked scalar namespace",
    );
}

#[test]
fn materializes_checked_value_facts_for_statement_expressions() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self, input: i32) -> i32 {
            let local: i32 = input + 1;
            self.value = local;
            self.echo(local);
            transition {
                local > 0 -> local
                _ -> self.value
            }
        }

        machine Main::echo(&mut self, value: i32) {
        }
    "#;

    let typed = typed_trees(source);
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let values = build_value_facts(&typed, &proof_plan);

    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            psi_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::LocalInitializer,
                ..
            }
        )),
        "local initializer should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            psi_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::AssignmentValue,
                ..
            }
        )),
        "assignment value should be visible as a checked value"
    );
    let assignment_value = values
        .values
        .iter()
        .map(|(_, value)| value)
        .find(|value| {
            matches!(
                value.origin,
                psi_checked_trees::CheckedValueOrigin::StateStatement {
                    role: CheckedValueStatementRole::AssignmentValue,
                    ..
                }
            )
        })
        .expect("assignment value fact");
    assert_eq!(
        typed.primitive_type_reference(assignment_value.type_reference),
        Some(psi_typed_trees::types::PrimitiveType::I32),
        "checked assignment values should retain their use-site declared type"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            psi_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::CallArgument,
                ..
            }
        )),
        "call argument should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            psi_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::TransitionGuard,
                ..
            }
        )),
        "transition guard should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            psi_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::TransitionTargetValue
                    | CheckedValueStatementRole::TransitionTargetArgument,
                ..
            }
        )),
        "transition target value should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            psi_checked_trees::CheckedValueOrigin::NestedExpression { .. }
        )),
        "nested expression values should preserve their parent relationship"
    );
}

#[test]
fn materializes_checked_value_facts_for_machine_decreases() {
    let source = r#"
        data Main {}

        machine Main::countdown(&mut self, remaining: u64)
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> self.countdown(remaining - 1)
                false -> 0
            }
        }
    "#;

    let typed = typed_trees(source);
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let values = build_value_facts(&typed, &proof_plan);
    let countdown = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::countdown")
        .expect("countdown machine");

    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            psi_checked_trees::CheckedValueOrigin::MachineDecrease {
                machine_symbol,
                ordinal: 0,
            } if machine_symbol == countdown.symbol
        )),
        "decreases clause should be visible as a checked value"
    );
}

#[test]
fn assignment_value_fact_retains_stable_guard_range() {
    let source = r#"
        data Main {
            source: i64;
            target: i64;
        }

        machine Main::main(&mut self) {
            transition self.source >= -128 && self.source <= 127 {
                true -> store()
                _ -> done()
            }

            state store(&mut self) {
                self.target = self.source;
            }

            state done(&mut self) {}
        }
    "#;

    let typed = typed_trees(source);
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    psi_proof::checker::check_proof_plan(&proof_plan).expect("guarded assignment should prove");
    let values = build_value_facts(&typed, &proof_plan);
    let guarded = values
        .values
        .iter()
        .map(|(_, value)| value)
        .find(|value| {
            matches!(
                value.origin,
                psi_checked_trees::CheckedValueOrigin::StateStatement {
                    role: CheckedValueStatementRole::AssignmentValue,
                    ..
                }
            )
        })
        .and_then(|value| value.integer_range.as_ref())
        .expect("guarded assignment should retain an integer range");
    assert_eq!(
        guarded.minimum,
        psi_numerics::bignum::BigInt::from_i64(-128)
    );
    assert_eq!(guarded.maximum, psi_numerics::bignum::BigInt::from_i64(127));
}

#[test]
fn checked_scalar_plan_retains_guard_proved_exact_integer_cast_range() {
    let source = r#"
        machine narrow(value: u64) -> u8 {
            transition value <= 255u64 {
                true -> finish(value as u8)
                _ -> finish(0u8)
            }

            state finish(value: u8) -> u8 { value }
        }
    "#;

    let checked = lower_typed_trees(typed_trees(source))
        .expect("the dominating guard proves the exact narrowing cast");
    let cast = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find_map(|located| match &located.expression {
            psi_checked_trees::CheckedScalarExpression::IntegerExactCast { range, .. } => {
                Some(range)
            }
            _ => None,
        })
        .expect("checked scalar facts should retain the exact cast");
    assert_eq!(cast.minimum, psi_numerics::bignum::BigInt::from_i64(0));
    assert_eq!(cast.maximum, psi_numerics::bignum::BigInt::from_i64(255));
}

#[test]
fn boolean_integer_cast_keeps_binary_range_for_exact_shift() {
    let source = r#"
        machine encode(flag: bool) -> i32 {
            (flag as i32) << 30
        }
    "#;

    lower_typed_trees(typed_trees(source))
        .expect("a Boolean integer cast is confined to zero or one before the exact shift");
}

#[test]
fn exact_integer_widen_keeps_source_range_for_exact_shift() {
    let source = r#"
        machine stride(width: u32) -> i64 {
            (width as i64) << 2
        }
    "#;

    lower_typed_trees(typed_trees(source))
        .expect("the exact widening retains u32 bounds before the exact shift");
}

#[test]
fn checked_scalar_plan_retains_guard_proved_exact_right_shift() {
    let source = r#"
        machine shift(value: u64, count: u64) -> u64 {
            transition count <= 63u64 {
                true -> finish(value >> count)
                _ -> finish(0u64)
            }

            state finish(value: u64) -> u64 { value }
        }
    "#;

    let checked = lower_typed_trees(typed_trees(source))
        .expect("the dominating guard proves the exact right-shift count");
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter()
            .any(|located| matches!(
                located.expression,
                psi_checked_trees::CheckedScalarExpression::IntegerBinary {
                    kind: psi_checked_trees::CheckedIntegerBinaryKind::ExactShiftRight,
                    ..
                }
            ))
    );
}

#[test]
fn checked_scalar_plan_retains_guard_proved_exact_left_shift() {
    let source = r#"
        machine shift(value: u32, count: u32) -> u32 {
            transition count <= 31u32 {
                true -> prove_value(value, count)
                _ -> finish(0u32)
            }

            state prove_value(value: u32, count: u32) -> u32 {
                transition value <= 1u32 {
                    true -> finish(value << count)
                    _ -> finish(0u32)
                }
            }

            state finish(value: u32) -> u32 { value }
        }
    "#;

    let checked = lower_typed_trees(typed_trees(source))
        .expect("the dominating guards prove exact left-shift count and value safety");
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter()
            .any(|located| matches!(
                located.expression,
                psi_checked_trees::CheckedScalarExpression::IntegerBinary {
                    kind: psi_checked_trees::CheckedIntegerBinaryKind::ExactShiftLeft,
                    ..
                }
            ))
    );
}

fn typed_trees(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

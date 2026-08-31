use super::*;
use psi_checked_trees::{CheckedScalarBindingValue, CheckedValueStatementRole};

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

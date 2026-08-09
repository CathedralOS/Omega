use super::*;
use psi_checked_trees::CheckedValueStatementRole;

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

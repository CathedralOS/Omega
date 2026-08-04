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
    let values = build_value_facts(&typed);

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
    let values = build_value_facts(&typed);
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

fn typed_trees(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

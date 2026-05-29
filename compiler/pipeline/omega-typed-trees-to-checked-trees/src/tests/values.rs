use super::*;
use omega_checked_trees::CheckedValueStatementRole;

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
            omega_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::LocalInitializer,
                ..
            }
        )),
        "local initializer should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            omega_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::AssignmentValue,
                ..
            }
        )),
        "assignment value should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            omega_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::CallArgument,
                ..
            }
        )),
        "call argument should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            omega_checked_trees::CheckedValueOrigin::StateStatement {
                role: CheckedValueStatementRole::TransitionGuard,
                ..
            }
        )),
        "transition guard should be visible as a checked value"
    );
    assert!(
        values.values.iter().any(|(_, value)| matches!(
            value.origin,
            omega_checked_trees::CheckedValueOrigin::StateStatement {
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
            omega_checked_trees::CheckedValueOrigin::NestedExpression { .. }
        )),
        "nested expression values should preserve their parent relationship"
    );
}

#[test]
fn materializes_checked_value_facts_for_machine_decreases() {
    let source = r#"
        data Main {}

        machine Main::countdown(&mut self, remaining: usize)
        terminates {
            decreases remaining -> Nat::Descending;
        }
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
            omega_checked_trees::CheckedValueOrigin::MachineDecrease {
                machine_symbol,
                ordinal: 0,
            } if machine_symbol == countdown.symbol
        )),
        "decreases clause should be visible as a checked value"
    );
}

#[test]
fn materializes_checked_value_facts_for_machine_owned_initializers() {
    let source = r#"
        data Main {
            left: i32 = 1;
            right: String = "omega";
        }

        machine Main::main(&mut self) {
        }
    "#;

    let typed = typed_trees(source);
    let values = build_value_facts(&typed);
    let main = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let attached_data = main.attached_data.as_ref().expect("attached data");
    let data = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name == *attached_data)
        .expect("attached data definition");
    let initialized_fields = typed
        .data_members(data)
        .iter()
        .filter(|member| {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                return false;
            };
            field.initial_value.is_valid()
        })
        .count();

    let initializer_values = values
        .values
        .iter()
        .filter(|(_, value)| {
            matches!(
                value.origin,
                omega_checked_trees::CheckedValueOrigin::MachineOwnedDataInitializer {
                    machine_symbol,
                    ..
                } if machine_symbol == main.symbol
            )
        })
        .count();

    assert_eq!(initializer_values, initialized_fields);
    assert_eq!(initializer_values, 2);
}

fn typed_trees(source: &str) -> omega_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

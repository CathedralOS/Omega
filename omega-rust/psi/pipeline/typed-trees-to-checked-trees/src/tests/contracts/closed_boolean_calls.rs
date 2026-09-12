use super::*;

fn guarded_call(requirement: &str) -> typed_trees::TypedTrees {
    parse_typed_trees(&format!(
        r#"
        machine identity(input: bool) -> bool
        requires {requirement}
        {{ input }}

        machine caller(flag: bool, other: bool) -> bool {{
            transition flag {{
                true -> (identity(other))
                false -> (false)
            }}
        }}
        "#,
    ))
}

#[test]
fn closed_boolean_requirements_are_proved_without_inherited_contract_facts() {
    for requirement in [
        "true == true",
        "false == false",
        "true != false",
        "!(true == false)",
        "(true == true) && (false != true)",
    ] {
        lower_typed_trees(guarded_call(requirement))
            .unwrap_or_else(|diagnostics| panic!("{requirement}: {diagnostics:#?}"));
    }
}

#[test]
fn false_or_unknown_boolean_requirements_do_not_gain_a_proof() {
    for requirement in [
        "true == false",
        "false != false",
        "!(true == true)",
        "(true == true) && (false == true)",
        "input == true",
    ] {
        let diagnostics = lower_typed_trees(guarded_call(requirement))
            .expect_err("a false or unknown requirement must remain unproved");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("cannot prove requires contract for call identity")
                    || (diagnostic.message.contains("call to `identity`")
                        && diagnostic.message.contains("violates required fact"))
            }),
            "{requirement}: {diagnostics:#?}"
        );
    }
}

#[test]
fn immutable_boolean_normal_result_equality_preserves_exact_return_identity() {
    for (guarantee, body) in [
        ("result == value", "value"),
        ("value == result", "value"),
        ("result == value", "Host::finish(false); value"),
    ] {
        let program = parse_typed_trees(&format!(
            "boundary trait Host {{ machine finish(value: bool) reaches Host; }}
             machine identity(value: bool) -> bool
             ensures {guarantee}
             reaches Host
             {{ {body} }}"
        ));
        lower_typed_trees(program)
            .unwrap_or_else(|diagnostics| panic!("{guarantee}, {body}: {diagnostics:#?}"));
    }
}

#[test]
fn boolean_normal_result_equality_cannot_substitute_another_value_or_mutable_entry() {
    for (parameters, body) in [
        ("value: bool, other: bool", "other"),
        ("value: bool", "!value"),
        ("mut value: bool", "value"),
    ] {
        let program = parse_typed_trees(&format!(
            "machine identity({parameters}) -> bool ensures result == value {{ {body} }}"
        ));
        let diagnostics = lower_typed_trees(program)
            .expect_err("a different result or unknown mutable post-state needs its own proof");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove ensures contract for exit from identity")),
            "{parameters}, {body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn authored_boolean_result_parameter_does_not_become_the_returned_value() {
    let program = parse_typed_trees(
        "machine identity(result: bool, value: bool) -> bool
         ensures result == value { value }",
    );
    let diagnostics = lower_typed_trees(program)
        .expect_err("the result-named formal is not the reserved normal result");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove ensures contract for exit from identity")),
        "{diagnostics:#?}"
    );
}

#[test]
fn computed_boolean_guarantees_compose_in_the_immutable_return_namespace() {
    for computed in [
        "!value",
        "value == other",
        "value && other",
        "value || other",
        "!(value == other)",
    ] {
        for guarantee in [
            format!("result == ({computed})"),
            format!("({computed}) == result"),
        ] {
            let program = parse_typed_trees(&format!(
                "boundary trait Host {{ machine finish(value: bool) reaches Host; }}
                 machine compute(marker: u16, other: bool, value: bool) -> bool
                 ensures {guarantee}
                 reaches Host
                 {{ Host::finish(false); {computed} }}"
            ));
            lower_typed_trees(program)
                .unwrap_or_else(|diagnostics| panic!("{guarantee}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn computed_boolean_guarantees_do_not_guess_values_or_replay_storage() {
    for (parameters, guarantee, body) in [
        ("value: bool, other: bool", "result == !value", "!other"),
        ("value: bool", "result == !value", "value"),
        ("mut value: bool", "result == !value", "!value"),
        ("result: bool, value: bool", "result == !value", "!value"),
    ] {
        let program = parse_typed_trees(&format!(
            "machine compute({parameters}) -> bool ensures {guarantee} {{ {body} }}"
        ));
        let diagnostics = lower_typed_trees(program)
            .expect_err("unrelated identities and unevidenced storage cannot prove the guarantee");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove ensures contract for exit from compute")),
            "{parameters}, {body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn saved_boolean_guarantees_follow_immutable_definition_order() {
    for body in [
        "let local: bool = value; !local",
        "let first: bool = !value; Host::finish(false); let second: bool = first; second",
        "let unrelated: u16 = marker; let first: bool = value; let second: bool = !first; second",
        "let unrelated: bool = Host::read(); let saved: bool = !value; saved",
        "let mut saved: bool = !value; saved",
        "let mut storage: bool = value; let saved: bool = !storage; saved",
        "let mut storage: bool = value; let saved: bool = !storage; storage = !storage; saved",
        "let mut storage: bool = value; storage = !storage; let saved: bool = storage; saved",
    ] {
        let program = parse_typed_trees(&format!(
            "boundary trait Host {{ machine finish(value: bool) reaches Host; machine read() -> bool reaches Host; }}
             machine compute(marker: u16, value: bool) -> bool
             ensures result == !value
             reaches Host {{ {body} }}"
        ));
        lower_typed_trees(program).unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn saved_boolean_guarantees_reject_borrowed_overwrites() {
    for body in [
        "let saved: bool = value; overwrite(&mut saved); saved",
        "let mut current: bool = value; overwrite(&mut current); let saved: bool = current; saved",
        "let mut current: bool = value; let alias: &mut bool = &mut current; alias = false; let saved: bool = current; saved",
        "let mut current: bool = value; current = other; let saved: bool = current; saved",
        "let saved: bool = value; first(clear(&mut saved), saved)",
    ] {
        let program = parse_typed_trees(&format!(
            "machine overwrite(destination: &mut bool) {{ destination = false; }}
             machine clear(destination: &mut bool) -> bool {{ destination = false; false }}
             machine first(ignored: bool, value: bool) -> bool ensures result == value {{ value }}
             machine compute(value: bool, other: bool) -> bool ensures result == value {{ {body} }}"
        ));
        let Err(diagnostics) = lower_typed_trees(program) else {
            panic!("overwritten binding retained its old value: {body}");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove ensures contract for exit from compute")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn saved_boolean_guarantees_do_not_confuse_storage_or_call_results_with_entry_values() {
    for (parameters, body) in [
        (
            "value: bool, other: bool",
            "let saved: bool = !other; saved",
        ),
        ("mut value: bool", "let saved: bool = !value; saved"),
        ("value: bool", "let saved: bool = identity(!value); saved"),
    ] {
        let program = parse_typed_trees(&format!(
            "machine identity(input: bool) -> bool {{ input }}
             machine compute({parameters}) -> bool ensures result == !value {{ {body} }}"
        ));
        let diagnostics = lower_typed_trees(program)
            .expect_err("unevidenced snapshots and calls remain unproved");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove ensures contract for exit from compute")),
            "{parameters}, {body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn saved_boolean_guarantees_require_exact_source_bound_definitions() {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CheckedScalarExpressionRole as Role,
    };
    let checked = lower_typed_trees(parse_typed_trees(
        "machine compute(value: bool, other: bool) -> bool ensures result == !value {
            let first: bool = !value;
            let second: bool = first;
            second
        }",
    ))
    .expect("saved guarantee");
    let first_role = Role::LocalInitializer { binding_ordinal: 0 };
    let second_role = Role::LocalInitializer { binding_ordinal: 1 };
    for mutation in 0..10 {
        let mut facts = checked.facts.clone();
        let plans = &mut facts.values.scalar_expressions;
        let (first, row) = plans
            .source_bindings
            .iter()
            .find(|(_, row)| row.role == first_role)
            .unwrap();
        let first_row = row.clone();
        let (second, row) = plans
            .source_bindings
            .iter()
            .find(|(_, row)| row.role == second_role)
            .unwrap();
        let second_row = row.clone();
        match mutation {
            0 => {
                plans.source_bindings.append(first_row);
            }
            1 => plans.source_bindings.get_mut(first).destination = second_row.destination,
            2 => plans.source_bindings.get_mut(first).state = symbols::SymbolHandle::invalid(),
            3 => plans.source_bindings.get_mut(first).statement_ordinal = 2,
            4 => plans.source_bindings.get_mut(first).expression = second_row.expression,
            5 => {
                let mut symbols = plans
                    .binding_symbols
                    .span_or_empty(first_row.symbols)
                    .to_vec();
                symbols.swap(0, 1);
                plans.source_bindings.get_mut(first).symbols =
                    plans.binding_symbols.insert_many(symbols);
            }
            6 => plans.source_bindings.get_mut(first).role = Role::StorageInitializer,
            7 => {
                let selected = plans
                    .expressions
                    .iter()
                    .find(|plan| plan.role == first_role)
                    .unwrap()
                    .clone();
                plans.expressions.push(selected);
            }
            8 => {
                // A purported reference to the later local has no identity in
                // the earlier definition's operand roster.
                plans
                    .expressions
                    .iter_mut()
                    .find(|plan| plan.role == first_role)
                    .unwrap()
                    .expression = Scalar::Boolean(Box::new(Boolean::Local { position: 3 }));
            }
            9 => {
                plans.source_bindings.get_mut(second).destination = first_row.destination;
            }
            _ => unreachable!(),
        }
        let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
            .expect_err("altered definition custody");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove ensures contract for exit from compute")),
            "mutation {mutation}: {diagnostics:#?}"
        );
    }
}

#[test]
fn mutable_boolean_snapshots_require_exact_store_custody() {
    use checked_trees::CheckedScalarExpressionRole as Role;
    let checked = lower_typed_trees(parse_typed_trees(
        "machine compute(value: bool) -> bool ensures result == !value {
            let mut current: bool = value;
            current = !current;
            let saved: bool = current;
            saved
        }",
    ))
    .expect("selected store snapshot");
    for mutation in 0..7 {
        let mut facts = checked.facts.clone();
        let plans = &mut facts.values.scalar_expressions;
        let (store, row) = plans
            .source_bindings
            .iter()
            .find(|(_, row)| row.role == Role::AssignmentValue)
            .unwrap();
        let row = row.clone();
        match mutation {
            0 => {
                plans.source_bindings.append(row);
            }
            1 => {
                plans.source_bindings.get_mut(store).destination = symbols::SymbolHandle::invalid()
            }
            2 => plans.source_bindings.get_mut(store).state = symbols::SymbolHandle::invalid(),
            3 => plans.source_bindings.get_mut(store).statement_ordinal = 0,
            4 => plans.source_bindings.get_mut(store).role = Role::StorageInitializer,
            5 => {
                plans.source_bindings.get_mut(store).expression =
                    typed_trees::expression::ExpressionHandle::invalid()
            }
            6 => {
                let selected = plans
                    .expressions
                    .iter()
                    .find(|plan| plan.role == Role::AssignmentValue)
                    .unwrap()
                    .clone();
                plans.expressions.push(selected);
            }
            _ => unreachable!(),
        }
        let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
            .expect_err("altered storage definition custody");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove ensures contract for exit from compute")),
            "mutation {mutation}: {diagnostics:#?}"
        );
    }
}

#[test]
fn saved_boolean_definition_expansion_has_a_shared_depth_limit() {
    for (count, accepted) in [(16, true), (80, false)] {
        let mut body = String::from("let saved0: bool = !value;");
        for position in 1..count {
            body.push_str(&format!(
                "let saved{position}: bool = saved{};",
                position - 1
            ));
        }
        body.push_str(&format!("saved{}", count - 1));
        let program = parse_typed_trees(&format!(
            "machine compute(value: bool) -> bool ensures result == !value {{ {body} }}"
        ));
        let result = lower_typed_trees(program);
        if accepted {
            result.expect("ordinary captured-local chain");
        } else {
            let diagnostics = result.expect_err("bounded denotation expansion");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove ensures contract for exit from compute")),
                "{diagnostics:#?}"
            );
        }
    }
}

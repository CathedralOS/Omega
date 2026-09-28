use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

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
        lower_typed_trees(guarded_call(requirement), &CheckingRequest::settled())
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
        let diagnostics = lower_typed_trees(guarded_call(requirement), &CheckingRequest::settled())
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
        let Err(diagnostics) = lower_typed_trees(program, &CheckingRequest::settled()) else {
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
fn saved_boolean_guarantees_require_exact_source_bound_definitions() {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CheckedScalarExpressionRole as Role,
    };
    let checked = lower_typed_trees(
        parse_typed_trees(
            "machine compute(value: bool, other: bool) -> bool ensures result == !value {
            let first: bool = !value;
            let second: bool = first;
            second
        }",
        ),
        &CheckingRequest::settled(),
    )
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
    let checked = lower_typed_trees(
        parse_typed_trees(
            "machine compute(value: bool) -> bool ensures result == !value {
            let mut current: bool = value;
            current = !current;
            let saved: bool = current;
            saved
        }",
        ),
        &CheckingRequest::settled(),
    )
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
        let result = lower_typed_trees(program, &CheckingRequest::settled());
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

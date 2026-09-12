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

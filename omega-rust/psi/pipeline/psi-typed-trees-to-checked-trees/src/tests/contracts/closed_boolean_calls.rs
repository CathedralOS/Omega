use super::*;

fn guarded_call(requirement: &str) -> psi_typed_trees::TypedTrees {
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

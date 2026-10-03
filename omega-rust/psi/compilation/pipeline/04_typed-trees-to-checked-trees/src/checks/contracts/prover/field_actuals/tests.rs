use crate::tests::front_end::checked_program_result;

#[test]
fn boolean_guard_polarity_preserves_exact_arrival_field_identity() {
    for (guard, selected_arm, accepted) in [
        ("!cell.flag", "false", true),
        ("!cell.flag", "true", false),
        ("cell.flag == false", "false", true),
        ("false == !cell.flag", "true", true),
        ("!!cell.flag", "true", true),
        ("!cell.other", "false", false),
        ("!other.flag", "false", false),
    ] {
        let source = format!(
            r#"
            data Cell {{ flag: bool; other: bool; }}
            machine caller(cell: &Cell, other: &Cell) {{
                transition {guard} {{
                    {selected_arm} -> selected(cell)
                    _ -> done()
                }}
                state selected(target: &Cell) requires target.flag {{}}
                state done() {{}}
            }}
            "#
        );
        match checked_program_result(&source) {
            Ok(_) => assert!(accepted, "wrong polarity or field accepted: {source}"),
            Err(diagnostics) => {
                assert!(!accepted, "{source}: {diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("cannot prove requires contract for call selected")),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}

fn source(access: &str, argument: &str, requirement: &str, before: &str, after: &str) -> String {
    let body = if access == "&" {
        ""
    } else {
        "target.flag = false;"
    };
    format!(
        r#"
        data Cell {{ flag: bool; other: bool; }}
        machine consume(target: {access} Cell)
        requires target.flag
        {{ {body} }}
        machine caller(cell: {access} Cell, other: {access} Cell)
        {requirement}
        {{ {before} consume({argument}); {after} }}
    "#
    )
}

#[test]
fn missing_wrong_and_invalidated_field_facts_cannot_supply_call_requirements() {
    for (requirement, argument, before, after) in [
        ("", "&write cell", "", ""),
        ("requires other.flag", "&write cell", "", ""),
        ("requires cell.other", "&write cell", "", ""),
        ("requires cell.flag", "&write other", "", ""),
        (
            "requires cell.flag",
            "&write cell",
            "cell.flag = false;",
            "",
        ),
        (
            "requires cell.flag",
            "&write cell",
            "",
            "consume(&write cell);",
        ),
    ] {
        let source = source("&write", argument, requirement, before, after);
        let Err(errors) = checked_program_result(&source) else {
            panic!("unproven field requirement accepted: {source}");
        };
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("cannot prove requires contract for call consume")),
            "{errors:?}"
        );
    }
}

#[test]
fn substituted_postcondition_subjects_keep_complete_paths_and_invalidation() {
    for (argument, before, accepted) in [
        ("cell", "", true),
        ("other", "", false),
        ("cell", "cell.flag = false;", false),
    ] {
        let source = format!(
            r#"
            data Cell {{ flag: bool; other: bool; }}
            machine set(target: &mut Cell)
            ensures target.flag
            {{ target.flag = true; }}
            machine consume(target: &Cell) requires target.flag {{}}
            machine caller(cell: &mut Cell, other: &Cell) {{
                set(&mut cell);
                {before}
                consume({argument});
            }}
        "#
        );
        let result = checked_program_result(&source);
        assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
    }
    let source = r#"
        data Cell { flag: bool; other: bool; }
        machine set(target: &mut Cell)
        ensures target.flag
        { target.flag = true; }
        machine consume(target: &Cell) requires target.other {}
        machine caller(cell: &mut Cell) {
            set(&mut cell);
            consume(cell);
        }
    "#;
    assert!(checked_program_result(source).is_err());

    for (actual, accepted) in [("container.inner", true), ("container.other", false)] {
        let source = format!(
            r#"
            data Cell {{ flag: bool; }}
            data Container {{ inner: Cell; other: Cell; }}
            machine set(target: &mut Cell)
            ensures target.flag
            {{ target.flag = true; }}
            machine consume(target: &Cell) requires target.flag {{}}
            machine caller(container: &mut Container) {{
                set(&mut container.inner);
                consume({actual});
            }}
        "#
        );
        let result = checked_program_result(&source);
        assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
    }
}

#[test]
fn postcondition_boolean_operands_do_not_exchange_formal_subjects() {
    for (argument, accepted) in [("first", true), ("second", false)] {
        let source = format!(
            r#"
            data Cell {{ flag: bool; }}
            machine set(first: &mut Cell, second: &mut Cell)
            ensures first.flag
            ensures !second.flag
            {{ first.flag = true; second.flag = false; }}
            machine consume(value: &Cell) requires value.flag {{}}
            machine caller(first: &mut Cell, second: &mut Cell) {{
                set(&mut first, &mut second);
                consume({argument});
            }}
            "#,
        );
        let result = checked_program_result(&source);
        assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
    }
    let source = r#"
        data Cell { flag: bool; }
        machine relate(enabled: bool, first: &Cell, second: &Cell)
        requires enabled && (first.flag == second.flag)
        ensures enabled && (first.flag == second.flag)
        {}
        machine consume(value: &Cell) requires value.flag {}
        machine caller(enabled: bool, first: &Cell, second: &Cell)
        requires enabled && (first.flag == second.flag)
        {
            relate(enabled, first, second);
            consume(first);
        }
    "#;
    let valid = checked_program_result(&source.replace("consume(first);", ""));
    assert!(
        valid.is_ok(),
        "the relation itself is valid: {:?}",
        valid.err()
    );
    assert!(
        checked_program_result(source).is_err(),
        "a true scalar operand and equal unknown fields cannot prove either field true",
    );
}

#[test]
fn owned_constructor_boolean_field_supplies_contextual_cleanup_requirement() {
    let source = r#"
        data Token { ready: bool; }
        machine Token::drop(token: &mut Token) requires token.ready {
            token.ready = false;
        }
        machine consume(token: Token) requires token.ready {}
        data Main {}
        machine Main::main() {
            let token: Token = Token { ready: true };
            consume(token);
        }
    "#;
    checked_program_result(source)
        .expect("current constructed field satisfies owned actual requirement");
}

#[test]
fn boolean_field_snapshots_follow_exact_current_storage_at_calls() {
    for (before, argument, accepted) in [
        ("", "&cell", true),
        ("", "&other", false),
        ("cell.flag = false;", "&cell", false),
        ("clear(&mut cell);", "&cell", false),
        ("cell.other = false;", "&cell", true),
        ("clear(&mut other);", "&cell", true),
        ("cell.flag = false; cell.flag = true;", "&cell", true),
    ] {
        let source = format!(
            r#"
            data Cell {{ flag: bool; other: bool; }}
            machine clear(value: &mut Cell) {{ value.flag = false; }}
            machine consume(value: &Cell) requires value.flag {{}}
            machine caller() {{
                let mut cell: Cell = Cell {{ flag: true, other: true }};
                let mut other: Cell = Cell {{ flag: false, other: true }};
                {before}
                consume({argument});
            }}
        "#
        );
        let result = checked_program_result(&source);
        assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
    }
}

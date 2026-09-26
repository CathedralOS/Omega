use crate::tests::front_end::checked_program_result;

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

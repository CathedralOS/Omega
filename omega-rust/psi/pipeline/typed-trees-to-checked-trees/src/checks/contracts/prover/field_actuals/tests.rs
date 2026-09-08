use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolved source");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed source")
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
fn supplied_fields_follow_exact_renamed_referents_across_borrow_modes() {
    for access in ["&", "&mut", "&write"] {
        let source = source(
            access,
            &format!("{access} cell"),
            "requires cell.flag",
            "",
            "",
        );
        let result = crate::lower_typed_trees(typed(&source));
        assert!(
            result.is_ok(),
            "{access}: {}",
            result
                .err()
                .map(|errors| format!("{errors:?}"))
                .unwrap_or_default()
        );
    }
    let source = r#"
        data Cell { flag: bool; }
        data Outer { inner: Cell; }
        machine consume(target: &Cell) requires target.flag {}
        machine caller(container: &Outer) requires container.inner.flag {
            consume(&container.inner);
        }
    "#;
    crate::lower_typed_trees(typed(source))
        .expect("projected actual prepends its exact nominal path");
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
        let Err(errors) = crate::lower_typed_trees(typed(&source)) else {
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
fn exact_field_subjects_reject_stale_symbols_and_same_spelled_foreign_fields() {
    let program = typed(
        r#"
        data Cell { flag: bool; }
        data Foreign { flag: bool; }
        machine consume(target: &Cell, foreign: &Foreign) requires target.flag {}
    "#,
    );
    let machine = &program.machines()[0];
    let parameters = program.state_parameters(&program.machine_states(machine)[0]);
    let expression = program
        .machine_contracts(machine)
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("requirement");
    assert!(checked_place(&program, expression).is_some());
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        panic!("member");
    };
    let receiver = member.receiver;
    let original = member.member_symbol;
    let typed_trees::data::DataMember::Field(foreign) =
        &program.data_members(&program.data_definitions()[1])[0]
    else {
        panic!("foreign field");
    };
    for wrong in [
        SymbolHandle::invalid(),
        foreign.symbol,
        SymbolHandle::from_parts(original.arena_index(), original.generation() + 1),
    ] {
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(expression)
        else {
            unreachable!();
        };
        member.member_symbol = wrong;
        assert!(checked_place(&invalid, expression).is_none());
    }
    let mut invalid = program.clone();
    let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(receiver) else {
        panic!("formal");
    };
    name.symbol = parameters[1].symbol;
    name.head_symbol = parameters[1].symbol;
    assert!(checked_place(&invalid, expression).is_none());
}

#[test]
fn instantiated_postcondition_field_keeps_its_substituted_call_place() {
    let source = r#"
        data Cell { flag: bool; }
        machine set(target: &mut Cell)
        ensures target.flag
        { target.flag = true; }
        machine consume(target: &Cell) requires target.flag {}
        machine caller(cell: &mut Cell) {
            set(&mut cell);
            consume(cell);
        }
    "#;
    let result = crate::lower_typed_trees(typed(source));
    assert!(
        result.is_ok(),
        "exact post-call field fact: {:?}",
        result.err()
    );
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
        let result = crate::lower_typed_trees(typed(&source));
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
    assert!(crate::lower_typed_trees(typed(source)).is_err());

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
        let result = crate::lower_typed_trees(typed(&source));
        assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
    }
}

#[test]
fn implicit_receiver_requirements_keep_the_selected_receiver_owner() {
    for (requirement, accepted) in [("self.allowed", false), ("other.allowed", true)] {
        let source = format!(
            r#"
            data Counter {{ allowed: bool; }}
            machine Counter::restricted(&self) -> u64 requires self.allowed {{ 1 }}
            machine Counter::caller(&self, other: &Counter) -> u64
            requires {requirement}
            {{ other.restricted() }}
        "#
        );
        let result = crate::lower_typed_trees(typed(&source));
        assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
    }
}

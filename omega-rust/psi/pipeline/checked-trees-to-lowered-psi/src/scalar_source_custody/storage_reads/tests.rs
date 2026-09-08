use super::*;
use checked_trees::{CheckedIntegerComparisonKind, CheckedStructuralPredicatePathSegment};

fn fixture(body: &str, result: &str) -> CheckedTrees {
    fixture_with_declarations(body, result, "")
}

fn fixture_with_declarations(body: &str, result: &str, declarations: &str) -> CheckedTrees {
    let source = format!(
        "data Limits {{ limit: u64; spare: u64; allowed: bool; other: bool; }}
         {declarations}
         machine read(marker: u64, limits: Limits, alternate: Limits) -> {result} {{ {body} }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("checked")
}

fn source(checked: &CheckedTrees) -> (symbols::SymbolHandle, ExpressionHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .expect("read machine");
    let state = &checked.machine_states(machine)[0];
    let StatementNode::Expression(expression) = checked
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("return")
    else {
        panic!("expression return");
    };
    (state.symbol, *expression)
}

fn validate_return(
    checked: &CheckedTrees,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let (state, expression) = source(checked);
    validate_expression(checked, state, 0, expression, retained)
}

fn integer_field(position: u32, identity: &str) -> CheckedScalarExpression {
    CheckedScalarExpression::StructuralParameterField {
        parameter_position: position,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            identity.into(),
        )],
        primitive_type: PrimitiveType::U64,
    }
}

fn boolean_field(position: u32, identity: &str) -> CheckedBooleanExpression {
    CheckedBooleanExpression::StructuralParameterField {
        parameter_position: position,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            identity.into(),
        )],
    }
}

#[test]
fn owned_integer_field_rejects_same_typed_field_parameter_and_erasure() {
    let checked = fixture("limits.limit", "u64");
    let state = &checked.machine_states(&checked.machines()[0])[0];
    assert_eq!(
        checked.type_multiplicity(checked.state_parameters(state)[1].type_reference),
        Multiplicity::Affine,
        "ordinary records keep their authored ownership classification"
    );
    assert!(validate_return(&checked, &integer_field(1, "limit")).is_ok());
    for forged in [
        integer_field(1, "spare"),
        integer_field(2, "limit"),
        integer_field(0, "limit"),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U64,
        },
    ] {
        assert!(validate_return(&checked, &forged).is_err(), "{forged:?}");
    }
}

#[test]
fn cast_wrapped_owned_field_keeps_its_source_occurrence() {
    let checked = fixture("limits.limit as u64", "u64");
    assert!(validate_return(&checked, &integer_field(1, "limit")).is_ok());
    assert!(validate_return(&checked, &integer_field(1, "spare")).is_err());
}

#[test]
fn owned_boolean_operands_retain_field_and_parameter_occurrences() {
    let checked = fixture("limits.allowed && alternate.other", "bool");
    let expression = |left, right| {
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::And {
            left: Box::new(left),
            right: Box::new(right),
        }))
    };
    assert!(
        validate_return(
            &checked,
            &expression(boolean_field(1, "allowed"), boolean_field(2, "other"))
        )
        .is_ok()
    );
    assert!(
        validate_return(
            &checked,
            &expression(boolean_field(2, "other"), boolean_field(1, "allowed"))
        )
        .is_err()
    );
    assert!(
        validate_return(
            &checked,
            &expression(boolean_field(1, "other"), boolean_field(2, "other"))
        )
        .is_err()
    );
}

#[test]
fn owned_integer_comparison_preserves_reversed_operand_positions() {
    let checked = fixture("limits.limit > alternate.spare", "bool");
    let comparison = |left, right| {
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::LessThan,
            left: Box::new(left),
            right: Box::new(right),
        }))
    };
    assert!(
        validate_return(
            &checked,
            &comparison(integer_field(2, "spare"), integer_field(1, "limit"))
        )
        .is_ok()
    );
    assert!(
        validate_return(
            &checked,
            &comparison(integer_field(1, "limit"), integer_field(2, "spare"))
        )
        .is_err()
    );
}

#[test]
fn owned_field_requires_exact_resolved_member_and_parameter_symbols() {
    let checked = fixture("limits.limit", "u64");
    let (_, expression) = source(&checked);
    let ExpressionNode::Member(member) = checked.expression_table.expression(expression) else {
        panic!("member")
    };
    let receiver = member.receiver;
    let member_symbol = member.member_symbol;
    for symbol in [
        symbols::SymbolHandle::invalid(),
        symbols::SymbolHandle::from_parts(
            member_symbol.arena_index(),
            member_symbol.generation() + 1,
        ),
    ] {
        let mut changed = checked.clone();
        let ExpressionNode::Member(member) =
            changed.typed.expression_table.expression_mut(expression)
        else {
            panic!("member")
        };
        member.member_symbol = symbol;
        assert!(validate_return(&changed, &integer_field(1, "limit")).is_err());
    }
    let mut changed = checked.clone();
    let ExpressionNode::Name(name) = changed.typed.expression_table.expression_mut(receiver) else {
        panic!("parameter")
    };
    name.symbol = symbols::SymbolHandle::invalid();
    name.head_symbol = symbols::SymbolHandle::invalid();
    assert!(validate_return(&changed, &integer_field(1, "limit")).is_err());
}

#[test]
fn numbered_owned_field_uses_identity_instead_of_spelling() {
    let mut checked = fixture("limits.limit", "u64");
    let members = checked.data_definitions()[0].members;
    let checked_trees::data::DataMember::Field(field) =
        &mut checked.typed.tables.data_members.span_mut_or_empty(members)[0]
    else {
        panic!("field")
    };
    field.identity = Some(7);
    assert!(validate_return(&checked, &integer_field(1, "#7")).is_ok());
    assert!(validate_return(&checked, &integer_field(1, "limit")).is_err());
    assert!(validate_return(&checked, &integer_field(1, "#8")).is_err());
}

#[test]
fn whole_record_equality_expansion_preserves_normalized_field_correspondence() {
    let checked = fixture_with_declarations(
        "limits == alternate",
        "bool",
        "trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
         LimitsEquatable: Limits satisfies Equatable;",
    );
    let (state, expression) = source(&checked);
    let state = authored_state(&checked, state).expect("state").1;
    let mut reads = Vec::new();
    let mut member_paths = Vec::new();
    collect_authored_storage_reads(
        &checked,
        state,
        0,
        expression,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut reads,
        &mut member_paths,
    )
    .expect("source traversal");
    assert_eq!(
        reads.len(),
        8,
        "typing expands both operands of four field comparisons"
    );
    assert_eq!(member_paths.len(), 8);
    let (_, retained) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(state.symbol, 0, CheckedScalarExpressionRole::Return)
        .expect("normalized equality has an exact checked return expression");
    validate_return(&checked, retained).expect("normalized equality retains its field reads");

    let mut forged = retained.clone();
    let CheckedScalarExpression::Boolean(boolean) = &mut forged else {
        panic!("Boolean equality result");
    };
    fn first_comparison(
        expression: &mut CheckedBooleanExpression,
    ) -> &mut CheckedBooleanExpression {
        match expression {
            CheckedBooleanExpression::And { left, .. } => first_comparison(left),
            _ => expression,
        }
    }
    let CheckedBooleanExpression::IntegerComparison { left, .. } = first_comparison(boolean) else {
        panic!("first normalized comparison reads the integer limit field");
    };
    let CheckedScalarExpression::StructuralParameterField { path, .. } = left.as_mut() else {
        panic!("normalized comparison retains a structural field");
    };
    assert_eq!(
        path,
        &[CheckedStructuralPredicatePathSegment::Field("limit".into())]
    );
    *path = vec![CheckedStructuralPredicatePathSegment::Field("spare".into())];
    assert!(
        validate_return(&checked, &forged).is_err(),
        "normalization cannot authorize another same-typed field"
    );
}

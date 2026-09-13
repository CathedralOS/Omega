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
    checked_source(&source)
}

fn checked_source(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("checked")
}

#[test]
fn entry_mutable_formals_keep_parameter_identity_while_body_reads_keep_storage() {
    for (spelling, primitive) in [("i32", PrimitiveType::I32), ("bool", PrimitiveType::Bool)] {
        let checked = checked_source(&format!(
            "machine read(mut input: {spelling}) -> {spelling} {{ input }}"
        ));
        let (state, expression) = source(&checked);
        let source_state = &checked.machine_states(&checked.machines()[0])[0];
        let symbol = checked.state_parameters(source_state)[0].symbol;
        let (parameter, storage) = if primitive == PrimitiveType::Bool {
            (
                CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Parameter {
                    position: 0,
                })),
                CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::StorageRead {
                    symbol,
                })),
            )
        } else {
            (
                CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: primitive,
                },
                CheckedScalarExpression::StorageRead {
                    symbol,
                    primitive_type: primitive,
                },
            )
        };
        assert!(validate_entry_read_expression(&checked, state, expression, &parameter).is_ok());
        assert!(validate_entry_read_expression(&checked, state, expression, &storage).is_err());
        assert!(validate_expression(&checked, state, 0, expression, &storage).is_ok());
        assert!(validate_expression(&checked, state, 0, expression, &parameter).is_err());
    }
}

#[test]
fn entry_widest_integer_formals_reject_same_typed_substitution() {
    for (spelling, primitive) in [("i64", PrimitiveType::I64), ("u64", PrimitiveType::U64)] {
        let checked = checked_source(&format!(
            "machine read(first: {spelling}, second: {spelling}) -> {spelling} {{ first }}"
        ));
        let (state, expression) = source(&checked);
        let parameter = |position| CheckedScalarExpression::Parameter {
            position,
            primitive_type: primitive,
        };
        assert!(validate_entry_read_expression(&checked, state, expression, &parameter(0)).is_ok());
        assert!(
            validate_entry_read_expression(&checked, state, expression, &parameter(1)).is_err()
        );
        assert!(
            validate_entry_read_expression(&checked, state, expression, &parameter(2)).is_err()
        );
        assert!(
            validate_entry_read_expression(
                &checked,
                state,
                expression,
                &CheckedScalarExpression::Local {
                    position: 0,
                    primitive_type: primitive,
                },
            )
            .is_err()
        );
    }
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

fn ensures_left(checked: &CheckedTrees, owner: &str) -> (symbols::SymbolHandle, ExpressionHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == owner)
        .expect("contract owner");
    let contract = checked
        .machine_contracts(machine)
        .iter()
        .find(|contract| contract.kind == checked_trees::signature::SignatureContractKind::Ensures)
        .expect("normal guarantee");
    let [checked_trees::domain::ProofFact::Expression(expression)] =
        checked.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("expression guarantee");
    };
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(*expression) else {
        panic!("comparison guarantee");
    };
    (checked.machine_states(machine)[0].symbol, binary.left)
}

fn result_parameter(position: usize) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::U64,
    }
}

#[test]
fn normal_result_reads_keep_reserved_slot_separate_from_parameters_and_locals() {
    let checked =
        checked_source("machine read(input: u64) -> u64 ensures result == input { input }");
    let (state, expression) = ensures_left(&checked, "read");
    assert!(
        validate_normal_result_read_expression(&checked, state, expression, &result_parameter(1))
            .is_ok()
    );
    for forged in [
        result_parameter(0),
        result_parameter(2),
        CheckedScalarExpression::Local {
            position: 1,
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::StorageRead {
            symbol: checked.machines()[0].symbol,
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I64,
        },
    ] {
        assert!(
            validate_normal_result_read_expression(&checked, state, expression, &forged).is_err(),
            "{forged:?}"
        );
    }
    let local =
        checked_source("machine read(input: u64) -> u64 { let result: u64 = input; result }");
    let (local_state, local_expression) = source(&local);
    assert!(
        validate_normal_result_read_expression(
            &local,
            local_state,
            local_expression,
            &result_parameter(1)
        )
        .is_err()
    );

    let shadowed =
        checked_source("machine read(result: u64) -> u64 ensures result == result { result }");
    let (state, expression) = ensures_left(&shadowed, "read");
    assert!(
        validate_normal_result_read_expression(&shadowed, state, expression, &result_parameter(0))
            .is_ok()
    );
    assert!(
        validate_normal_result_read_expression(&shadowed, state, expression, &result_parameter(1))
            .is_err()
    );
}

#[test]
fn normal_result_reads_reject_foreign_owner_and_forged_resolved_identity() {
    let checked = checked_source(
        "machine read(input: u64) -> u64 ensures result == input { input }
         machine other(input: u64) -> u64 ensures result == input { input }",
    );
    let (state, expression) = ensures_left(&checked, "read");
    let (_, foreign) = ensures_left(&checked, "other");
    assert!(
        validate_normal_result_read_expression(&checked, state, foreign, &result_parameter(1))
            .is_err()
    );
    for component in 0..3 {
        let mut changed = checked.clone();
        let symbol = changed.machines()[0].symbol;
        let ExpressionNode::Name(mut path) = *changed.expression_table.expression(expression)
        else {
            panic!("reserved result");
        };
        match component {
            0 => path.symbol = symbol,
            1 => path.head_symbol = symbol,
            _ => {
                path.member_symbols = arena::HandleSpan::empty();
                changed
                    .typed
                    .expression_table
                    .push_name_path_member_symbol(&mut path.member_symbols, symbol);
            }
        }
        *changed.typed.expression_table.expression_mut(expression) = ExpressionNode::Name(path);
        assert!(
            validate_normal_result_read_expression(
                &changed,
                state,
                expression,
                &result_parameter(1)
            )
            .is_err(),
            "component {component}"
        );
    }
}

#[test]
fn normal_result_reads_do_not_reinterpret_mutable_post_state_as_entry() {
    let checked = checked_source("machine read(mut input: u64) -> u64 { input }");
    let (state, expression) = source(&checked);
    assert!(
        validate_entry_read_expression(&checked, state, expression, &result_parameter(0)).is_ok()
    );
    assert!(
        validate_normal_result_read_expression(&checked, state, expression, &result_parameter(0))
            .is_err()
    );
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
fn case_membership_rejects_substituted_case_subject_and_erasure() {
    let tokens = source_files_to_tokens::Lexer::new(
        "data Choice { case Empty; case Some(value: u32); }
         machine read(choice: &Choice, alternate: &Choice) -> bool {
             choice in Choice::Empty
         }",
    )
    .tokenize()
    .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let membership = |parameter_position, case: &str| {
        CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::StructuralCaseMembership {
                subject: checked_trees::CheckedStructuralParameterField {
                    parameter_position,
                    path: Vec::new(),
                },
                case: case.into(),
            },
        ))
    };
    assert!(validate_return(&checked, &membership(0, "Empty")).is_ok());
    for forged in [
        membership(0, "Some"),
        membership(1, "Empty"),
        membership(2, "Empty"),
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(true))),
    ] {
        assert!(validate_return(&checked, &forged).is_err(), "{forged:?}");
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
fn case_membership_receiver_rejoins_only_its_declaring_machine() {
    let checked = checked_source(
        "data Choice [copy] { case Empty; case Full; }
         machine Choice::read(&self) -> bool { self in Choice::Empty }
         machine Choice::other(&self) -> bool { self in Choice::Empty }",
    );
    let state = checked.machine_states(&checked.machines()[0])[0].symbol;
    let root = super::super::locate(&checked, state, 0, CheckedScalarExpressionRole::Return)
        .expect("receiver return");
    let membership = CheckedScalarExpression::Boolean(Box::new(
        CheckedBooleanExpression::StructuralCaseMembership {
            subject: checked_trees::CheckedStructuralParameterField {
                parameter_position: 0,
                path: Vec::new(),
            },
            case: "Empty".into(),
        },
    ));
    assert!(validate_expression(&checked, state, 0, root.expression, &membership).is_ok());
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(root.expression)
    else {
        panic!("case expression");
    };
    let receiver = binary.left;
    for change_head in [false, true] {
        let mut forged = checked.clone();
        let foreign = forged.machines()[1].symbol;
        let ExpressionNode::Name(name) = forged.typed.expression_table.expression_mut(receiver)
        else {
            panic!("receiver name");
        };
        if change_head {
            name.head_symbol = foreign;
        } else {
            name.symbol = foreign;
        }
        assert!(validate_expression(&forged, state, 0, root.expression, &membership).is_err());
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
        ReadScope::Body(0),
        expression,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut reads,
        &mut member_paths,
        &mut Vec::new(),
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

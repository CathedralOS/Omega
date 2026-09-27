use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    ExpressionHandle, ExpressionNode, LoweringError, PrimitiveType, StatementNode,
    validate_entry_read_expression, validate_expression, validate_normal_result_read_expression,
};
use checked_trees::CheckedStructuralPredicatePathSegment;

fn fixture(body: &str, result: &str) -> CheckedTrees {
    fixture_with_declarations(body, result, "")
}

fn fixture_with_declarations(body: &str, result: &str, declarations: &str) -> CheckedTrees {
    let source = format!(
        "data Limits {{ limit: u64; spare: u64; allowed: bool; other: bool; }}
         {declarations}
         machine read(marker: u64, limits: Limits, alternate: Limits) -> {result} {{ {body} }}"
    );
    crate::front_end::checked_program(&source)
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
    let checked = crate::front_end::checked_program(
        "machine read(input: u64) -> u64 ensures result == input { input }",
    );
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
    let local = crate::front_end::checked_program(
        "machine read(input: u64) -> u64 { let result: u64 = input; result }",
    );
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

    let shadowed = crate::front_end::checked_program(
        "machine read(result: u64) -> u64 ensures result == result { result }",
    );
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
    let checked = crate::front_end::checked_program(
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
    let checked =
        crate::front_end::checked_program("machine read(mut input: u64) -> u64 { input }");
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

#[test]
fn projected_case_membership_replays_exact_root_field_index_and_case() {
    let checked = crate::front_end::checked_program(
        "data Color [copy] { case Red; case Blue; }
         data Palette { colors: [Color; 3]; spare: [Color; 3]; }
         machine read(palette: &Palette, alternate: &Palette) -> bool {
             palette.colors[1] in Color::Blue
         }",
    );
    let membership = |parameter_position, field: &str, element_index, case: &str| {
        CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::StructuralCaseMembership {
                subject: checked_trees::CheckedStructuralParameterField {
                    parameter_position,
                    path: vec![
                        CheckedStructuralPredicatePathSegment::Field(field.into()),
                        CheckedStructuralPredicatePathSegment::FixedIndex(element_index),
                    ],
                },
                case: case.into(),
            },
        ))
    };
    assert!(validate_return(&checked, &membership(0, "colors", 1, "Blue")).is_ok());
    for forged in [
        membership(1, "colors", 1, "Blue"),
        membership(0, "spare", 1, "Blue"),
        membership(0, "colors", 0, "Blue"),
        membership(0, "colors", 3, "Blue"),
        membership(0, "colors", 1, "Red"),
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(true))),
    ] {
        assert!(validate_return(&checked, &forged).is_err(), "{forged:?}");
    }
}

#[test]
fn case_membership_receiver_rejoins_only_its_declaring_machine() {
    let checked = crate::front_end::checked_program(
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

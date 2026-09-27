use super::has_exact_case_membership_meaning;
use super::has_exact_parameter_case_membership_meaning;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

fn result_bound_comparison(program: &TypedTrees, machine_index: usize) -> ExpressionHandle {
    let contract = &program.machine_contracts(&program.machines()[machine_index])[0];
    let typed_trees::domain::ProofFact::Expression(expression) =
        program.proof_facts.get(contract.facts.start())
    else {
        panic!("result comparison fact");
    };
    *expression
}

#[test]
fn result_bound_operand_types_require_exact_primitive_and_field_contract_owners() {
    let mut program = crate::front_end::typed_program(
        "data Count { value: u64; }
         machine first() -> u64 ensures result < 8 { 4 }
         machine second() -> u64 ensures result < 8 { 4 }
         machine first_record() -> Count ensures result.value < 8 { Count { value: 4 } }
         machine second_record() -> Count ensures result.value < 8 { Count { value: 4 } }",
    );
    for (owner_index, foreign_index) in [(0, 1), (2, 3)] {
        let expression = result_bound_comparison(&program, owner_index);
        let ExpressionNode::Binary(comparison) = program.expression_table.expression(expression)
        else {
            panic!("result comparison");
        };
        let operand = comparison.left;
        let machine = program.machines()[owner_index].clone();
        let state = program.machine_states(&machine)[0].clone();
        let reference = super::operand_type(&program, &machine, Some(&state), operand)
            .expect("own result operand has its declared type");
        assert_eq!(
            program.primitive_type_reference(reference),
            Some(typed_trees::types::PrimitiveType::U64)
        );
        let foreign = &program.machines()[foreign_index];
        assert!(
            super::operand_type(
                &program,
                foreign,
                program.machine_states(foreign).first(),
                operand
            )
            .is_none(),
            "another machine with the same carrier cannot own this result occurrence"
        );

        let copied_node = program.expression_table.expression(operand).clone();
        let copied = program.expression_table.insert(copied_node);
        let copied_type = super::operand_type(&program, &machine, Some(&state), copied);
        if matches!(
            program.expression_table.expression(operand),
            ExpressionNode::Member(_)
        ) {
            // An exact retained root still determines its field type. Derived
            // type information grants no authored postcondition occurrence custody.
            assert_eq!(copied_type, Some(reference));
            assert!(crate::reserved_result_place(&program, copied).is_none());
            let foreign = &program.machines()[foreign_index];
            assert!(
                super::operand_type(
                    &program,
                    foreign,
                    program.machine_states(foreign).first(),
                    copied
                )
                .is_none()
            );
        } else {
            assert!(
                copied_type.is_none(),
                "a copied result root outside the authored contract has no owner"
            );
        }

        if let ExpressionNode::Member(mut member) =
            program.expression_table.expression(operand).clone()
        {
            let copied_receiver = program.expression_table.expression(member.receiver).clone();
            member.receiver = program.expression_table.insert(copied_receiver);
            let grafted = program
                .expression_table
                .insert(ExpressionNode::Member(member));
            assert!(
                super::operand_type(&program, &machine, Some(&state), grafted).is_none(),
                "a nominally correct field cannot authorize a grafted result root"
            );
        }
    }
}

fn membership_program() -> (TypedTrees, ExpressionHandle, ExpressionHandle) {
    let program = crate::front_end::typed_program(
        "data Choice [copy] { case Ready(value: u64); case Empty; }
         data Foreign [copy] { case Ready(value: u64); case Empty; }
         machine member(choice: Choice, choices: [Choice; 1]) -> bool {
             choice in Choice::Ready
         }
         machine foreign(choice: Foreign) -> bool { choice in Foreign::Ready }
         machine create() -> Choice { Choice::Ready { value: 37 } }",
    );
    let mut comparisons =
        program
            .expression_table
            .expression_entries()
            .filter_map(|(expression, node)| {
                matches!(node, ExpressionNode::Binary(_)).then_some(expression)
            });
    let member = comparisons.next().expect("membership");
    let foreign = comparisons.next().expect("foreign membership");
    assert!(comparisons.next().is_none());
    drop(comparisons);
    (program, member, foreign)
}

fn is_exact_membership(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(expression) else {
        panic!("membership comparison");
    };
    has_exact_case_membership_meaning(program, machine, Some(state), expression, comparison)
}

#[test]
fn parameter_membership_rejects_conflicting_retained_root_symbols() {
    let (mut program, member, _) = membership_program();
    let parameters = program
        .state_parameters(&program.machine_states(&program.machines()[0])[0])
        .to_vec();
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(member) else {
        panic!("membership comparison");
    };
    let comparison = *comparison;
    let ExpressionNode::Name(subject) = program.expression_table.expression_mut(comparison.left)
    else {
        panic!("parameter subject");
    };
    subject.head_symbol = parameters[1].symbol;
    assert!(!has_exact_parameter_case_membership_meaning(
        &program,
        &parameters,
        member,
        &comparison
    ));
    let ExpressionNode::Name(subject) = program.expression_table.expression_mut(comparison.left)
    else {
        panic!("parameter subject");
    };
    subject.head_symbol = subject.symbol;
    subject.member_symbols = arena::HandleSpan::empty();
    assert!(!has_exact_parameter_case_membership_meaning(
        &program,
        &parameters,
        member,
        &comparison
    ));
}

#[test]
fn membership_requires_the_complete_matching_selection_roster() {
    let (mut program, member, foreign) = membership_program();
    assert!(is_exact_membership(&program, member));
    let selections: Vec<_> = program
        .expression_table
        .authored_selection_occurrences(member)
        .collect();
    assert_eq!(selections.len(), 2);
    for occurrences in [vec![], vec![selections[0]], vec![selections[1]]] {
        let comparison = program.expression_table.expression(member).clone();
        let copy = program.expression_table.insert(comparison);
        program
            .expression_table
            .attach_authored_selection_occurrences(copy, occurrences);
        assert!(!is_exact_membership(&program, copy));
    }
    let foreign_selections: Vec<_> = program
        .expression_table
        .authored_selection_occurrences(foreign)
        .collect();
    let comparison = program.expression_table.expression(member).clone();
    let rebound = program.expression_table.insert(comparison);
    program
        .expression_table
        .attach_authored_selection_occurrences(rebound, foreign_selections.iter().copied());
    assert!(!is_exact_membership(&program, rebound));
    program
        .expression_table
        .attach_authored_selection_occurrences(member, foreign_selections);
    assert!(!is_exact_membership(&program, member));
}

#[test]
fn membership_constructor_subject_retains_its_own_owner_and_case() {
    let (mut program, member, foreign) = membership_program();
    let (constructor, literal) = program
        .expression_table
        .expression_entries()
        .find_map(|(expression, node)| match node {
            ExpressionNode::StructLiteral(literal) => Some((expression, literal.clone())),
            _ => None,
        })
        .expect("constructor subject");
    let ExpressionNode::Binary(foreign_comparison) = program.expression_table.expression(foreign)
    else {
        panic!("foreign membership");
    };
    let ExpressionNode::Name(foreign_case) = program
        .expression_table
        .expression(foreign_comparison.right)
    else {
        panic!("foreign case");
    };
    let foreign_case = *foreign_case;
    let ExpressionNode::Binary(comparison) = program.expression_table.expression_mut(member) else {
        panic!("membership comparison");
    };
    comparison.left = constructor;
    assert!(is_exact_membership(&program, member));
    for replacement in [
        typed_trees::expression::TableStructLiteral {
            type_symbol: foreign_case.head_symbol,
            ..literal.clone()
        },
        typed_trees::expression::TableStructLiteral {
            case_symbol: Some(foreign_case.symbol),
            ..literal.clone()
        },
        typed_trees::expression::TableStructLiteral {
            type_symbol: symbols::SymbolHandle::invalid(),
            ..literal.clone()
        },
        typed_trees::expression::TableStructLiteral {
            case_symbol: None,
            ..literal
        },
    ] {
        *program.expression_table.expression_mut(constructor) =
            ExpressionNode::StructLiteral(replacement);
        assert!(!is_exact_membership(&program, member));
    }
}

#[test]
fn generated_case_membership_requires_exact_nominal_subject_and_classifier() {
    let (mut program, member, foreign) = membership_program();
    let ExpressionNode::Binary(mut comparison) =
        program.expression_table.expression(member).clone()
    else {
        panic!("membership comparison");
    };
    comparison.operator = BinaryOperator::CaseMembership;
    let generated = program
        .expression_table
        .insert(ExpressionNode::Binary(comparison));
    assert!(
        program
            .expression_table
            .authored_selection_occurrences(generated)
            .next()
            .is_none()
    );
    assert!(is_exact_membership(&program, generated));
    let ExpressionNode::Binary(foreign_comparison) = program.expression_table.expression(foreign)
    else {
        panic!("foreign membership");
    };
    let foreign_comparison = *foreign_comparison;
    for replacement in [
        typed_trees::expression::TableBinaryExpression {
            operator: BinaryOperator::Equal,
            ..comparison
        },
        typed_trees::expression::TableBinaryExpression {
            left: comparison.right,
            ..comparison
        },
        typed_trees::expression::TableBinaryExpression {
            left: foreign_comparison.left,
            ..comparison
        },
        typed_trees::expression::TableBinaryExpression {
            right: foreign_comparison.right,
            ..comparison
        },
        typed_trees::expression::TableBinaryExpression {
            left: ExpressionHandle::invalid(),
            ..comparison
        },
        typed_trees::expression::TableBinaryExpression {
            right: ExpressionHandle::invalid(),
            ..comparison
        },
    ] {
        *program.expression_table.expression_mut(generated) = ExpressionNode::Binary(replacement);
        assert!(!is_exact_membership(&program, generated), "{replacement:?}");
    }
    *program.expression_table.expression_mut(generated) = ExpressionNode::Binary(comparison);
    let machine = &program.machines()[0];
    let collection = program
        .state_parameters(&program.machine_states(machine)[0])
        .iter()
        .find(|parameter| parameter.name.as_str() == "choices")
        .unwrap()
        .symbol;
    let ExpressionNode::Name(subject) = program.expression_table.expression_mut(comparison.left)
    else {
        panic!("subject");
    };
    subject.head_symbol = collection;
    subject.symbol = collection;
    assert!(!is_exact_membership(&program, generated));
}

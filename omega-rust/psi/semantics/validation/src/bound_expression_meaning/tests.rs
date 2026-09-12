use super::has_exact_case_membership_meaning;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

fn membership_program() -> (TypedTrees, ExpressionHandle, ExpressionHandle) {
    let tokens = Lexer::new(
        "data Choice [copy] { case Ready(value: u64); case Empty; }
         data Foreign [copy] { case Ready(value: u64); case Empty; }
         machine member(choice: Choice, choices: [Choice; 1]) -> bool {
             choice in Choice::Ready
         }
         machine foreign(choice: Foreign) -> bool { choice in Foreign::Ready }
         machine create() -> Choice { Choice::Ready { value: 37 } }",
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
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
fn membership_rejoins_the_nominal_subject_owner_and_case_symbols() {
    let (program, member, foreign) = membership_program();
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(member) else {
        panic!("membership comparison");
    };
    let ExpressionNode::Binary(foreign_comparison) = program.expression_table.expression(foreign)
    else {
        panic!("foreign membership comparison");
    };
    let ExpressionNode::Name(foreign_case) = program
        .expression_table
        .expression(foreign_comparison.right)
    else {
        panic!("foreign case reference");
    };
    let ExpressionNode::Name(original_case) = program.expression_table.expression(comparison.right)
    else {
        panic!("case reference");
    };
    for replacement in [
        typed_trees::expression::TableNamePath {
            symbol: foreign_case.symbol,
            ..*original_case
        },
        typed_trees::expression::TableNamePath {
            head_symbol: foreign_case.head_symbol,
            ..*original_case
        },
        *foreign_case,
    ] {
        let mut altered = program.clone();
        *altered.expression_table.expression_mut(comparison.right) =
            ExpressionNode::Name(replacement);
        assert!(!is_exact_membership(&altered, member));
        let mut diagnostics = Vec::new();
        crate::struct_literals::validate_struct_literal_fields(&altered, &mut diagnostics);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("case membership must test a value of the exact declaring data type")),
            "a mismatched membership cannot fall back to ordinary equality: {diagnostics:?}"
        );
    }

    // A consistent path still cannot put a foreign case under this owner.
    let mut altered = program.clone();
    let mut member_symbols = arena::HandleSpan::empty();
    for symbol in [original_case.head_symbol, foreign_case.symbol] {
        altered
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, symbol);
    }
    *altered.expression_table.expression_mut(comparison.right) =
        ExpressionNode::Name(typed_trees::expression::TableNamePath {
            symbol: foreign_case.symbol,
            member_symbols,
            ..*original_case
        });
    assert!(!is_exact_membership(&altered, member));

    // Keep a consistent foreign RHS and ledger, but the original Choice subject.
    let mut altered = program.clone();
    let foreign_selections: Vec<_> = program
        .expression_table
        .authored_selection_occurrences(foreign)
        .collect();
    let mut substituted = *foreign_comparison;
    substituted.left = comparison.left;
    let substituted = altered
        .expression_table
        .insert(ExpressionNode::Binary(substituted));
    altered
        .expression_table
        .attach_authored_selection_occurrences(substituted, foreign_selections);
    assert!(!is_exact_membership(&altered, substituted));

    // An array of the nominal owner is not itself a value of that owner.
    let machine = &program.machines()[0];
    let parameters = program.state_parameters(&program.machine_states(machine)[0]);
    let collection = parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == "choices")
        .expect("array parameter");
    let ExpressionNode::Name(subject) = altered.expression_table.expression_mut(comparison.left)
    else {
        panic!("membership subject");
    };
    subject.symbol = collection.symbol;
    subject.head_symbol = collection.symbol;
    assert!(!is_exact_membership(&altered, member));
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
fn membership_tag_domain_is_not_a_payload_subject_value() {
    let (mut program, member, _) = membership_program();
    let ExpressionNode::Binary(comparison) = program.expression_table.expression_mut(member) else {
        panic!("membership comparison");
    };
    comparison.left = comparison.right;
    assert!(!is_exact_membership(&program, member));
}

#[test]
fn inequality_is_not_a_lowered_membership_test() {
    let (mut program, member, _) = membership_program();
    let ExpressionNode::Binary(comparison) = program.expression_table.expression_mut(member) else {
        panic!("membership comparison");
    };
    comparison.operator = BinaryOperator::NotEqual;
    assert!(!is_exact_membership(&program, member));
}

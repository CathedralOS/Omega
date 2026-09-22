//! A collection measure is the compiler-owned metadata, not the spelling.
//!
//! The same program spells `len` twice: once as the compiler-owned length of
//! a fixed array, and once as a declared field of a record. Only the first is
//! a measure, and `semantic_calls::collection_measure_member` is the one place
//! which decides that for every consumer lane holding a receiver type.

use super::{AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionTarget};
use crate::semantic_calls::MeasureReceiver;
use crate::tests::front_end::{checked_program, typed_program};
use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

const BOTH_SPELLINGS: &str = r#"
    data Tally { len: u8; }

    machine main(tally: &Tally) -> u8 {
        let fixed: [u8; 2] = [1, 2];
        let count: u64 = fixed.len;
        tally.len
    }
"#;

/// Both `len` members, as (expression, receiver display name) pairs.
fn len_members(program: &typed_trees::TypedTrees) -> Vec<(ExpressionHandle, String)> {
    program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            (member.member.as_str() == CollectionMeasure::Length.authored_spelling()).then(|| {
                (
                    expression,
                    program.expression_table.display_name(member.receiver),
                )
            })
        })
        .collect()
}

#[test]
fn a_record_field_spelled_len_is_not_a_collection_measure() {
    let program = typed_program(BOTH_SPELLINGS);
    let members = len_members(&program);
    assert_eq!(members.len(), 2, "both `len` members are retained");

    for (expression, receiver) in members {
        let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
            panic!("retained member expression");
        };
        let receiver_type =
            crate::flow::expression_place_type_reference(&program, member.receiver, &[])
                .unwrap_or_else(|| panic!("`{receiver}` has a declared type"));
        let selected = crate::semantic_calls::collection_measure_member(
            &program,
            member,
            MeasureReceiver::Declared(receiver_type),
        );
        match receiver.as_str() {
            "fixed" => assert_eq!(
                selected,
                Some(CollectionMeasure::Length),
                "`fixed.len` is the compiler-owned length of a fixed array"
            ),
            "tally" => assert_eq!(
                selected, None,
                "`tally.len` is `Tally`'s declared field, not a measure"
            ),
            other => panic!("unexpected `len` receiver `{other}`"),
        }
    }
}

#[test]
fn checking_retains_the_measure_and_the_declared_field_separately() {
    let checked = checked_program(BOTH_SPELLINGS);
    let mut measure_selections = 0usize;
    let mut resolved_selections = 0usize;

    for (expression, node) in checked.expression_table.iter_expressions() {
        let ExpressionNode::Member(member) = node else {
            continue;
        };
        if member.member.as_str() != CollectionMeasure::Length.authored_spelling() {
            continue;
        }
        for occurrence in checked
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let Some(selection) = checked.authored_declaration_selections().get(occurrence) else {
                continue;
            };
            match selection.target() {
                AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                ) => measure_selections += 1,
                AuthoredDeclarationSelectionTarget::Resolved(_) => resolved_selections += 1,
                other => panic!("unexpected `len` selection target: {other:?}"),
            }
        }
    }

    assert_eq!(
        measure_selections, 1,
        "one compiler-owned measure is retained"
    );
    assert_eq!(
        resolved_selections, 1,
        "the declared field keeps its resolved declaration"
    );
}

//! Member meaning is selected once from the actual receiver type. A field
//! called `len` remains that field; an unknown receiver gains no builtin.

use super::boundary_calls::typed;
use typed_trees::expression::{CollectionMeasure, ExpressionNode, MemberSelection};

fn members(text: &str) -> Vec<MemberSelection> {
    typed(text)
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) => Some(member.selection),
            _ => None,
        })
        .collect()
}

#[test]
fn collection_length_is_selected_on_the_real_borrowed_receiver() {
    assert_eq!(
        members("machine observe(items: &[u8]) -> u64 { items.len }"),
        vec![MemberSelection::CollectionMeasure(
            CollectionMeasure::Length
        )]
    );
}

#[test]
fn a_same_spelled_record_field_keeps_its_declaration() {
    let program =
        typed("data Counter { len: u64 } machine observe(value: Counter) -> u64 { value.len }");
    let field = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_member_fields(program.data_members(data)))
        .find(|field| field.name.as_str() == "len")
        .unwrap()
        .symbol;
    assert!(field.is_valid());
    let selections: Vec<_> = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) => Some(member.selection),
            _ => None,
        })
        .collect();
    assert_eq!(selections, vec![MemberSelection::Field(field)]);
}

#[test]
fn a_noncollection_cannot_acquire_length_from_its_spelling() {
    assert_eq!(
        members("machine observe(value: u64) -> u64 { value.len }"),
        vec![MemberSelection::Unresolved]
    );
}

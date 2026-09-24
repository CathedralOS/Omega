//! A generic argument may itself be a generic application
//! (generics.md, "Applications and specialization"). Every application's
//! argument handles land in one shared table, so an enclosing application's
//! span must be inserted after its arguments are complete. Appending each
//! handle as it is parsed interleaves a nested application's arguments with
//! the enclosing list: the two authored spans then overlap and the enclosing
//! application reads argument handles that belong to its child.

use arena::HandleSpan;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{DataMember, Item};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// `Pair<u64[0..=3], Pair<u64[0..=7], u64[0..=15]>>`: the enclosing
/// application has two arguments and its second argument has two of its own.
const NESTED_APPLICATION: &str =
    "data Main { pair: Pair<u64[0..=3], Pair<u64[0..=7], u64[0..=15]>>; }";

fn parse_single_field_type(source: &str) -> (SyntaxTrees, TypeReferenceHandle) {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize nested application");
    let syntax = crate::parse_syntax_trees(&tokens).expect("parse nested application");
    let Item::Data(definition) = syntax.root_items().next().expect("Main declaration") else {
        panic!("data declaration");
    };
    let [DataMember::Field(field)] = syntax.items.data_members(definition.members) else {
        panic!("one field");
    };
    let type_reference = field.type_reference;
    (syntax, type_reference)
}

fn application_arguments(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    base: &str,
) -> HandleSpan<TypeReferenceHandle> {
    let TypeReferenceNode::Generic {
        base_name,
        arguments,
        ..
    } = syntax.type_references.type_reference(type_reference)
    else {
        panic!("generic application, not {type_reference:?}");
    };
    assert_eq!(base_name.as_str(), base);
    *arguments
}

/// The authored inclusive upper bound of a `u64[0..=n]` argument.
fn range_maximum(syntax: &SyntaxTrees, type_reference: TypeReferenceHandle) -> String {
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = syntax.type_references.type_reference(type_reference)
    else {
        panic!("range-shell argument, not {type_reference:?}");
    };
    assert!(matches!(
        syntax.type_references.type_reference(*base_type),
        TypeReferenceNode::Named(name) if name.as_str() == "u64"
    ));
    let [
        TypeConstraintNode::Range {
            maximum,
            end_inclusive: true,
            ..
        },
    ] = syntax.type_references.constraints(*constraints)
    else {
        panic!("one inclusive range constraint");
    };
    syntax.expressions.display_name(*maximum)
}

#[test]
fn a_nested_application_keeps_the_enclosing_applications_authored_arguments() {
    let (syntax, field_type) = parse_single_field_type(NESTED_APPLICATION);

    let outer = application_arguments(&syntax, field_type, "Pair");
    let [first, second] = syntax.type_references.type_reference_handles(outer) else {
        panic!("the enclosing application has two arguments");
    };
    assert_eq!(range_maximum(&syntax, *first), "3");

    let nested = application_arguments(&syntax, *second, "Pair");
    let [nested_first, nested_second] = syntax.type_references.type_reference_handles(nested)
    else {
        panic!("the nested application has two arguments");
    };
    assert_eq!(range_maximum(&syntax, *nested_first), "7");
    assert_eq!(range_maximum(&syntax, *nested_second), "15");
}

#[test]
fn a_nested_applications_argument_span_is_disjoint_from_the_enclosing_span() {
    let (syntax, field_type) = parse_single_field_type(NESTED_APPLICATION);

    let outer = application_arguments(&syntax, field_type, "Pair");
    let [_, second] = syntax.type_references.type_reference_handles(outer) else {
        panic!("the enclosing application has two arguments");
    };
    let nested = application_arguments(&syntax, *second, "Pair");

    let outer_start = outer.start().arena_index();
    let outer_end = outer_start + outer.count();
    let nested_start = nested.start().arena_index();
    let nested_end = nested_start + nested.count();
    assert!(
        outer_end <= nested_start || nested_end <= outer_start,
        "argument spans overlap: enclosing {outer_start}..{outer_end}, \
         nested {nested_start}..{nested_end}"
    );
}

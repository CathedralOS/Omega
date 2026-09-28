use super::super::lower_typed_trees;
use super::parse_typed_trees;
use crate::CheckingRequest;
use checked_trees::CheckedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

#[derive(Clone, Copy, Debug)]
enum Destination {
    Record,
    Variant,
    NestedRecord,
}

const SCALAR_DESTINATIONS: [Destination; 2] = [Destination::Record, Destination::Variant];
const EXACT_SEVEN: [&str; 3] = ["7 / 2 * 2", "7 / 2.0 * 2", "0.1 * 70"];

fn source(destination: Destination, target: &str, expression: &str) -> String {
    match destination {
        Destination::Record => format!(
            "data Number {{ value: {target}; }}
             machine construct() {{ let number: Number = Number {{ value: {expression} }}; }}"
        ),
        Destination::Variant => format!(
            "data Number {{ case Empty; case Present(value: {target}); }}
             machine construct() {{ let number: Number = Number::Present {{ value: {expression} }}; }}"
        ),
        Destination::NestedRecord => format!(
            "data Number {{ value: {target}; }}
             data Envelope {{ number: Number; }}
             machine construct() {{
                 let envelope: Envelope = Envelope {{ number: Number {{ value: {expression} }} }};
             }}"
        ),
    }
}

fn accepts(source: &str) -> CheckedTrees {
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    // Parsing and typing must succeed; an unrelated frontend failure is not a rejection witness.
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => panic!("invalid integer field landing accepted: {source}"),
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "{source}"),
    }
}

fn single_field_value(checked: &CheckedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    let ExpressionNode::StructLiteral(record) = checked.expression_table.expression(expression)
    else {
        panic!("the live initializer must retain its record or selected case literal");
    };
    let [field] = checked.expression_table.struct_fields(record.fields) else {
        panic!("one constructed field");
    };
    field.value
}

fn assert_retained_seven(checked: &CheckedTrees, destination: Destination, source: &str) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "construct")
        .expect("constructor machine");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    let [StatementNode::LocalData(local)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        panic!("one local constructor: {source}");
    };
    // Follow the live field, so an orphaned folded literal cannot satisfy the assertion.
    let mut field = single_field_value(checked, local.initial_value);
    if matches!(destination, Destination::NestedRecord) {
        field = single_field_value(checked, field);
    }
    // Source expressions stay intact; the destination consumer renders their
    // exact value. Replacing this node would lose fractional warning custody.
    let landed = validation::land_anonymous_integer_expression(
        checked,
        field,
        typed_trees::types::PrimitiveType::I32,
        |expression| validation::has_anonymous_operator_meaning(checked, expression),
    );
    assert_eq!(
        landed.and_then(|literal| literal.value_i64()),
        Some(7),
        "{source}"
    );
}

#[test]
fn typed_floats_cannot_land_implicitly_in_record_or_selected_variant_integer_fields() {
    for destination in SCALAR_DESTINATIONS {
        for expression in ["7.0f32", "7.0f64", "7.0f64 / 2 * 2", "0.1f64 * 70"] {
            rejects(&source(destination, "i32", expression));
        }
    }
}

#[test]
fn nested_record_scalar_fields_land_at_the_inner_field_destination() {
    for expression in EXACT_SEVEN {
        let source = source(Destination::NestedRecord, "i32 [7..=7]", expression);
        assert_retained_seven(&accepts(&source), Destination::NestedRecord, &source);
    }
    rejects(&source(
        Destination::NestedRecord,
        "i32 [6..=6]",
        "7 / 2 * 2",
    ));
    rejects(&source(Destination::NestedRecord, "i32", "7 / 2"));
}

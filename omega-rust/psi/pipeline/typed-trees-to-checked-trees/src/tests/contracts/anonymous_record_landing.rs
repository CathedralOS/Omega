use super::{lower_typed_trees, parse_typed_trees};
use checked_trees::{CheckedScalarExpression, CheckedTrees, CheckedUnitEffectOperationPlan};
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
const LARGE_SEVEN: [&str; 3] = [
    "(18446744073709551615 + 7) - 18446744073709551615",
    "(18446744073709551615 * 18446744073709551615) / 18446744073709551615 - 18446744073709551608",
    "(18446744073709551615 / 2.0 * 2) - 18446744073709551608",
];

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
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    // Parsing and typing must succeed; an unrelated frontend failure is not a rejection witness.
    match lower_typed_trees(parse_typed_trees(source)) {
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
fn record_scalar_fields_accept_exact_seven_and_retain_its_value() {
    for expression in EXACT_SEVEN {
        let source = source(Destination::Record, "i32 [7..=7]", expression);
        assert_retained_seven(&accepts(&source), Destination::Record, &source);
    }
}

#[test]
fn selected_variant_scalar_fields_accept_exact_seven_and_retain_its_value() {
    for expression in EXACT_SEVEN {
        let source = source(Destination::Variant, "i32 [7..=7]", expression);
        assert_retained_seven(&accepts(&source), Destination::Variant, &source);
    }
}

#[test]
fn record_scalar_fields_reject_the_truncated_singleton() {
    for expression in EXACT_SEVEN {
        rejects(&source(Destination::Record, "i32 [6..=6]", expression));
    }
}

#[test]
fn selected_variant_scalar_fields_reject_the_truncated_singleton() {
    for expression in EXACT_SEVEN {
        rejects(&source(Destination::Variant, "i32 [6..=6]", expression));
    }
}

#[test]
fn record_fields_cancel_intermediates_beyond_u64_at_the_destination() {
    for expression in LARGE_SEVEN {
        let source = source(Destination::Record, "u8 [7..=7]", expression);
        assert_retained_seven(&accepts(&source), Destination::Record, &source);
    }
}

#[test]
fn selected_variant_fields_cancel_intermediates_beyond_u64_at_the_destination() {
    for expression in LARGE_SEVEN {
        let source = source(Destination::Variant, "u8 [7..=7]", expression);
        assert_retained_seven(&accepts(&source), Destination::Variant, &source);
    }
}

#[test]
fn final_fractions_cannot_land_in_record_or_selected_variant_integer_fields() {
    for destination in SCALAR_DESTINATIONS {
        for expression in ["7 / 2", "7 / 2 / 2", "7 / 2.0", "0.1 * 71"] {
            rejects(&source(destination, "i32", expression));
        }
    }
}

#[test]
fn record_and_selected_variant_fields_reject_final_carrier_overflow() {
    for destination in SCALAR_DESTINATIONS {
        for (target, expression) in [
            ("u8", "255 + 1"),
            ("i8", "127 + 1"),
            ("i8", "0 - 129"),
            ("u8", "0 - 1"),
            ("u64", "18446744073709551615 + 1"),
        ] {
            rejects(&source(destination, target, expression));
        }
    }
}

#[test]
fn anonymous_zero_divisors_cannot_land_in_record_or_selected_variant_fields() {
    for destination in SCALAR_DESTINATIONS {
        for expression in ["1 / 0", "7 / (2 - 2)", "1 / 0.0", "(1 / 0) * 0"] {
            rejects(&source(destination, "i32", expression));
        }
    }
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
fn typed_integer_field_arithmetic_satisfies_six_but_not_seven() {
    for destination in SCALAR_DESTINATIONS {
        accepts(&source(destination, "i32 [6..=6]", "7i32 / 2 * 2"));
        rejects(&source(destination, "i32 [7..=7]", "7i32 / 2 * 2"));
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

#[test]
fn retained_scalar_record_field_plans_deliver_exact_seven_to_an_owned_call() {
    for expression in EXACT_SEVEN.into_iter().chain(LARGE_SEVEN) {
        let source = format!(
            "data Packet {{ value: i64; }}
             data Sink {{}}
             machine Sink::accept(packet: Packet) {{}}
             data Root {{}}
             machine Root::enter() {{
                 let packet: Packet = Packet {{ value: {expression} }};
                 Sink::accept(move packet);
             }}"
        );
        let checked = accepts(&source);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .expect("owned record caller");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .expect("the existing owned scalar-record call plan");
        let fields = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                    value, ..
                } => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            matches!(fields.as_slice(), [CheckedScalarExpression::IntegerLiteral { literal }]
                if literal.value_i64() == Some(7)),
            "the retained field-value plan must deliver exact seven: {source}: {fields:#?}"
        );
    }
}

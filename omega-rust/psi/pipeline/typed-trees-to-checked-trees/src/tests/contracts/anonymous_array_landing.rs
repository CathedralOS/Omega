use super::{lower_typed_trees, parse_typed_trees};
use checked_trees::CheckedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

#[derive(Clone, Copy, Debug)]
enum Destination {
    Local,
    RecordField,
    Assignment,
    Argument,
    Return,
    TransitionReturn,
    Window,
    MutableWindow,
}

const DESTINATIONS: [Destination; 8] = [
    Destination::Local,
    Destination::RecordField,
    Destination::Assignment,
    Destination::Argument,
    Destination::Return,
    Destination::TransitionReturn,
    Destination::Window,
    Destination::MutableWindow,
];
const EXACT_SEVEN: [&str; 3] = ["7 / 2 * 2", "7 / 2.0 * 2", "0.1 * 70"];
const LARGE_SEVEN: [&str; 3] = [
    "(18446744073709551615 + 7) - 18446744073709551615",
    "(18446744073709551615 * 18446744073709551615) / 18446744073709551615 - 18446744073709551608",
    "(18446744073709551615 / 2.0 * 2) - 18446744073709551608",
];

fn source(destination: Destination, nested: bool, expression: &str) -> String {
    let (array_type, initializer, zero) = if nested {
        ("[[i32; 1]; 1]", format!("[[{expression}]]"), "[[0]]")
    } else {
        ("[i32; 1]", format!("[{expression}]"), "[0]")
    };
    match destination {
        Destination::Local => {
            format!("machine construct() {{ let values: {array_type} = {initializer}; }}")
        }
        Destination::RecordField => format!(
            "data Packet {{ values: {array_type}; }}
             machine construct() {{ let packet: Packet = Packet {{ values: {initializer} }}; }}"
        ),
        Destination::Assignment => format!(
            "machine construct() {{
                 let mut values: {array_type} = {zero};
                 values = {initializer};
             }}"
        ),
        Destination::Argument => format!(
            "machine accept(values: {array_type}) {{}}
             machine construct() {{ accept({initializer}); }}"
        ),
        Destination::Return => {
            format!("machine construct() -> {array_type} {{ {initializer} }}")
        }
        Destination::TransitionReturn => format!(
            "machine construct() -> {array_type} {{ transition {{ _ -> ({initializer}) }} }}"
        ),
        Destination::Window => format!(
            "machine construct(values: &write {array_type}) {{ values[0..1] = {initializer}; }}"
        ),
        Destination::MutableWindow => format!(
            "machine construct(values: &mut {array_type}) {{ values[0..1] = {initializer}; }}"
        ),
    }
}

fn accepts(source: &str) -> CheckedTrees {
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str, expected_fragments: &[&str]) {
    // A syntax, resolution, or typing failure is not an arithmetic rejection witness.
    let typed = parse_typed_trees(source);
    let diagnostics = match lower_typed_trees(typed) {
        Ok(_) => panic!("invalid integer array element accepted: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| expected_fragments
            .iter()
            .all(|fragment| diagnostic.message.contains(fragment))),
        "expected arithmetic diagnostic containing {expected_fragments:?}: {source}: {diagnostics:#?}"
    );
}

fn single_element(checked: &CheckedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    let ExpressionNode::ArrayLiteral(elements) = checked.expression_table.expression(expression)
    else {
        panic!("the live initializer must retain its array literal");
    };
    let [element] = checked.expression_table.expression_handles(*elements) else {
        panic!("one array element");
    };
    *element
}

fn local_element(
    checked: &CheckedTrees,
    destination: Destination,
    nested: bool,
) -> ExpressionHandle {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "construct")
        .expect("array constructor machine");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    let [StatementNode::LocalData(local)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        panic!("one local array or record constructor");
    };
    let mut expression = local.initial_value;
    if matches!(destination, Destination::RecordField) {
        let ExpressionNode::StructLiteral(record) = checked.expression_table.expression(expression)
        else {
            panic!("explicit record constructor");
        };
        let [field] = checked.expression_table.struct_fields(record.fields) else {
            panic!("one array field");
        };
        expression = field.value;
    }
    expression = single_element(checked, expression);
    if nested {
        expression = single_element(checked, expression);
    }
    expression
}

fn assert_exact_seven(checked: &CheckedTrees, destination: Destination, nested: bool) {
    let element = local_element(checked, destination, nested);
    // Query the live source expression; landing must not require rewriting it to a literal.
    assert!(matches!(
        checked.expression_table.expression(element),
        ExpressionNode::Binary(_)
    ));
    let landed = validation::land_anonymous_integer_expression(
        checked,
        element,
        PrimitiveType::I32,
        |expression| validation::has_anonymous_operator_meaning(checked, expression),
    );
    assert_eq!(landed.and_then(|literal| literal.value_i64()), Some(7));
}

#[test]
fn local_and_record_arrays_accept_exact_seven_with_intact_element_expressions() {
    for destination in [Destination::Local, Destination::RecordField] {
        for nested in [false, true] {
            for expression in EXACT_SEVEN {
                let checked = accepts(&source(destination, nested, expression));
                assert_exact_seven(&checked, destination, nested);
            }
        }
    }
}

#[test]
fn assigned_and_argument_arrays_accept_exact_integral_elements() {
    for destination in [
        Destination::Assignment,
        Destination::Argument,
        Destination::Window,
    ] {
        for nested in [false, true] {
            for expression in EXACT_SEVEN {
                accepts(&source(destination, nested, expression));
            }
        }
    }
}

#[test]
fn direct_and_transition_array_returns_accept_exact_integral_elements() {
    for destination in [Destination::Return, Destination::TransitionReturn] {
        for nested in [false, true] {
            for expression in EXACT_SEVEN {
                accepts(&source(destination, nested, expression));
            }
        }
    }
}

#[test]
fn array_elements_cancel_intermediates_beyond_u64_before_integer_landing() {
    for destination in DESTINATIONS {
        for nested in [false, true] {
            for expression in LARGE_SEVEN {
                let checked = accepts(&source(destination, nested, expression));
                if matches!(destination, Destination::Local | Destination::RecordField) {
                    assert_exact_seven(&checked, destination, nested);
                }
            }
        }
    }
}

#[test]
fn final_fractional_array_elements_report_the_exact_noninteger_value() {
    for destination in DESTINATIONS {
        for nested in [false, true] {
            for (expression, exact_value) in
                [("7 / 2", "7/2"), ("7 / 2.0", "7/2"), ("0.1 * 71", "71/10")]
            {
                rejects(
                    &source(destination, nested, expression),
                    &["not an integer", exact_value],
                );
            }
        }
    }
}

#[test]
fn array_elements_reject_final_i32_overflow() {
    for destination in DESTINATIONS {
        for nested in [false, true] {
            for expression in ["2147483647 + 1", "0 - 2147483649"] {
                rejects(
                    &source(destination, nested, expression),
                    &["narrowing store", "i32", "not provably in range"],
                );
            }
        }
    }
}

#[test]
fn typed_float_array_elements_require_explicit_integer_conversion() {
    for destination in DESTINATIONS {
        for nested in [false, true] {
            for expression in ["7.0f32", "7.0f64", "7.0f64 / 2 * 2", "0.1f64 * 70"] {
                rejects(&source(destination, nested, expression), &["float", "i32"]);
            }
        }
    }
}

#[test]
fn typed_integer_array_elements_keep_their_typed_division() {
    for destination in DESTINATIONS {
        for nested in [false, true] {
            let checked = accepts(&source(destination, nested, "7i32 / 2 * 2"));
            if matches!(destination, Destination::Local | Destination::RecordField) {
                let element = local_element(&checked, destination, nested);
                assert!(matches!(
                    checked.expression_table.expression(element),
                    ExpressionNode::Binary(_)
                ));
                assert!(
                    validation::land_anonymous_integer_expression(
                        &checked,
                        element,
                        PrimitiveType::I32,
                        |expression| validation::has_anonymous_operator_meaning(
                            &checked, expression
                        ),
                    )
                    .is_none(),
                    "typed arithmetic must not become anonymous array arithmetic"
                );
            }
        }
    }
}

#[test]
fn typed_integer_six_can_be_stored_in_flat_and_nested_arrays() {
    for (array_type, initializer) in [
        ("[i32; 1]", "[typed_integer]"),
        ("[[i32; 1]; 1]", "[[typed_integer]]"),
    ] {
        accepts(&format!(
            "machine construct() -> i32 ensures result == 6 {{
                 let typed_integer: i32 [6..=6] = 7i32 / 2 * 2;
                 let values: {array_type} = {initializer};
                 typed_integer
             }}"
        ));
    }
}

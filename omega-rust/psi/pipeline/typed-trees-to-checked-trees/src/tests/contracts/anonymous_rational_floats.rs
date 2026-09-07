use super::{lower_typed_trees, parse_typed_trees};
use checked_trees::{CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees};
use numerics::literals::FloatFormat;
use semantic_vocabulary::IeeeFloatValue;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

#[derive(Clone, Copy, Debug)]
enum Destination {
    Return,
    Local,
    Argument,
    Array,
    Record,
}

const DESTINATIONS: [Destination; 5] = [
    Destination::Return,
    Destination::Local,
    Destination::Argument,
    Destination::Array,
    Destination::Record,
];

fn source(destination: Destination, target: &str, expression: &str) -> String {
    match destination {
        Destination::Return => format!("machine value() -> {target} {{ {expression} }}"),
        Destination::Local => {
            format!("machine value() {{ let landed: {target} = {expression}; }}")
        }
        Destination::Argument => format!(
            "machine accept(delivered: {target}) {{}}
             machine value() {{ accept({expression}); }}"
        ),
        Destination::Array => {
            format!("machine value() {{ let landed: [{target}; 1] = [{expression}]; }}")
        }
        Destination::Record => format!(
            "data Packet {{ delivered: {target}; }}
             machine value() {{ let landed: Packet = Packet {{ delivered: {expression} }}; }}"
        ),
    }
}

fn accepts(source: &str) -> CheckedTrees {
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => panic!("invalid float destination accepted: {source}"),
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "{source}"),
    }
}

fn delivered_expression(checked: &CheckedTrees, destination: Destination) -> ExpressionHandle {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .expect("the destination's machine");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    let [statement] = checked.statement_table.statements(state.statement_nodes) else {
        panic!("one destination statement: {destination:?}");
    };
    match (destination, statement) {
        (Destination::Return, StatementNode::Expression(expression)) => *expression,
        (Destination::Local, StatementNode::LocalData(local)) => local.initial_value,
        (Destination::Argument, StatementNode::Call(call)) => {
            let [argument] = checked.statement_table.expression_handles(call.arguments) else {
                panic!("one delivered argument");
            };
            *argument
        }
        (Destination::Array, StatementNode::LocalData(local)) => {
            let ExpressionNode::ArrayLiteral(elements) =
                checked.expression_table.expression(local.initial_value)
            else {
                panic!("retained array initializer");
            };
            let [element] = checked.expression_table.expression_handles(*elements) else {
                panic!("one array element");
            };
            *element
        }
        (Destination::Record, StatementNode::LocalData(local)) => {
            let ExpressionNode::StructLiteral(record) =
                checked.expression_table.expression(local.initial_value)
            else {
                panic!("retained record initializer");
            };
            let [field] = checked.expression_table.struct_fields(record.fields) else {
                panic!("one record field");
            };
            field.value
        }
        _ => panic!("unexpected destination statement: {destination:?}: {statement:#?}"),
    }
}

fn assert_delivered_bits(
    checked: &CheckedTrees,
    destination: Destination,
    expected: IeeeFloatValue,
) {
    // Follow the live destination, not orphaned literals left in the arena by folding.
    let expression = delivered_expression(checked, destination);
    let ExpressionNode::Float(literal) = checked.expression_table.expression(expression) else {
        panic!("the complete anonymous tree must land as one float: {destination:?}");
    };
    match expected {
        IeeeFloatValue::Binary32(bits) => {
            assert_eq!(literal.landing(), Some(FloatFormat::F32));
            assert_eq!(literal.f32_bits(), bits, "{destination:?}");
        }
        IeeeFloatValue::Binary64(bits) => {
            assert_eq!(literal.landing(), Some(FloatFormat::F64));
            assert_eq!(literal.landed_f64().to_bits(), bits, "{destination:?}");
        }
    }

    let role = match destination {
        Destination::Return => CheckedScalarExpressionRole::Return,
        Destination::Local => CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        Destination::Argument => CheckedScalarExpressionRole::UnitCallArgument {
            call_ordinal: 0,
            argument_ordinal: 0,
        },
        // Aggregate construction retains its leaves in the checked tree itself.
        Destination::Array | Destination::Record => return,
    };
    let plans = &checked.facts.values.scalar_expressions;
    let bindings = plans
        .source_bindings
        .iter()
        .map(|(_, binding)| binding)
        .filter(|binding| binding.role == role)
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "one retained scalar destination: {destination:?}"
    );
    let binding = bindings[0];
    assert_eq!(
        plans.expression_at(binding.state, binding.statement_ordinal, binding.role),
        Some(&CheckedScalarExpression::IeeeFloatLiteral { value: expected }),
        "the checked scalar plan must deliver the same landed bits: {destination:?}"
    );
}

#[test]
fn anonymous_integer_arithmetic_retains_float_bits_at_each_declared_destination() {
    for (expression, binary32, binary64) in [
        ("7 / 2 / 2", 0x3fe0_0000, 0x3ffc_0000_0000_0000),
        ("7 / 2 * 2", 0x40e0_0000, 0x401c_0000_0000_0000),
        ("(0 - 7) / 2 / 2", 0xbfe0_0000, 0xbffc_0000_0000_0000),
        ("7 / (0 - 2) * (0 - 2)", 0x40e0_0000, 0x401c_0000_0000_0000),
    ] {
        for (target, expected) in [
            ("f32", IeeeFloatValue::Binary32(binary32)),
            ("f64", IeeeFloatValue::Binary64(binary64)),
        ] {
            for destination in DESTINATIONS {
                let checked = accepts(&source(destination, target, expression));
                assert_delivered_bits(&checked, destination, expected);
            }
        }
    }
}

#[test]
fn f32_destinations_round_exact_integer_trees_once_without_an_f64_intermediate() {
    for (expression, bits) in [
        // Just above the midpoint 8388608.5: f64 first would erase the tiny addend.
        ("16777217 / 2 + 1 / 18014398509481984", 0x4b00_0001),
        // Just below 8388609.5: rounding that midpoint would instead give 8388610.
        (
            "(16777219 * 1000000000000000000000 - 1) / 2000000000000000000000",
            0x4b00_0001,
        ),
    ] {
        for destination in DESTINATIONS {
            let checked = accepts(&source(destination, "f32", expression));
            assert_delivered_bits(&checked, destination, IeeeFloatValue::Binary32(bits));
        }
    }
}

#[test]
fn anonymous_zero_divisors_cannot_land_at_float_destinations() {
    for target in ["f32", "f64"] {
        for expression in ["1 / 0", "7 / (2 - 2)", "(1 / 0) * 0"] {
            for destination in DESTINATIONS {
                rejects(&source(destination, target, expression));
            }
        }
    }
}

#[test]
fn typed_integer_operands_cannot_be_reinterpreted_as_anonymous_float_arithmetic() {
    for target in ["f32", "f64"] {
        for expression in ["7i32 / 2", "7i32 / 2 * 2", "7 / 2i32"] {
            for destination in DESTINATIONS {
                rejects(&source(destination, target, expression));
            }
        }
    }
    // Check the same typed operations positively at their integer boundary, with
    // paired guarantees so rejection above cannot conceal broken integer division.
    for (expression, expected, wrong) in [("7i32 / 2", 3, 4), ("7i32 / 2 * 2", 6, 7)] {
        accepts(&format!(
            "machine value() -> i32 ensures result == {expected} {{ {expression} }}"
        ));
        rejects(&format!(
            "machine value() -> i32 ensures result == {wrong} {{ {expression} }}"
        ));
    }
}

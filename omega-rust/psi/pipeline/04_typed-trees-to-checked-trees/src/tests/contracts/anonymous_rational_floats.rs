use super::super::lower_typed_trees;
use super::parse_typed_trees;
use crate::CheckingRequest;
use checked_trees::CheckedTrees;

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
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => panic!("invalid float destination accepted: {source}"),
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "{source}"),
    }
}

#[test]
fn anonymous_zero_divisors_cannot_land_at_float_destinations() {
    for target in ["f32", "f64"] {
        for expression in [
            "1 / 0",
            "7 / (2 - 2)",
            "(1 / 0) * 0",
            "1 / 0.0",
            "7.0 / (2 - 2.0)",
            "(1.0 / 0) * 0",
            "0.0 / 0.0",
            "1.0 / -0.0",
        ] {
            for destination in DESTINATIONS {
                rejects(&source(destination, target, expression));
            }
        }
    }
}

#[test]
fn typed_float_operands_keep_ieee_division_and_anonymous_peer_boundaries() {
    for target in ["f32", "f64"] {
        for expression in [format!("0.0{target} / 0.0"), format!("1.0{target} / 0")] {
            for destination in DESTINATIONS {
                accepts(&source(destination, target, &expression));
            }
        }
        for expression in [
            format!("1.0{target} + (1.0 / 0)"),
            "runtime + (1 / 0.0)".into(),
        ] {
            rejects(&format!(
                "machine value(runtime: {target}) -> {target} {{ {expression} }}"
            ));
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

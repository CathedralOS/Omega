//! Programs the specification makes wrong in every case, read from source.
//! Each is rejected with the rule it breaks, where a valid program of the
//! same form would only be "not supported yet"; each failing twin differs
//! from an accepted program in the one construct the rule is about. The
//! source is a subject without entries, so every machine of it is judged.

use super::*;

/// The rule statements of the rejected subject `text`, or what it was.
fn violations(text: &str) -> Result<Vec<String>, String> {
    match super::entryless::judged(text, false) {
        RouteVerdict::Rejected { violations, .. } => Ok(violations
            .iter()
            .map(|violation| violation.statement.clone())
            .collect()),
        RouteVerdict::Accepted(_) => Err("accepted".into()),
        RouteVerdict::Incomplete(_) => Err("incomplete".into()),
        RouteVerdict::FallBack(refusal) => Err(format!("not supported: {}", refusal.detail)),
    }
}

fn accepted(text: &str) {
    assert_eq!(violations(text).unwrap_err(), "accepted", "{text}");
}

fn rejected(text: &str, rule: &str) {
    let violations = violations(text).unwrap_or_else(|other| panic!("{other}: {text}"));
    assert!(
        violations.iter().any(|violation| violation.contains(rule)),
        "{violations:?}"
    );
}

/// state_contracts.md, "Statements after a call that never returns": "an
/// unknown machine name ... is an error wherever it appears". Nothing in the
/// program has the name `undeclared`; its twin calls a declared machine.
#[test]
fn a_call_of_a_name_nothing_declares_is_rejected() {
    let program = |called: &str| {
        format!("machine relay(value: u32) {{ }}\nmachine caller() {{ {called}(1); }}\n")
    };
    accepted(&program("relay"));
    rejected(
        &program("undeclared"),
        "an unknown machine name is an error",
    );
}

/// state_contracts.md: "an unknown machine name or malformed type is an
/// error wherever it appears". Nothing in the program is called `Nowhere`;
/// its twin names the declared domain `Km`.
#[test]
fn a_type_naming_a_domain_nothing_declares_is_rejected() {
    let program = |domain: &str| {
        format!(
            "domain i32::Km;\nmachine carry(value: i32 in {domain}) -> i32 in {domain} {{\n    \
             value\n}}\n"
        )
    };
    accepted(&program("Km"));
    rejected(
        &program("Nowhere"),
        "this type names the domain `Nowhere`, which nothing in the program declares",
    );
}

/// patterns.md, "Data and case decomposition": "Named patterns account for
/// every field once", and "Without `..`, a structural pattern mentions every
/// field".
#[test]
fn a_case_pattern_that_misses_or_repeats_a_payload_field_is_rejected() {
    let program = |pattern: &str| {
        format!(
            "data Shape {{\n    case Point(x: u32, y: u32);\n    case Empty;\n}}\n\
             machine measure(shape: Shape) -> u32 {{\n    match shape {{\n        \
             Shape::Point {{ {pattern} }} -> 1\n        Shape::Empty -> 0\n    }}\n}}\n"
        )
    };
    accepted(&program("x as _, y as _"));
    accepted(&program("x as _, .."));
    rejected(&program("x as _"), "mentions every payload field");
    rejected(
        &program("x as _, x as _, y as _"),
        "names each payload field once",
    );
}

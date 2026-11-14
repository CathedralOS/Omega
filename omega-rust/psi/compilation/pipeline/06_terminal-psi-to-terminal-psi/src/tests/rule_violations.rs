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

/// ownership.md, "Borrows and aliases": `&T` grants "Shared observation
/// under its loan contract", and "Owned local mutability": "A callee cannot
/// grant writable access that the caller's place lacks." Each failing twin
/// changes only the access its callee's parameter asks for. (A shared
/// reference formal handed on is the loan analysis's to reject: the formal
/// is its referent's place there, lent again.)
#[test]
fn an_argument_that_lends_only_observation_is_rejected_for_a_parameter_that_writes() {
    let rule = "the caller lends only immutable access";
    // A shared view handed on: a formal, and the result of a call.
    let view = |access: &str| {
        format!(
            "machine take(view: &{access}[u8]) {{ }}\n\
             machine same<'held>(view: &'held [u8]) -> &'held [u8] {{ view }}\n\
             machine pass(view: &[u8]) {{ take(view); }}\n\
             machine relay(view: &[u8]) {{ take(same(view)); }}\n"
        )
    };
    accepted(&view(""));
    rejected(&view("mut "), rule);
    // An explicit shared loan of a local.
    let loan = |access: &str| {
        format!(
            "machine take(count: &{access}u32) {{ }}\n\
             machine pass() {{ let mut count: u32 = 1; take(&count); }}\n"
        )
    };
    accepted(&loan(""));
    rejected(&loan("mut "), rule);
}

/// Policy suffixes need an unqualified integer carrier. These restrictions
/// are source errors, unlike a total operation whose formation proof the
/// compiler cannot yet express. Check the actual source-to-verdict path.
#[test]
fn invalid_contract_arithmetic_is_not_an_implementation_gap() {
    accepted(
        "machine sum(left: u32, right: u32) -> u32 \
         ensures result == left +% right { left +% right }",
    );
    accepted(
        "machine sum(left: u32, right: u32) -> u32 \
         ensures result == left +| right { left +| right }",
    );
    for (text, rule) in [
        (
            "machine sum(left: u32, right: u32) -> u32 \
             ensures result == left +! right { left +% right }",
            "can crash forms no proof term",
        ),
        (
            "machine sum() -> u32 ensures result == 7 +% 1 { 8 }",
            "policy operator on a wholly anonymous tree",
        ),
        (
            "machine sum(left: u32 in Wrapping, right: u32) \
             requires left +% right == right { }",
            "not an unqualified integer",
        ),
        (
            "domain u32::Tagged; machine sum(left: u32 in Tagged, right: u32) \
             requires left +% right == right { }",
            "not an unqualified integer",
        ),
        (
            "machine sum(left: bool, right: bool) requires left +% right == left { }",
            "not an unqualified integer",
        ),
        (
            "machine sum(left: u32, right: u64) requires left +% right == left { }",
            "policy operator operand types differ",
        ),
        (
            "machine sum() -> u32 { 7 +% 1 }",
            "policy operator on a wholly anonymous tree",
        ),
        (
            "machine sum(left: u32 in Wrapping, right: u32) -> u32 { left +% right }",
            "not an unqualified integer",
        ),
    ] {
        let result = super::entryless::judged(text, false);
        let RouteVerdict::Rejected { violations, .. } = result else {
            panic!("{text}: {result:?}");
        };
        assert!(
            violations.iter().any(|violation| {
                violation.statement.contains(rule) && violation.site.is_some()
            }),
            "{text}: {violations:?}"
        );
    }

    let valid_but_unsupported = "machine quotient(left: u32, right: u32) -> u32 \
        requires right > 0 ensures result == left /% right { left /% right }";
    let RouteVerdict::FallBack(refusal) = super::entryless::judged(valid_but_unsupported, false)
    else {
        panic!("valid quotient contract must retain its formation-support refusal");
    };
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(
        refusal.detail,
        "contract policy operator in a clause term that owes a condition before it is total"
    );
}

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
        "this type names the domain `Nowhere`, but no domain of that name is visible here",
    );
}

/// modules.md, "Resolution", and effects.md: "A row names a parameter path
/// or a requirement identity". Typing decides whether a written domain or
/// `invokes` name names nothing from what the use's own source can see, not
/// from any symbol anywhere that shares its last segment. Here a state of a
/// trait and another machine's parameter are spelled `Nowhere`; neither is
/// a domain or a requirement visible to the use, so both uses stay unknown
/// names. The twins make the spelling name something visible that is not
/// served there: a domain of another carrier, a machine in `invokes`.
#[test]
fn an_unrelated_declaration_sharing_a_spelling_does_not_make_a_name_known() {
    let masks = "boundary trait Registry {\n    machine Nowhere() reaches none crashes none;\n}\n\
                 machine holder(Nowhere: u32) { }\n";
    let domain = |declared: &str| {
        format!(
            "{masks}{declared}machine carry(value: i32 in Nowhere) -> i32 in Nowhere {{\n    \
             value\n}}\n"
        )
    };
    rejected(
        &domain(""),
        "this type names the domain `Nowhere`, but no domain of that name is visible here",
    );
    assert!(
        violations(&domain("domain u8::Nowhere;\n"))
            .unwrap_err()
            .starts_with("not supported"),
        "a domain of another carrier is a request, not an unknown name"
    );
    let invokes =
        |declared: &str| format!("{masks}{declared}machine entry() invokes Nowhere {{ }}\n");
    rejected(
        &invokes(""),
        "`invokes Nowhere` names no parameter and no requirement",
    );
    assert!(
        violations(&invokes("machine Nowhere() { }\n"))
            .unwrap_err()
            .starts_with("not supported"),
        "a visible machine in `invokes` is a request, not an unknown name"
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

    // The same divisor condition now forms the quotient in its guarantee.
    // Omitting it must still fail formation rather than invent a divisor.
    accepted(
        "machine quotient(left: u32, right: u32) -> u32 \
        requires right > 0 ensures result == left /% right { left /% right }",
    );
}

/// expressions.md: "Dot access remains data-field projection; it does not
/// select or invoke a machine", and state_contracts.md: "Syntax, name
/// resolution, types and declaration validity are checked as usual". A name
/// after the dot that is no field of the record's declaration names nothing,
/// read or stored through; its twin names a declared field.
#[test]
fn a_member_that_is_no_field_of_its_record_is_rejected() {
    let program = |statement: &str| {
        format!(
            "data Settings {{ quiet: bool; level: u32; }}\n\
             machine configure(settings: &mut Settings) -> u32 {{\n    {statement}\n}}\n"
        )
    };
    accepted(&program("settings.level"));
    accepted(&program("settings.quiet = true; settings.level"));
    let rule = "data `Settings` has no field `bogus`";
    rejected(&program("settings.bogus"), rule);
    rejected(&program("settings.bogus = 1; settings.level"), rule);
    rejected(
        &program("settings.quiet = settings.bogus; settings.level"),
        rule,
    );
}

/// data_and_literals.md, "Quoted bytes": "A quoted literal contains bytes,
/// has ordinary shared byte-view type `&[u8]`". Supplied to a place that
/// holds a scalar it is another type, wherever it is supplied; its twin
/// supplies a value of the scalar.
#[test]
fn a_quoted_literal_supplied_to_a_scalar_place_is_rejected() {
    let program = |stored: &str, passed: &str| {
        format!(
            "data Settings {{ quiet: bool; level: u32; }}\n\
             machine take(level: u32) {{ }}\n\
             machine configure(settings: &mut Settings) {{\n    \
             settings.quiet = {stored};\n    take({passed});\n}}\n"
        )
    };
    accepted(&program("true", "7"));
    rejected(
        &program("\"yes\"", "7"),
        "this quoted literal is a shared view of bytes, `&[u8]`, and the place it is supplied \
         to holds a `bool`",
    );
    rejected(
        &program("true", "\"seven\""),
        "this quoted literal is a shared view of bytes, `&[u8]`, and the place it is supplied \
         to holds a `u32`",
    );
}

/// data_and_literals.md: "Case values always use their carrier-qualified
/// member path". `Light` has the cases `On` and `Off`; `Light::Dim` names
/// nothing it declares.
#[test]
fn a_case_path_its_sum_does_not_declare_is_rejected() {
    let program = |case: &str| {
        format!(
            "data Light {{ case On; case Off; }}\nmachine pick() -> Light {{\n    \
             Light::{case}\n}}\n"
        )
    };
    accepted(&program("On"));
    rejected(&program("Dim"), "data `Light` has no member `Dim`");
}

/// data_and_literals.md: "A sum value is written as a case literal", and
/// "data does not own machine declarations. Collisions reject". The twin
/// writes the same case as its literal.
#[test]
fn a_case_called_like_a_machine_is_rejected() {
    let program = |value: &str| {
        format!(
            "data Level {{ case Set(value: u16); case Unset; }}\nmachine pick() -> Level {{\n    \
             {value}\n}}\n"
        )
    };
    accepted(&program("Level::Set { value: 7 }"));
    rejected(
        &program("Level::Set(7)"),
        "this call names a case of a sum, and a case is not a machine",
    );
}

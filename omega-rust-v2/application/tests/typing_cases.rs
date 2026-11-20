//! Positive and negative CLI twins for diagnostics owned by the L3 typer.
//!
//! Each test changes one contract-governed fact between a source that prepares and a
//! source that `source prepare` and `source lower` both refuse at the same typer site.
//! The list samples distinct rule families; it does not claim every L3 failure site.
#[path = "support/diagnostic_twins.rs"]
mod diagnostic_twins;
use diagnostic_twins::{Case, check};

#[test]
fn domain_membership_checks_the_declared_carrier() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("domain.omg", "domain u8::Ready; machine test(x:u8)->bool{x in u8::Ready}")],
        bad: &[("domain.omg", "domain u8::Ready; machine test(x:u16)->bool{x in u8::Ready}")],
        file: "domain.omg",
        occurrence: "u8::Ready",
        reason: "membership subject has the wrong carrier",
        rule: "Domains",
    });
}

#[test]
fn case_membership_checks_the_owning_sum() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("case.omg", "data Mode{case On;case Off;} machine test(x:Mode)->bool{x in Mode::On}")],
        bad: &[("case.omg", "data Mode{case On;case Off;} machine test(x:u16)->bool{x in Mode::On}")],
        file: "case.omg",
        occurrence: "Mode::On",
        reason: "membership subject has the wrong carrier",
        rule: "Domains",
    });
}

#[test]
fn an_assignment_to_a_computed_value_identifies_the_missing_storage() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("assign.omg", "machine value()->u8{1u8} machine set(){let mut x=value(); x=2u8;}")],
        bad: &[("assign.omg", "machine value()->u8{1u8} machine set(){value()=2u8;}")],
        file: "assign.omg",
        occurrence: "value()",
        reason: "computed value does not name writable storage",
        rule: "Patterns and places",
    });
}

#[test]
fn an_assignment_to_an_immutable_binding_identifies_the_missing_authority() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("assign.omg", "machine set(){let mut cell=1u8; cell=2u8;}")],
        bad: &[("assign.omg", "machine set(){let cell=1u8; cell=2u8;}")],
        file: "assign.omg",
        occurrence: "cell",
        reason: "place requires writable authority",
        rule: "Patterns and places",
    });
}

// Calls: a by-value parameter never implicitly dereferences an argument.
#[test]
fn by_value_parameter_requires_an_explicit_dereference() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("calls.omg", "machine take(n: u8) -> u8 { n }\nmachine f(v: @u8) -> u8 { take($v) }")],
        bad: &[("calls.omg", "machine take(n: u8) -> u8 { n }\nmachine f(v: @u8) -> u8 { take(v) }")],
        file: "calls.omg",
        occurrence: "take",
        reason: "no matching overload for take(@u8)",
        rule: "Calls",
    });
}

// Numbers and operators: an operator is selected from the typed operand carriers.
#[test]
fn mixed_width_operands_have_no_matching_operator() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("operators.omg", "machine add(a: u16, b: u16) -> u16 { a + b }")],
        bad: &[("operators.omg", "machine add(a: u8, b: u16) -> u16 { a + b }")],
        file: "operators.omg",
        occurrence: "a + b",
        reason: "no matching operator + for u8 and u16",
        rule: "Numbers and operators",
    });
}

// Numbers and operators: fractional history must be acknowledged at an integer landing.
#[test]
fn fractional_intermediate_requires_an_explicit_cast_at_landing() {
    let bad = "machine f() -> u8 { (5 / 2) * 2 }";
    let start = bad.find("5 / 2").unwrap();
    let end = start + "5 / 2".len();
    let reason = format!("fractional intermediate at bytes {start}..{end} yields 5; use an explicit cast to u8");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("landing.omg", "machine f() -> u8 { ((5 / 2) * 2) as u8 }")],
        bad: &[("landing.omg", bad)],
        file: "landing.omg",
        occurrence: "(5 / 2) * 2",
        reason: &reason,
        rule: "Numbers and operators",
    });
}

// Applications: integer widths must be positive.
#[test]
fn integer_width_must_be_positive() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("width.omg", "machine take(x: u<8>) {}")],
        bad: &[("width.omg", "machine take(x: u<0>) {}")],
        file: "width.omg",
        occurrence: "0",
        reason: "integer width must be positive",
        rule: "Applications",
    });
}

// Data and bindings: an unannotated binding cannot hold an unsuffixed number.
#[test]
fn unannotated_binding_of_an_unsuffixed_number_is_refused() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[(
            "binding.omg",
            "machine send(x: u8) {}\nmachine f() {\n    let count: u8 = 5;\n    send(count)\n}",
        )],
        bad: &[("binding.omg", "machine send(x: u8) {}\nmachine f() {\n    let count = 5;\n    send(count)\n}")],
        file: "binding.omg",
        occurrence: "5",
        reason: "an unsuffixed numeric binding must declare its type, as in `let count: u8 = 5` or `5u8`",
        rule: "Data and bindings",
    });
}

// Patterns and places: Boolean dispatch must cover both values.
#[test]
fn boolean_dispatch_must_cover_its_subject() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("dispatch.omg", "machine pick(b: bool) -> u8 { match b { true -> 1, false -> 0, } }")],
        bad: &[("dispatch.omg", "machine pick(b: bool) -> u8 { match b { true -> 1, } }")],
        file: "dispatch.omg",
        occurrence: "match b { true -> 1, }",
        reason: "dispatch does not cover its subject",
        rule: "Patterns and places",
    });
}

// Patterns and places: a returned reference must not expose invocation-local storage.
#[test]
fn returned_reference_to_local_storage_is_refused() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("views.omg", "machine f(x: @u8) -> @u8 { let v = x; move v }")],
        bad: &[("views.omg", "machine f(x: u8) -> @u8 { let v = @x; move v }")],
        file: "views.omg",
        occurrence: "move v",
        reason: "returned reference borrows invocation-local storage",
        rule: "Patterns and places",
    });
}

// Collection access: a by-copy indexed read requires a copy element.
#[test]
fn indexed_read_requires_a_copy_element() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("index.omg", "data Cell [copy] { v: u8; }\nmachine first(s: @[Cell]) -> Cell { s[0] }")],
        bad: &[("index.omg", "data Cell { v: u8; }\nmachine first(s: @[Cell]) -> Cell { s[0] }")],
        file: "index.omg",
        occurrence: "s[0]",
        reason: "indexed read requires a copy element",
        rule: "Collection access",
    });
}

// Cleanup: the selected machine consumes exactly one owned carrier.
#[test]
fn cleanup_must_consume_one_owned_carrier() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("cleanup.omg", "data Lease dropped by finish { slot: u8; }\nmachine finish(value: Lease) {}")],
        bad: &[("cleanup.omg", "data Lease dropped by finish { slot: u8; }\nmachine finish(value: @Lease) {}")],
        file: "cleanup.omg",
        occurrence: "dropped by finish",
        reason: "cleanup must consume exactly one owned carrier and return no value",
        rule: "Cleanup",
    });
}

// Domains: an explicit domain cast accounts for every domain of its operand.
#[test]
fn domain_cast_requires_complete_accounting() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[(
            "domains.omg",
            "domain u8::A;\ndomain u8::B;\nmachine f(x: u8 in A & B) -> u8 in A { x as =A -B }",
        )],
        bad: &[("domains.omg", "domain u8::A;\ndomain u8::B;\nmachine f(x: u8 in A & B) -> u8 in A { x as =A }")],
        file: "domains.omg",
        occurrence: "x as =A",
        reason: "cast omits domain u8::B",
        rule: "Domains",
    });
}

#[test]
fn a_known_builtin_domain_carrier_refuses_arguments_during_typing() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("domains.omg", "domain u8::Ready; machine f(x:u8 in u8::Ready){}")],
        bad: &[("domains.omg", "domain u8::Ready; machine f(x:u8 in u8<1>::Ready){}")],
        file: "domains.omg",
        occurrence: "<1>",
        reason: "builtin type u8 does not take arguments",
        rule: "Domains",
    });
}

#[test]
fn a_compact_domain_reference_uses_the_declared_integer_width() {
    let declarations = ("domains.omg", "domain u<8>::Ready;");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[declarations, ("use.omg", "machine test(x:u8 in u8::Ready){}")],
        bad: &[declarations, ("use.omg", "machine test(x:u16 in u16::Ready){}")],
        file: "use.omg",
        occurrence: "u16::Ready",
        reason: "no visible domain on the written carrier",
        rule: "Domains",
    });
}

#[test]
fn an_applied_integer_domain_qualifier_must_match_the_subject_width() {
    let declarations = ("domains.omg", "domain u8::Ready; domain u16::Ready;");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[declarations, ("use.omg", "machine f(x:u16 in u<16>::Ready){}")],
        bad: &[declarations, ("use.omg", "machine f(x:u16 in u<8>::Ready){}")],
        file: "use.omg",
        occurrence: "u<8>::Ready",
        reason: "domain qualifier names u8, found u16",
        rule: "Domains",
    });
}

#[test]
fn conformance_requirement_is_supplied_once() {
    let declarations = "trait Logger { machine write(x: u8); }\nmachine write_log(x: u8) {}\n";
    let good = format!("{declarations}Sink satisfies Logger {{ Logger::write = write_log; }}");
    let bad = format!("{declarations}Sink satisfies Logger {{ Logger::write = write_log; Logger::write = write_log; }}");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("rows.omg", &good)],
        bad: &[("rows.omg", &bad)],
        file: "rows.omg",
        occurrence: "Logger::write = write_log;",
        reason: "requirement is supplied twice",
        rule: "Evidence and effects",
    });
}

#[test]
fn an_alias_cycle_requires_a_dependency_on_the_selected_integer_width() {
    let good = "domain u8::Base; domain u16::Ready; domain u8::Ready = u<8>::Base; domain u16::Base = u<16>::Ready; machine f(x:u8 in Ready){}";
    let bad = good.replace("domain u8::Base;", "domain u8::Base = u<8>::Ready;");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("aliases.omg", good)],
        bad: &[("aliases.omg", &bad)],
        file: "aliases.omg",
        occurrence: "u8::Base",
        reason: "cyclic domain alias",
        rule: "Domains",
    });
}

// Reach ceilings: L2 resolves the service name; L3 requires that it name a boundary trait.
#[test]
fn a_reach_ceiling_requires_a_boundary_trait() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("reach.omg", "boundary trait Sensor {}\nmachine read() reaches Sensor {}")],
        bad: &[("reach.omg", "trait Sensor {}\nmachine read() reaches Sensor {}")],
        file: "reach.omg",
        occurrence: "Sensor",
        reason: "reach requires a boundary trait",
        rule: "L3 checks that each named service in a reach ceiling is a boundary trait.",
    });
}

// Matched sum cases: selecting a case on one holder field does not select it on a sibling.
#[test]
fn a_matched_case_does_not_select_the_sibling_holder_field() {
    let good = "data Token{v:u8;} data Choice{case A(left:Token);case B(left:Token);} data Holder{first:Choice;second:Choice;} machine pick(h:Holder)->Token{match h.first{Choice::A{..}->move h.first.left,Choice::B{..}->move h.first.left,}}";
    let bad = good.replacen("move h.first.left", "move h.second.left", 1);
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("held.omg", good)],
        bad: &[("held.omg", &bad)],
        file: "held.omg",
        occurrence: "h.second.left",
        reason: "Choice has no field left",
        rule: "A matched sum case applies to the exact subject.",
    });
}

// Access plans and progress: a resolved machine is not an admitted ranking view.
#[test]
fn an_explicit_ranking_view_must_be_admitted() {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("ranking.omg", "machine fake(x:u64)->u64{x}\nmachine f(x:u64) terminates by x {}")],
        bad: &[("ranking.omg", "machine fake(x:u64)->u64{x}\nmachine f(x:u64) terminates by x -> fake {}")],
        file: "ranking.omg",
        occurrence: "fake",
        reason: "ranking view is absent from the admitted theory",
        rule: "Access plans and progress",
    });
}

// Mathematical terms: ordinary arguments apply by prefix and cannot exceed the function type.
#[test]
fn a_mathematical_application_refuses_excess_arguments() {
    let definition = "let first(x:u8,y:bool):u8=x;\n";
    let good = format!("{definition}let term():u8=first(5u8,true);");
    let bad = format!("{definition}let term():u8=first(5u8,true,false);");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("math.omg", &good)],
        bad: &[("math.omg", &bad)],
        file: "math.omg",
        occurrence: "first(5u8,true,false)",
        reason: "excess mathematical arguments",
        rule: "Mathematical terms and assembly",
    });
}

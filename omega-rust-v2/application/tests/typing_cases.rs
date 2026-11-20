//! Positive and negative CLI twins for diagnostics owned by the L3 typer.
//!
//! Each test changes one contract-governed fact between a source that prepares and a
//! source that `source prepare` and `source lower` both refuse at the same typer site.
//! The list samples distinct rule families; it does not claim every L3 failure site.
#[path = "support/diagnostic_twins.rs"]
mod diagnostic_twins;
use diagnostic_twins::{Case, check};

mod direct_call_pause {
    use super::*;
    use omega_typer::DirectCallPauseError::{self, *};

    fn check_pause(effect: &str, good_caller: &str, bad_caller: &str, reason: &str, cause: DirectCallPauseError) {
        let source = |caller: &str| format!("machine work() {effect} {{}}\n{caller}");
        diagnostic_twins::check_with_reason(Case {
            layer: "L3",
            producer: "typer",
            good: &[("pause.omg", &source(good_caller))],
            bad: &[("pause.omg", &source(bad_caller))],
            file: "pause.omg",
            occurrence: "work()",
            reason,
            rule: "Evidence and effects",
        }, None, Some(cause.into()));
    }

    #[test]
    fn missing_block() {
        check_pause("blocks", "machine f(){block work()}", "machine f(){work()}",
            "call requires a block acknowledgement", MissingBlockAcknowledgement);
    }

    #[test]
    fn missing_suspend() {
        check_pause("suspends", "machine f(){suspend work()}", "machine f(){work()}",
            "call requires a suspend acknowledgement", MissingSuspendAcknowledgement);
    }

    #[test]
    fn blocking_ceiling() {
        check_pause("blocks", "public machine f() blocks {block work()}", "public machine f(){block work()}",
            "call exceeds the machine's blocking ceiling", ExceedsBlockingCeiling);
    }

    #[test]
    fn suspension_ceiling() {
        check_pause("suspends", "public machine f() suspends {suspend work()}", "public machine f(){suspend work()}",
            "call exceeds the machine's suspension ceiling", ExceedsSuspensionCeiling);
    }
}

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
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("dispatch.omg", "machine pick(b: bool) -> u8 { match b { true -> 1, false -> 0, } }")],
        bad: &[("dispatch.omg", "machine pick(b: bool) -> u8 { match b { true -> 1, } }")],
        file: "dispatch.omg",
        occurrence: "match b { true -> 1, }",
        reason: "dispatch does not cover its subject",
        rule: "Patterns and places",
    }, None, Some(omega_typer::DispatchError::UncoveredSubject.into()));
}

#[test]
fn tuple_dispatch_must_cover_each_declared_case_in_its_branch() {
    let schema = "data Mode { case On; case Off; }";
    let source = |dispatch: &str| format!("{schema}\nmachine pick(x: Mode, b: bool) -> bool {{ {dispatch} }}");
    let missing = "match (x, b) { (_, true) -> true, (Mode::On, false) -> false, }";
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("dispatch.omg", &source("match (x, b) { (_, true) -> true, (Mode::On, false) -> false, (Mode::Off, false) -> true, }"))],
        bad: &[("dispatch.omg", &source(missing))],
        file: "dispatch.omg",
        occurrence: missing,
        reason: "dispatch does not cover its subject",
        rule: "Patterns and places",
    }, None, Some(omega_typer::DispatchError::UncoveredSubject.into()));
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

// Rebuilding the argument reads only the copyable leaf. Moving the affine
// field instead would remove it from shared storage before the named call.
#[test]
fn a_named_call_cannot_move_an_affine_field_from_shared_storage() {
    let program = |arg: &str| format!("data B{{n:u8;}} data H{{item:B;}} machine take(b:B){{}} machine run(h:@H){{take({arg});}}");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("field.omg", &program("B{n:h.item.n}"))],
        bad: &[("field.omg", &program("move h.item"))],
        file: "field.omg",
        occurrence: "h.item",
        reason: "reading root\\B out of borrowed storage requires a copy type; borrow it with @ instead",
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
fn domain_declarations_collide_only_after_their_integer_carriers_match() {
    let good = "domain u<8>::Ready; domain u<16>::Ready; machine f(x:u16 in u16::Ready){}";
    let bad = "domain u<8>::Ready; domain u<4 + 4>::Ready; machine f(x:u16 in u16::Ready){}";
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("domains.omg", good)],
        bad: &[("domains.omg", bad)],
        file: "domains.omg",
        occurrence: "u<4 + 4>::Ready",
        reason: "duplicate domain u8::Ready",
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

fn check_source_change(good: &str, original: &str, replacement: &str, occurrence: &str, reason: &str, rule: &str) {
    assert_eq!(good.matches(original).count(), 1, "the change must identify one source occurrence");
    let bad = good.replacen(original, replacement, 1);
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("main.omg", good)],
        bad: &[("main.omg", &bad)],
        file: "main.omg",
        occurrence,
        reason,
        rule,
    });
}

#[test]
fn a_static_index_expression_cannot_read_a_runtime_parameter() {
    let source = "data B<const K:u8>{} machine buf<const N:u8>(n:u8)->B<N>{B::<N>{}}";
    check_source_change(source, "B::<N>{}", "B::<N + n>{}",
        "N + n", "const index needs a static argument", "Applications");
}

#[test]
fn a_closed_primitive_index_preserves_the_literal_error() {
    let source = "const BASE:u64=1; machine f(x:[u8;BASE + (1.0e0)]){}";
    check_source_change(source, "1.0e0", "1.0e9223372036854775808",
        "1.0e9223372036854775808", "decimal exponent exceeds implementation capacity", "Applications");
}

#[test]
fn an_open_primitive_index_identifies_the_failing_subexpression() {
    let source = "data B<const K:u8>{} machine f<const N:u8>(x:B<N + (4 / 2)>){}";
    check_source_change(source, "4 / 2", "4 / 0",
        "4 / 0", "anonymous division by zero", "Applications");
}

#[test]
fn a_primitive_index_identifies_the_operand_that_cannot_land() {
    let source = "data B<const K:u8>{} machine f<const N:u8>(x:B<(N + 2) * 1>){}";
    check_source_change(source, "N + 2", "N + 256",
        "256", "static operand 256 does not land in u8", "Applications");
}

#[test]
fn a_value_statement_requires_an_explicit_discard() {
    let source = "machine identity(x:u64)->u64 {x}";
    check_source_change(source, "{x}", "{x;}",
        "x", "value-producing statement requires an explicit discard", "Data and bindings");
}

#[test]
fn a_type_family_argument_matches_its_subject() {
    let source = r"let hold<A,F:A->core\Type<0>>(y:A,w:F(y),z:A):A=z;";
    check_source_change(source, "y:A", "y:u8",
        "y", "type-family subject has the wrong type", "Mathematical terms and assembly");
}

#[test]
fn a_machine_cannot_satisfy_an_ordinary_machine() {
    let source = "trait Step { machine run(x:u8)->u8; }
machine copy(x:u8)->u8{x}
machine selected(x:u8)->u8 satisfies Step::run {x}";
    check_source_change(source, "satisfies Step::run", "satisfies copy",
        "copy", "satisfaction must select a trait requirement", "Evidence and effects");
}

#[test]
fn a_carry_declaration_cannot_repeat_an_axis() {
    let source = "data Item [linear,carry(suspension:allowed,cpu:same,thread:any,address:movable,)] {}";
    check_source_change(source, "thread:any", "cpu:any",
        "carry(suspension:allowed,cpu:same,cpu:any,address:movable,)", "repeated carry axis", "Data and bindings");
}

#[test]
fn a_data_declaration_cannot_mix_fields_and_cases() {
    let source = "data Item { value: u64; }";
    check_source_change(source, "value: u64;", "value: u64; case Empty;",
        "data Item { value: u64; case Empty; }", "data cannot contain both fields and cases", "Data and bindings");
}

#[test]
fn a_decimal_exponent_must_be_representable() {
    let source = "machine f() -> u8 { 1.0e0 }";
    check_source_change(source, "1.0e0", "1.0e9223372036854775808",
        "1.0e9223372036854775808", "decimal exponent exceeds implementation capacity", "Numbers and operators");
}

#[test]
fn an_indirect_call_requires_a_callable_parameter() {
    let source = "data Ready=machine()->bool reaches none crashes none; machine f(cb:Ready)->bool{cb()}";
    check_source_change(source, "cb:Ready", "cb:bool",
        "cb", "indirect call requires a callable value", "Calls");
}

#[test]
fn a_reflection_query_rejects_its_own_type_arguments() {
    let source = r"use omega\language\core\reflect; data Buffer{used:u8;} machine inspect(){let x=reflect\field(Buffer::used);}";
    check_source_change(source, r"reflect\field(", r"reflect\field::<u8>(",
        r"reflect\field::<u8>", "a declaration query does not take type arguments", "Calls");
}

#[test]
fn a_case_pattern_belongs_to_the_subject_sum() {
    let source = "data A { case X; case Y; }
data B { case X; case Y; }
machine inspect(subject: @A) -> bool {
    match subject {
        A::X -> true,
        A::Y -> false,
    }
}";
    check_source_change(source, "A::X", "B::X",
        "B::X", "case does not belong to the subject", "Patterns and places");
}

#[test]
fn an_access_policy_supplies_the_registered_plan_requirement() {
    let source = r"use omega\language\core; use omega\language\core\reflect; use omega\language\core\access;
        Rule satisfies AccessPolicy { machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {new_plan::<'t>(@schema, Issuance::Unrestricted{})} }
        data LocalSchema{}
        data LocalPlan{}
        trait OtherPolicy{machine plan(s:LocalSchema)->LocalPlan;}
        OtherRule satisfies OtherPolicy{machine plan(s:LocalSchema)->LocalPlan terminates{LocalPlan{}}}
        data C{}
        domain C::Locked access Rule;";
    check_source_change(source, "access Rule", "access OtherRule",
        "OtherRule", "selected policy does not supply the admitted plan requirement", "Access plans and progress");
}

#[test]
fn a_call_cannot_repeat_its_block_acknowledgement() {
    let source = "machine may_wait() blocks {}
machine caller() { block may_wait(); }";
    check_source_change(source, "block may_wait", "block block may_wait",
        "block block may_wait()", "repeated or reversed call acknowledgement", "Evidence and effects");
}

#[test]
fn a_logical_hypothesis_cannot_suspend() {
    let source = "machine theorem<machine Step>(x:u8) requires machine Step(y:u8) requires y != 0 ensures y > 0; ensures Step::ensures(x) {} machine lemma(y:u8) requires y != 0 ensures y > 0 {} machine use_theorem(x:u8){theorem::<lemma>(x)}";
    check_source_change(source, "Step(y:u8) requires", "Step(y:u8) suspends requires",
        "suspends", "logical hypothesis cannot promise executable effects", "Mathematical terms and assembly");
}

#[test]
fn a_carry_declaration_needs_all_four_axes() {
    let source = "data Item [linear,carry(suspension:allowed,cpu:same,thread:any,address:movable,)] {}";
    check_source_change(source, "address:movable,", "",
        "carry(suspension:allowed,cpu:same,thread:any,)", "carry requires exactly four axes", "Data and bindings");
}

#[test]
fn selecting_a_trait_member_requires_trait_evidence() {
    let source = "trait Convert {machine keep(x:u8)->u8;} machine call<const E:Convert>(x:u8)->u8{E::keep(x)}";
    check_source_change(source, "E:Convert", "E:bool",
        "E::keep", "member selection requires a trait evidence binder", "Evidence and effects");
}

#[test]
fn a_nested_indexed_move_needs_an_owned_array() {
    let source = "data Resource{n:u8;} machine pick(a:[[Resource; 1]; 1])->Resource{move a[0][0]}";
    check_source_change(source, "a:[[", "a:@mut [[",
        "a[0]", "nested indexed move requires an owned fixed-array place", "Collection access");
}

#[test]
fn a_record_pattern_needs_a_record_subject() {
    let source = "data D{flag:bool;} machine choose<'a>(d:@D)->bool{match d {D{flag:true}->true,_->false,}}";
    check_source_change(source, "d:@D", "d:bool",
        "D{flag:true}", "pattern requires a record or case subject", "Patterns and places");
}

#[test]
fn a_binding_cannot_initialize_from_a_resultless_call() {
    let source = "machine one()->u8 { 1u8 }
machine noop() {}
machine demo() { let x = one(); }";
    check_source_change(source, "let x = one()", "let x = noop()",
        "noop()", "binding requires a value", "Data and bindings");
}

#[test]
fn an_inferred_constant_cannot_depend_on_itself() {
    check_source_change("const A = 1u8;", "1u8", "A", "A",
        "cannot infer a constant through a dependency cycle", "Data and bindings");
}

#[test]
fn a_match_of_unsuffixed_integers_needs_a_result_type() {
    let source = "machine choose(flag: bool, fallback: u8) -> u8 {
    let value = match flag {
        true -> 1,
        false -> fallback,
    };
    value
}";
    let refused_match = "match flag {
        true -> 1,
        false -> 2,
    }";
    check_source_change(source, "false -> fallback", "false -> 2", refused_match,
        "dispatch needs a result type", "Patterns and places");
}

#[test]
fn cleanup_must_select_one_exact_machine() {
    let source = "data Handle dropped by release { mark: u8; }
machine release(h: Handle) { let Handle { mark as _ } = h; }
";
    check_source_change(
        source,
        "dropped by release",
        "dropped by true",
        "true",
        "cleanup must select one exact machine",
        "Cleanup",
    );
}

fn check_crash(good: &str, bad: &str, occurrence: &str, reason: &str, cause: omega_typer::CrashDeclarationError) {
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("main.omg", good)],
        bad: &[("main.omg", bad)],
        file: "main.omg",
        occurrence,
        reason,
        rule: "Evidence and effects",
    }, None, Some(cause.into()));
}

#[test]
fn crashes_none_rejects_a_route_guard() {
    check_crash(
        "machine f() crashes none {}\n",
        "machine f() crashes none true {}\n",
        "crashes none true",
        "crashes none cannot carry a route guard",
        omega_typer::CrashDeclarationError::GuardOnNone,
    );
}

#[test]
fn process_exit_requires_the_exact_boundary_call() {
    let source = r"use omega\language\core;
machine quit<const E:core\ProcessExit>(code:i32) -> core\Never reaches core\ProcessExit {
    core\ProcessExit::exit_process(code)
}
";
    check_source_change(
        source,
        r"core\ProcessExit::exit_process",
        "E::exit_process",
        "E::exit_process(code)",
        "canonical process exit requires its exact boundary invocation",
        "Evidence and effects",
    );
}

#[test]
fn a_block_acknowledgement_requires_a_call() {
    let source = "machine wait() blocks {}
machine check() {
    block wait();
}
";
    check_source_change(
        source,
        "block wait()",
        "block wait",
        "block wait",
        "acknowledgement requires a call",
        "Evidence and effects",
    );
}

#[test]
fn a_fixed_array_extent_must_fit_u64() {
    let source = "machine check(a:[u8;3]) {}\n";
    let oversized = "18446744073709551616";
    check_source_change(
        source,
        "3",
        oversized,
        oversized,
        "array length exceeds u64",
        "Applications",
    );
}

#[test]
fn a_callback_call_must_match_its_declared_arity() {
    check_source_change(
        "data Step = machine(x:u8)->u8;\nmachine apply<const F:Step>(x:u8)->u8 { F(x) }\n",
        "F(x)",
        "F()",
        "F()",
        "incorrect callback arity",
        "Calls",
    );
}

#[test]
fn a_crash_clause_requires_a_builtin_cause() {
    check_crash(
        "data Other {}\nmachine check() crashes Trap {}\n",
        "data Other {}\nmachine check() crashes Other {}\n",
        "Other",
        "crash clause requires Trap, Abort or none",
        omega_typer::CrashDeclarationError::UnknownCause,
    );
}

#[test]
fn crashes_none_conflicts_with_a_declared_route() {
    let bad = "machine wrong() crashes none crashes Abort{}";
    check_crash(
        "machine wrong() crashes Abort{}", bad, bad,
        "crashes none conflicts with a declared cause",
        omega_typer::CrashDeclarationError::NoneWithDeclaredCause,
    );
}

#[test]
fn cleanup_cannot_publish_a_crash_route() {
    let source = "data Lease dropped by finish { slot: u8; }
machine finish(value: Lease) crashes none { let Lease { slot as _ } = value; }
";
    check_source_change(
        source,
        "crashes none",
        "crashes Trap",
        "dropped by finish",
        "cleanup cannot publish a crash route",
        "Cleanup",
    );
}

#[test]
fn cleanup_cannot_block_or_suspend() {
    let source = "data Lease dropped by finish { slot: u8; }
machine finish(value: Lease) { let Lease { slot as _ } = value; }
";
    check_source_change(
        source,
        "finish(value: Lease)",
        "finish(value: Lease) suspends",
        "dropped by finish",
        "cleanup cannot block or suspend",
        "Cleanup",
    );
}

#[test]
fn a_domain_head_index_must_name_a_distinct_binder() {
    check_source_change(
        "domain<const A:u8,const B:u8> u16::Tag<A,B>;\n",
        "<A,B>",
        "<A,A>",
        "A",
        "domain head index must name a distinct binder",
        "Domains",
    );
}

#[test]
fn an_empty_array_literal_needs_an_element_type() {
    check_source_change(
        "machine check() { let xs: [u8; 0] = []; }\n",
        ": [u8; 0]",
        "",
        "[]",
        "array literal needs an element type",
        "Data and bindings",
    );
}

// Positional record patterns cannot name more components than declared fields.
#[test]
fn a_positional_record_pattern_rejects_an_extra_component() {
    let source = "data Pair {
    left: u32;
    right: u32;
}

machine first(pair: Pair) -> u32 {
    let (left, _) = pair;
    left
}
";
    check_source_change(
        source,
        "(left, _)",
        "(left, _, extra)",
        "extra",
        "too many pattern components",
        "Patterns and places",
    );
}

#[test]
fn a_logical_hypothesis_cannot_supply_mutable_authority() {
    let source = "machine theorem<machine Step>()\nrequires machine Step(value: @u8);\n{}\n";
    check_source_change(
        source,
        "@u8",
        "@mut u8",
        "value: @mut u8",
        "logical hypothesis cannot supply mutable authority",
        "Mathematical terms and assembly",
    );
}

#[test]
fn a_type_parameter_rejects_a_lifetime_argument() {
    check_source_change(
        "data Box<T> { value: T; }\nmachine check<'a>(loan: @'a u8, boxed: Box<u8>) {}\n",
        "Box<u8>",
        "Box<'a>",
        "'a",
        "unexpected lifetime argument",
        "Applications",
    );
}

#[test]
fn a_record_pattern_requires_rest_to_omit_fields() {
    let source = "data Pair { x: u8; y: u8; }
machine pick(p: Pair) -> u8 {
    let Pair { x, .. } = p;
    x
}
";
    check_source_change(
        source,
        " ..",
        "",
        "Pair { x, }",
        "pattern omits fields without ..",
        "Patterns and places",
    );
}

#[test]
fn an_access_plan_computation_must_be_effect_free() {
    let source = r"use omega\language\core;
use omega\language\core\reflect;
use omega\language\core\access;
Rule satisfies AccessPolicy {
    machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
        new_plan::<'t>(@schema, Issuance::Unrestricted{})
    }
}
data C{}
domain C::Locked access Rule;
";
    check_source_change(
        source,
        "crashes none",
        "crashes Abort",
        "Rule",
        "access plan computation must be effect-free",
        "Access plans and progress",
    );
}

#[test]
fn a_shared_reborrow_requires_readable_storage() {
    check_source_change(
        "machine inspect(r: @mut u8) { _ = @$r; }\n",
        "@mut u8",
        "@write u8",
        "$r",
        "cannot read through a write-only reference",
        "Patterns and places",
    );
}

#[test]
fn a_sum_construction_requires_a_case() {
    check_source_change(
        "data Choice { case Pick(value: u32); }\nmachine check() { _ = Choice::Pick { value: 7 }; }\n",
        "::Pick",
        "",
        "Choice",
        "sum construction requires a case",
        "Data and bindings",
    );
}

#[test]
fn a_universe_requires_one_level() {
    check_source_change(
        "data Sort = core\\Type<0>;\n",
        "<0>",
        "",
        r"core\Type",
        "universe requires one level",
        "Applications",
    );
}

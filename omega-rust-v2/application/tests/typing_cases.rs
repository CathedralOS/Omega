//! Positive and negative CLI twins for diagnostics owned by the L3 typer.
//!
//! Each test changes one contract-governed fact between a source that prepares and a
//! source that `source prepare` and `source lower` both refuse at the same typer site.
//! The list samples distinct rule families; it does not claim every L3 failure site.
#[path = "support/diagnostic_twins.rs"]
mod diagnostic_twins;
use diagnostic_twins::{Case, check};

#[path = "typing_cases/invocation_ceiling.rs"]
mod invocation_ceiling;

#[path = "typing_cases/reviewed_call_inputs.rs"]
mod reviewed_call_inputs;
#[path = "typing_cases/reviewed_value_inputs.rs"]
mod reviewed_value_inputs;
#[path = "typing_cases/reviewed_static_inputs.rs"]
mod reviewed_static_inputs;

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
        good: &[("assign.omg", "machine value()->u8{1u8} machine set(){let mutable x=value(); x=2u8;}")],
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
        good: &[("assign.omg", "machine set(){let mutable cell=1u8; cell=2u8;}")],
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

mod operator_declaration {
    use super::*;
    use omega_typer::OperatorDeclarationError::{self, *};

    fn twin(good: &str, bad: &str, message: &str, cause: OperatorDeclarationError) {
        diagnostic_twins::check_with_reason(Case {
            layer: "L3", producer: "typer", rule: "Numbers and operators",
            good: &[("op.omg", good)], bad: &[("op.omg", bad)],
            file: "op.omg", occurrence: "+", reason: message,
        }, None, Some(cause.into()));
    }

    #[test]
    fn competing_declared_homes() {
        // Both imports really attach Ready to u8. One spelling defers the width check to L3.
        let east = ("east/defs.omg", "public domain u<8>::Ready;");
        let west = ("west/defs.omg", "public domain u8::Ready;");
        let head = "machine + u8::Ready::add(x:u8,y:u8)->u8{x}";
        check(Case {
            layer: "L3", producer: "typer", rule: "Numbers and operators",
            good: &[east, west, ("op.omg", &format!("use east; {head}"))],
            bad: &[east, west, ("op.omg", &format!("use east; use west; {head}"))],
            file: "op.omg", occurrence: "+",
            reason: r"competing declarations: root\east\u::Ready, root\west\u8::Ready (root\east\u::Ready on u, import op.omg@4..8) (root\west\u8::Ready on u8, import op.omg@14..18)",
        });
    }

    #[test]
    fn conformance_binds_token() {
        let good = "trait Add<T>{machine + add(x:T,y:T)->T;} Impl satisfies Add<u8>{machine add(x:u8,y:u8)->u8{x}}";
        twin(good, &good.replace("machine add", "machine + add"),
            "a conformance supplies the trait's token and cannot bind one", ConformanceBindsToken);
    }

    #[test]
    fn multiple_semantic_homes() {
        let good = "domain u8::P; domain u8::Q;\nmachine + add(x:u8 in P,y:u8 in P)->u8 in P{x}";
        twin(good, &good.replace("y:u8 in P", "y:u8 in Q"),
            "operator declaration has multiple possible semantic homes", MultipleSemanticHomes);
    }

    #[test]
    fn foreign_home() {
        let good = r"use omega\language\core; trait Local{} machine + Local::add(x:u8,y:u8)->u8{x}";
        twin(good, &good.replace("Local::add", r"core\AccessPolicy::add"),
            "only the declaring package may bind an operator for this home", ForeignHome);
    }

    #[test]
    fn foreign_nominal_operands() {
        let good = r"use omega\language\core; data Point{} data P=Point; machine + add(x:P,y:P)->P{x}";
        twin(good, &good.replace("=Point", r"=core\Exposure"),
            "only a nominal operand's declaring package may bind this operator", ForeignNominalOperands);
    }

    #[test]
    fn incorrect_arity() {
        let good = "data P{} machine + add(x:P,y:P)->P{x}";
        twin(good, &good.replace(",y:P", ""), "incorrect operator arity", IncorrectArity);
    }

    #[test]
    fn primitive_family() {
        let good = "domain u8::P; machine + add(x:u8 in P,y:u8)->u8{y}";
        twin(good, &good.replace(" in P", ""), "cannot bind a compiler-owned primitive operator family", PrimitiveFamily);
    }

    #[test]
    fn duplicate_binding() {
        let good = "data P{} data Q{} data Operand=Q;\nmachine + first(x:P,y:P)->P{x}\nmachine + second(x:P,y:Operand)->P{x}";
        twin(good, &good.replace("=Q", "=P"), "duplicate normalized operator binding", DuplicateBinding);
    }
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
    let good = "data Lease dropped by finish { slot: u8; }\nmachine finish(value: Lease) {}";
    check_cleanup_selection(good, &good.replace("value: Lease", "value: @Lease"), "dropped by finish",
        "cleanup must consume exactly one owned carrier and return no value", omega_typer::CleanupSelectionError::InvalidSignature);
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
fn a_child_cast_cannot_remove_its_required_parent() {
    let declarations = "domain u8::P; domain u8::P::Small requires true;";
    let good = format!("{declarations} machine small(p:u8 in P)->u8 in P::Small {{p as +P::Small}}");
    let bad = good.replace("p as +P::Small", "p as -P +P::Small");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("domains.omg", &good)],
        bad: &[("domains.omg", &bad)],
        file: "domains.omg",
        occurrence: "p as -P +P::Small",
        reason: r"cast removes domain root\u8::P required by root\u8::P::Small",
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
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("aliases.omg", good)],
        bad: &[("aliases.omg", &bad)],
        file: "aliases.omg",
        occurrence: "u8::Base",
        reason: "cyclic domain alias",
        rule: "Domains",
    }, None, Some(omega_typer::DefinitionCycleError::DomainAlias.into()));
}

#[test]
fn a_transparent_alias_cycle_reports_the_dependency_in_the_first_file() {
    let left = "data Left = Right;";
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("left.omg", left), ("right.omg", "data Right = u8;")],
        bad: &[("left.omg", left), ("right.omg", "data Right = Left;")],
        file: "left.omg",
        occurrence: "Right",
        reason: "cyclic type alias or binder kind",
        rule: "Applications",
    }, None, Some(omega_typer::DefinitionCycleError::TypeAliasOrBinderKind.into()));
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
    indirect_call_shape::runtime_value();
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

fn check_hypothesis_change(
    good: &str, original: &str, replacement: &str, occurrence: &str, reason: &str,
    cause: omega_typer::LogicalHypothesisError,
) {
    assert_eq!(good.matches(original).count(), 1);
    let bad = good.replacen(original, replacement, 1);
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("main.omg", good)],
        bad: &[("main.omg", &bad)],
        file: "main.omg",
        occurrence,
        reason,
        rule: "Mathematical terms and assembly",
    }, None, Some(cause.into()));
}

#[test]
fn a_logical_hypothesis_cannot_promise_an_observed_type_result() {
    let source = "machine theorem<machine Step>(x:u8)\nrequires machine Step(y:u8)->core\\Strict<0> requires y != 0;\nrequires x != 0 {}";
    check_hypothesis_change(source, "->core\\Strict<0>", "->u8", "->u8",
        "logical hypothesis cannot promise an observed Type result", omega_typer::LogicalHypothesisError::ObservedResult);
}

#[test]
fn a_nested_hypothesis_checks_its_result_at_the_outer_application() {
    let source = "data Output = core\\Squash<u8>; machine theorem<T,machine Step>() requires machine Step() requires machine Inner()->T; ; {} machine base(){} machine use_it(){theorem::<Output,base>()}";
    check_hypothesis_change(source, "core\\Squash<u8>", "u8", "theorem::<Output,base>",
        "logical hypothesis cannot promise an observed Type result", omega_typer::LogicalHypothesisError::ObservedResult);
}

#[test]
fn a_hypothesis_without_a_machine_slot_checks_its_specialized_authority() {
    // Inner has no machine slot: it arrives inside Step's evidence. A root hypothesis needs its binder at a citation.
    let source = "machine theorem<T,machine Step>() requires machine Step() requires machine Inner(x:T); ; {} machine base(){} machine use_it(){theorem::<@u8,base>()}";
    check_hypothesis_change(source, "@u8", "@mutable u8", "theorem::<@mutable u8,base>",
        "logical hypothesis cannot supply mutable authority", omega_typer::LogicalHypothesisError::MutableAuthority);
}

#[test]
fn a_logical_result_cannot_gain_a_runtime_carrier_at_specialization() {
    let source = "data Output = core\\Squash<u8>; machine theorem<T,machine Step>() requires machine Step(x:T)->T; {} machine keep(x:Output)->Output{x} machine use_it(){theorem::<Output,keep>()}";
    check_hypothesis_change(source, "core\\Squash<u8>", "u8", "theorem::<Output,keep>",
        "logical hypothesis cannot promise an observed Type result", omega_typer::LogicalHypothesisError::ObservedResult);
}

#[test]
fn a_logical_parameter_cannot_gain_write_authority_at_specialization() {
    let source = "data Input = @u8; machine theorem<T,machine Step>() requires machine Step(x:T); {} machine sink(x:Input){} machine use_it(){theorem::<Input,sink>()}";
    check_hypothesis_change(source, "@u8", "@write u8", "theorem::<Input,sink>",
        "logical hypothesis cannot supply mutable authority", omega_typer::LogicalHypothesisError::MutableAuthority);
}

#[test]
fn an_explicit_lifetime_cannot_hide_exclusive_hypothesis_authority() {
    let source = "machine theorem<'life,machine Step>() requires machine Step(value:@'life u8); {}";
    check_hypothesis_change(source, "@'life u8", "@'life mutable u8", "value:@'life mutable u8",
        "logical hypothesis cannot supply mutable authority", omega_typer::LogicalHypothesisError::MutableAuthority);
}

#[test]
fn a_logical_hypothesis_cannot_suspend() {
    let source = "machine theorem<machine Step>(x:u8) requires machine Step(y:u8) requires y != 0 ensures y > 0; ensures Step::ensures(x) {} machine lemma(y:u8) requires y != 0 ensures y > 0 {} machine use_theorem(x:u8){theorem::<lemma>(x)}";
    check_hypothesis_change(source, "Step(y:u8) requires", "Step(y:u8) suspends requires",
        "suspends", "logical hypothesis cannot promise executable effects", omega_typer::LogicalHypothesisError::ExecutableEffect);
}

#[test]
fn a_carry_declaration_needs_all_four_axes() {
    let source = "data Item [linear,carry(suspension:allowed,cpu:same,thread:any,address:movable,)] {}";
    check_source_change(source, "address:movable,", "",
        "carry(suspension:allowed,cpu:same,thread:any,)", "carry requires exactly four axes", "Data and bindings");
}

#[test]
fn a_callback_requirement_preserves_the_selected_trait_arguments() {
    let good = "trait Step<T>{machine run<U>(x:u8)->u8;}
        machine selected<T>(x:u8)->u8 satisfies Step<T>::run {x}
        machine accept<const F:Step<u8>::run>(){}
        machine f(){accept::<selected::<u8>>()}";
    let bad = good.replace("selected::<u8>", "selected::<bool>");
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("callback.omg", good)],
        bad: &[("callback.omg", &bad)],
        file: "callback.omg",
        occurrence: "selected::<bool>",
        reason: "selected evidence has the wrong trait application",
        rule: "Evidence and effects",
    }, None, Some(omega_typer::TraitEvidenceError::WrongTraitApplication.into()));
}

#[test]
fn explicit_evidence_must_satisfy_the_selected_trait_application() {
    let source = |argument| format!(
        "trait Step<T>{{machine run(x:T)->T;}}\n\
         ByteStep satisfies Step<u8>{{machine run(x:u8)->u8{{x}}}}\n\
         BoolStep satisfies Step<bool>{{machine run(x:bool)->bool{{x}}}}\n\
         machine selected<const E:Step<u8>>(x:u8)->u8{{E::run(x)}}\n\
         machine f(x:u8)->u8{{selected::<{argument}>(x)}}"
    );
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("evidence.omg", &source("ByteStep"))],
        bad: &[("evidence.omg", &source("BoolStep"))],
        file: "evidence.omg",
        occurrence: "BoolStep",
        reason: "selected evidence has the wrong trait application",
        rule: "Evidence and effects",
    }, None, Some(omega_typer::TraitEvidenceError::WrongTraitApplication.into()));
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
    check_source_change(source, "a:[[", "a:@mutable [[",
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
    check_cleanup_selection(source, &source.replace("by release", "by true"), "true",
        "cleanup must select one exact machine", omega_typer::CleanupSelectionError::NotExactMachine);
}

fn check_cleanup_selection(good: &str, bad: &str, occurrence: &str, reason: &str, cause: omega_typer::CleanupSelectionError) {
    diagnostic_twins::check_with_reason(Case {
        layer: "L3", producer: "typer", good: &[("cleanup.omg", good)], bad: &[("cleanup.omg", bad)],
        file: "cleanup.omg", occurrence, reason, rule: "Cleanup",
    }, None, Some(cause.into()));
}

#[test]
fn a_resolved_cleanup_declaration_needs_a_machine_signature() {
    // An unknown name would be refused by the resolver, not by signature lookup.
    let good = "const token = 1u8; machine finish(x:R){} data R dropped by finish {}";
    check_cleanup_selection(good, &good.replace("by finish", "by token"), "token",
        "cleanup must select one exact machine", omega_typer::CleanupSelectionError::NotExactMachine);
}

#[test]
fn cleanup_checks_the_normalized_carriers_package() {
    let good = r"use omega\language\core; data Local {} data Carrier=Local;
        domain Carrier::Marked dropped by finish; machine finish(x:Carrier){}";
    check_cleanup_selection(good, &good.replace("=Local", r"=core\Exposure"), "dropped by finish",
        "cleanup selection requires the carrier's owning package", omega_typer::CleanupSelectionError::ForeignCarrier);
}

#[test]
fn indexed_cleanup_accepts_the_carrier_and_index_and_rejects_a_missing_index() {
    let good = "domain<T,const N:u8> T::Level<N> dropped by finish<T>;
        machine finish<U>(x:U){} machine read(x:u8 in u8::Level<2>){}";
    let bad = good.replace("Level<2>", "Level");
    check(Case {
        layer: "L3", producer: "typer", rule: "Domains",
        good: &[("cleanup.omg", good)], bad: &[("cleanup.omg", &bad)],
        file: "cleanup.omg", occurrence: "u8::Level", reason: "incorrect indexed domain arity",
    });
}

#[test]
fn cleanup_rejects_an_unbound_machine_argument_before_substitution() {
    let good = "domain<T> u8::Mark<T> dropped by end<T>; machine end<U>(x:u8){} machine read(x:u8 in Mark<u8>){}";
    let bad = good.replace("Mark<T>", "Mark").replace("Mark<u8>", "Mark");
    check_cleanup_selection(good, &bad, "u8 in Mark", "incomplete cleanup domain application",
        omega_typer::CleanupSelectionError::IncompleteDomainApplication);
}

#[test]
fn indexed_cleanup_compares_the_substituted_machine_arguments() {
    let good = "data R dropped by end<R,u8> {} domain<U,T> T::Mark<U> dropped by end<T,U>;
        machine end<X,Y>(r:X){} machine read(x:R in R::Mark<u8>){}";
    check_cleanup_selection(good, &good.replace("Mark<u8>", "Mark<u16>"), "R in R::Mark<u16>",
        "conflicting cleanup selections on one owned subject", omega_typer::CleanupSelectionError::ConflictingSelections);
}

#[test]
fn indexed_cleanup_preserves_const_arguments_across_domain_binder_orders() {
    let good = "domain<T,const N:u8> T::A<N> dropped by end<T,N>;
        domain<const M:u8,U> U::B<M> dropped by end<U,M>;
        machine end<V,const K:u8>(x:V){} machine read(x:u8){let guarded=x as +A<1> +B<1>;}";
    let bad = good.replace("+B<1>", "+B<2>");
    check_cleanup_selection(
        good, &bad, "x as +A<1> +B<2>", "conflicting cleanup selections on one owned subject",
        omega_typer::CleanupSelectionError::ConflictingSelections,
    );
}

#[test]
fn qualified_cleanup_selections_must_agree_on_machine_arguments() {
    let good = "data R dropped by finish<u8> {} domain R::Live dropped by finish<u8>;
        machine finish<T>(x:R){} machine read(x:R in Live){}";
    check_cleanup_selection(good, &good.replace("Live dropped by finish<u8>", "Live dropped by finish<u16>"), "R in Live",
        "conflicting cleanup selections on one owned subject", omega_typer::CleanupSelectionError::ConflictingSelections);
}

#[test]
fn expression_cleanup_selections_must_agree_on_machine_identity() {
    let good = "domain u8::A dropped by finish; domain u8::B dropped by finish;
        machine finish(x:u8){} machine other(x:u8){} machine read(x:u8){let tagged=x as +A +B;}";
    check_cleanup_selection(good, &good.replace("B dropped by finish", "B dropped by other"), "x as +A +B",
        "conflicting cleanup selections on one owned subject", omega_typer::CleanupSelectionError::ConflictingSelections);
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

mod callback_effects {
    use super::*;
    use omega_typer::CallbackEffectError::{self, *};

    const SUSPENDING: &str = "data Step=machine()->u8 reaches none crashes none suspends;\nmachine apply(f:Step)->u8 suspends {suspend f()}";

    fn check_change(
        good: &str, before: &str, after: &str, call: &str, reason: &str, cause: CallbackEffectError,
    ) {
        assert_eq!(good.matches(before).count(), 1);
        let bad = good.replacen(before, after, 1);
        diagnostic_twins::check_with_reason(Case {
            layer: "L3", producer: "typer", good: &[("callback.omg", good)], bad: &[("callback.omg", &bad)],
            file: "callback.omg", occurrence: call, reason, rule: "Evidence and effects",
        }, None, Some(cause.into()));
    }

    #[test]
    fn open_reach_under_a_finite_ceiling() {
        let good = "data Step=machine() reaches none crashes none;\nmachine apply<const F:Step>() reaches none {F()}";
        check_change(good, "reaches none crashes", "crashes", "F()",
            "callback with open service reach cannot fit the machine's reach ceiling", OpenReachUnderCeiling);
    }

    #[test]
    fn bounded_reach_exceeds_the_caller() {
        let good = "boundary trait Clock {}\ndata Step=machine() reaches Clock crashes none;\nmachine apply<const F:Step>() reaches Clock {F()}";
        check_change(good, "reaches Clock {", "reaches none {", "F()",
            "callback exceeds the machine's service reach", ExceedsServiceReach);
    }

    #[test]
    fn block_does_not_acknowledge_suspension() {
        check_change(SUSPENDING, "suspend f()", "block f()", "f()",
            "callback requires its declared pause acknowledgement", MissingPauseAcknowledgement);
    }

    #[test]
    fn acknowledgement_preserves_the_caller_ceiling() {
        check_change(SUSPENDING, "suspends {", "suspends false {", "f()",
            "callback exceeds the machine's pause ceiling", ExceedsPauseCeiling);
    }
}

#[path = "support/indirect_call_shape.rs"]
mod indirect_call_shape;

#[test]
fn a_callback_call_must_match_its_declared_arity() {
    indirect_call_shape::local_short();
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

fn check_cleanup_effect(good: &str, bad: &str, reason: &str, cause: omega_typer::CleanupEffectError) {
    let source = |effect: &str| format!("data Lease dropped by finish {{ slot: u8; }}\nmachine finish(value: Lease){effect} {{ let Lease {{ slot as _ }} = value; }}\n");
    diagnostic_twins::check_with_reason(Case {
        layer: "L3",
        producer: "typer",
        good: &[("main.omg", &source(good))],
        bad: &[("main.omg", &source(bad))],
        file: "main.omg",
        occurrence: "dropped by finish",
        reason,
        rule: "Cleanup",
    }, None, Some(cause.into()));
}

#[test]
fn cleanup_cannot_publish_a_crash_route() {
    check_cleanup_effect(" crashes none", " crashes Trap", "cleanup cannot publish a crash route", omega_typer::CleanupEffectError::PublishesCrashRoute);
}

#[test]
fn cleanup_cannot_publish_an_abort_route() {
    check_cleanup_effect(" crashes none", " crashes Abort", "cleanup cannot publish a crash route", omega_typer::CleanupEffectError::PublishesCrashRoute);
}

#[test]
fn cleanup_cannot_block_or_suspend() {
    check_cleanup_effect("", " suspends", "cleanup cannot block or suspend", omega_typer::CleanupEffectError::MayPause);
}

#[test]
fn cleanup_cannot_block() {
    check_cleanup_effect("", " blocks", "cleanup cannot block or suspend", omega_typer::CleanupEffectError::MayPause);
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
    check_hypothesis_change(
        source,
        "@u8",
        "@mutable u8",
        "value: @mutable u8",
        "logical hypothesis cannot supply mutable authority",
        omega_typer::LogicalHypothesisError::MutableAuthority,
    );
}

#[test]
fn a_stored_view_names_its_own_lifetime() {
    check_source_change(
        "data Bytes<'b> { view: @'b [u8]; }\ndata Schema<'s> { field: @'s Bytes<'s>; }\n",
        "@'s Bytes<'s>",
        "@Bytes<'s>",
        "@Bytes<'s>",
        "a stored reference must name its lifetime",
        "Data and bindings",
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
        "machine inspect(r: @mutable u8) { _ = @$r; }\n",
        "@mutable u8",
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

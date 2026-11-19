mod support;
#[path = "support/evaluation.rs"]
mod evaluation;
use omega_typer::ValueType;
use support::check;

// Keep each test at the crate root so existing --test calls -- --exact selectors still work.
include!("calls/assembly.rs");
include!("calls/borrowed_field_reads.rs");
include!("calls/conformance.rs");
include!("calls/contracts_and_effects.rs");
include!("calls/data_and_places.rs");
include!("calls/generic_applications.rs");
include!("calls/mathematical_evidence.rs");
include!("calls/numbers_and_operators.rs");
include!("calls/operator_signatures.rs");
include!("calls/package_authority.rs");
include!("calls/result_domain_selection.rs");
include!("calls/returned_views.rs");

#[test]
fn selects_by_operand_and_checks_the_selected_result() {
    let declarations = ("draw.omg", include_str!("../fixtures/draw.omg"));
    let caller = ("render.omg", include_str!("../fixtures/render.omg"));
    let stage = check(&[caller, declarations]).unwrap();
    assert_eq!(stage, check(&[declarations, caller]).unwrap());
    assert_eq!(stage.calls.len(), 1);
    assert_eq!(stage.calls[0].machine.operands, ["bool"]);
    assert_eq!(stage.calls[0].result, ValueType::Bool);
    let wrong = include_str!("../fixtures/wrong_result.omg");
    let err = check(&[declarations, ("wrong.omg", wrong)]).unwrap_err();
    assert_eq!(err.span.slice(wrong.as_bytes()).unwrap(), b"draw(flag)");
    assert!(err.message.contains("expected u32, found bool"), "{err:?}");
}

#[test]
fn follows_nominal_field_types_through_nested_calls_and_shadowing() {
    let code = "data Flag { value: bool; } machine take(x: bool) -> bool { x } machine f(x: Flag) -> bool { let x = x.value; take(take(x)) }";
    let stage = check(&[("main.omg", code)]).unwrap();
    assert_eq!(stage.calls.len(), 2);
    assert!(stage.calls.iter().all(|c| c.result == ValueType::Bool));
    let err = check(&[("main.omg", "data A {} data B {} machine take(x: A) {} machine f(x: B) { take(x) }")]).unwrap_err();
    assert!(err.message.contains("no matching overload"));
}

#[test]
fn refuses_wrong_arity_ambiguous_calls_and_unsupported_forms() {
    for (code, message) in [
        ("machine take(x: bool) {} machine f() { take() }", "no matching overload"),
        ("machine take(x: u8) {} machine take(x: u16) {} machine f() { take(1) }", "ambiguous"),
        ("machine f() -> u32 { true }", "expected u32"),
        ("machine f() -> u32 {}", "missing result"),
        ("machine f(x: u32) -> u32 { x + true }", "no matching operator"),
        ("machine f<T>(x: T) -> bool { x }", "expected bool"),
    ] {
        let err = check(&[("a.omg", code)]).unwrap_err();
        assert!(err.message.contains(message), "{err:?}");
    }
}

#[test]
fn range_is_checked_after_call_selection() {
    let err = check(&[("a.omg", "machine f(n: u8) {} machine g() { f(256) }")]).unwrap_err();
    assert!(err.message.contains("expected u8, found 256"));
    let err = check(&[("a.omg", "machine f(x: u8) {} machine f(x: u16) {} machine g() { f(256) }")]).unwrap_err();
    assert!(err.message.contains("ambiguous"));
    let stage = check(&[("a.omg", "machine f(x: u8) {} machine f(x: u16) {} machine g() { f(7u16) }")]).unwrap();
    assert_eq!(stage.calls[0].machine.operands, ["u16"]);
}

#[test]
fn identical_normalized_operand_types_reject_without_a_call() {
    let err = check(&[("a.omg", "machine f(x: u8) {} machine f(y: u08) {}")]).unwrap_err();
    assert!(err.message.contains("duplicate typed operand signature"));
}

#[test]
fn permuting_files_preserves_the_whole_typed_stage() {
    let a = ("alpha.omg", "data Byte=u8; trait R<T>{machine read(x:T)->T;}");
    let b = (
        "beta.omg",
        "Impl satisfies R<Byte>{machine read(x:Byte)->Byte{x}} machine get<T,const E:R<T>>(x:T)->T{E::read(x)}",
    );
    let c = ("gamma.omg", "machine f()->u8{get::<Byte,Impl>(1)}");
    let expected = check(&[a, b, c]).unwrap();
    for input in [[a, c, b], [b, a, c], [b, c, a], [c, b, a], [c, a, b]] {
        assert_eq!(check(&input).unwrap(), expected);
    }
}

#[test]
fn imported_operand_types_distinguish_overloads_shadowed_by_value_binders() {
    let stage = check(&[
        ("red/point.omg", "public data Point {}"),
        ("blue/point.omg", "public data Point {}"),
        ("a.omg", "use red::Point; machine visit<const Point:u8>(p:Point) {}"),
        ("b.omg", "use blue::Point; machine visit<const Point:u8>(p:Point) {}"),
        (
            "caller.omg",
            "machine f(red:red::Point,blue:blue::Point) { visit::<1>(red); visit::<2>(blue); }",
        ),
    ])
    .unwrap();
    let owners: Vec<_> = stage
        .calls
        .iter()
        .map(|call| call.machine.signature_owners[0].1.as_str())
        .collect();
    assert_eq!(owners, ["red::Point", "blue::Point"]);
}

#[test]
fn an_explicit_type_argument_can_name_a_domain_on_its_written_carrier() {
    let source = "domain<T> T::Flag; machine take<const A:core::Type<0>>(){} machine test(){take::<u8::Flag>();}";
    let stage = check(&[("main.omg", source)]).unwrap();
    let call = stage.calls.iter().find(|c| c.machine.path == "take").unwrap();
    assert_eq!(call.type_arguments.len(), 1);
    assert_eq!(call.type_arguments[0].carrier().to_string(), "u8");
    assert!(matches!(call.type_arguments[0].domains(),[domain] if domain.declaration.path=="T::Flag"));
    for (bound, ambiguous) in [("u8", false), ("core::Type<0>", true)] {
        let definitions = format!("data T {{}} data Bound={bound}; domain<const T:Bound> T::Flag; domain u8::Flag;");
        let program = source.replace("domain<T> T::Flag;", &definitions);
        let result = check(&[("main.omg", &program)]);
        if ambiguous {
            let error = result.unwrap_err();
            assert!(error.message.contains("competing declarations"), "{error:?}");
        } else {
            let stage = result.unwrap();
            let argument = &stage.calls[0].type_arguments[0];
            assert!(matches!(argument.domains(),[domain] if domain.declaration.path=="u8::Flag"));
        }
    }
}

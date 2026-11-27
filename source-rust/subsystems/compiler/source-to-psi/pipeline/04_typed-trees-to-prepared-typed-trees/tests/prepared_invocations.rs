//! The source driver runs each compile-time call with the operands, static
//! arguments and constants its own source names, and stage 04 hands
//! lowering the value that call returned. These tests check that the value
//! follows exactly the input that should decide it: an array index, a
//! callback's static argument, the static argument of an enclosing machine,
//! a negated operand, or the operand of a call written as an array extent.
//!
//! The spec rules are in `wiki/spec/language/evaluation.md`: "Specialize the
//! selected machine's complete contract at its concrete arguments", and
//! "Admission is invocation-sensitive". `wiki/spec/language/constants.md`,
//! "Evaluation", says a constant position requests that evaluation.
//!
//! Each program has a twin that differs only in the deciding input and
//! gives another number, so a driver that ignored the input, or read a
//! neighbouring one, freezes the wrong value.

#[path = "support/prepared_package.rs"]
mod package;

use build_model::values::BuildTimeValue;
use package::{Demand, integer, prepare};

fn frozen(program: &str, constant: &str) -> Option<i64> {
    let prepared = prepare(&[("main.omg", program)], Demand::Package)
        .unwrap_or_else(|refusal| panic!("{program}: {refusal:?}"));
    integer(prepared.value(constant))
}

const PICK: &str = "data Packet [copy] { byte: u8; } \
    const ITEMS: [Packet; 2] = [Packet { byte: 4u8 }, Packet { byte: 9u8 }]; \
    machine pick(items: [Packet; 2], i: u64) -> u8 requires i < 2 { items[i].byte }";

/// `pick(ITEMS, i)` reads the element its index operand selects from the
/// frozen array, and the requirement `i < 2` is judged with that operand:
/// 0 and 1 freeze the first and second element's field, and 2 is refused at
/// the call.
#[test]
fn a_fixed_array_read_freezes_the_element_its_index_selects() {
    for (index, expected) in [(0, 4), (1, 9)] {
        let program = format!("{PICK} const N: u8 = pick(ITEMS, {index}u64);");
        let prepared = prepare(&[("main.omg", &program)], Demand::Package).unwrap();
        assert_eq!(
            integer(prepared.value("N")),
            Some(expected),
            "index {index}"
        );
        assert_eq!(prepared.inputs_for("N").constants, ["ITEMS"]);
        assert_eq!(
            prepared.evaluations(),
            [("N".to_owned(), BuildTimeValue::Int(expected))]
        );
    }
    let outside = format!("{PICK} const N: u8 = pick(ITEMS, 2u64);");
    let refusal = prepare(&[("main.omg", &outside)], Demand::Package)
        .err()
        .expect("index 2 is outside the array");
    let [(message, at)] = refusal.as_slice() else {
        panic!("one refusal: {refusal:?}")
    };
    assert_eq!(at, "pick(ITEMS, 2u64)");
    assert!(
        message.contains("cannot prove requires contract for call pick")
            && message.contains("2 < 2"),
        "{message}"
    );
}

const APPLY: &str = "machine level<const N: u8>(v: u8) -> u8 { N } \
    data ByteStep = machine(v: u8) -> u8; \
    machine apply<const F: ByteStep>(v: u8) -> u8 { F(v) }";

/// `apply::<level::<N>>(7u8)` calls the callback with the static argument
/// written inside its own application, not with the runtime operand `7u8`,
/// and two applications in one program keep their own arguments. The same
/// holds where the call is a type's const argument.
#[test]
fn an_applied_callback_runs_with_its_own_static_argument() {
    let program = format!(
        "{APPLY} const TWO: u8 = apply::<level::<2>>(7u8); \
         const FIVE: u8 = apply::<level::<5>>(7u8);"
    );
    let prepared = prepare(&[("main.omg", &program)], Demand::Package).unwrap();
    assert_eq!(
        prepared.evaluations(),
        [
            ("FIVE".to_owned(), BuildTimeValue::Int(5)),
            ("TWO".to_owned(), BuildTimeValue::Int(2)),
        ]
    );
    for level in [2, 5] {
        let argument = format!(
            "{APPLY} data Buffer<const N: u8> {{}} \
             machine take(x: Buffer<apply::<level::<{level}>>(7u8)>) {{}}"
        );
        let prepared = prepare(&[("main.omg", &argument)], Demand::Package).unwrap();
        assert_eq!(
            prepared.evaluations(),
            [("take".to_owned(), BuildTimeValue::Int(level))],
            "Buffer<apply::<level::<{level}>>(7u8)>"
        );
    }
}

/// `helper` is declared inside `outer` and reads `outer`'s `N`. Each
/// application of `outer` gives its own copy of `helper` its own `N`, so
/// `outer::<3>(2)` is 5 and `outer::<9>(2)` is 11.
#[test]
fn a_nested_helper_reads_the_static_argument_of_its_own_enclosing_application() {
    let program = "machine outer<const N: u64>(input: u64) -> u64 { \
            helper(input) \
            machine helper(n: u64) -> u64 { N +% n } \
        } \
        const SMALL: u64 = outer::<3>(2u64); \
        const LARGE: u64 = outer::<9>(2u64);";
    let prepared = prepare(&[("main.omg", program)], Demand::Package).unwrap();
    assert_eq!(integer(prepared.value("SMALL")), Some(5));
    assert_eq!(integer(prepared.value("LARGE")), Some(11));
    assert_eq!(
        prepared.evaluations(),
        [
            ("LARGE".to_owned(), BuildTimeValue::Int(11)),
            ("SMALL".to_owned(), BuildTimeValue::Int(5)),
        ]
    );
}

const NEGATE: &str = "machine negate(x: i8) -> i8 requires x > -128 { -x }";

/// A negative literal, a negative literal with its carrier suffix and a
/// call that negates its operand all freeze as -5. The call is judged with
/// its operand: negating `MIN` breaks `negate`'s requirement and is refused.
#[test]
fn a_negated_literal_and_a_call_that_negates_its_operand_freeze_the_same_value() {
    let program = format!(
        "{NEGATE} const EXACT: i8 = -5; const SUFFIXED: i8 = -5i8; \
         const PARAMETER: i8 = negate(5);"
    );
    let prepared = prepare(&[("main.omg", &program)], Demand::Package).unwrap();
    for constant in ["EXACT", "SUFFIXED", "PARAMETER"] {
        assert_eq!(integer(prepared.value(constant)), Some(-5), "{constant}");
    }
    assert_eq!(
        prepared.evaluations(),
        [("PARAMETER".to_owned(), BuildTimeValue::Int(-5))]
    );
    assert_eq!(
        frozen(&format!("{NEGATE} const VALUE: i8 = negate(4);"), "VALUE"),
        Some(-4)
    );
    let minimum = format!("{NEGATE} const MIN: i8 = -128; const VALUE: i8 = negate(MIN);");
    let refusal = prepare(&[("main.omg", &minimum)], Demand::Package)
        .err()
        .expect("-(-128) does not fit an i8");
    let [(message, at)] = refusal.as_slice() else {
        panic!("one refusal: {refusal:?}")
    };
    assert_eq!(at, "negate(MIN)");
    assert!(message.contains("MIN > -128"), "{message}");
}

/// An array extent written as a call, `[u8; width(7u8)]`, is evaluated with
/// the operand written there. Beside it, `[u8; width(8u8)]` in the same
/// machine gets its own value, and a constant that makes the first call gets
/// the first value.
#[test]
fn an_array_extent_written_as_a_call_is_evaluated_with_its_own_operand() {
    let program = "machine width(n: u8) -> u64 crashes none { n as u64 +% 1u64 } \
        const SIZE: u64 = width(7u8); \
        machine take(a: [u8; width(7u8)], b: [u8; width(8u8)], c: [u8; SIZE]) {}";
    let prepared = prepare(&[("main.omg", program)], Demand::Package).unwrap();
    assert_eq!(integer(prepared.value("SIZE")), Some(8));
    let mut evaluations = prepared.evaluations();
    evaluations.sort_by_key(|(requester, value)| (requester.clone(), format!("{value:?}")));
    assert_eq!(
        evaluations,
        [
            ("SIZE".to_owned(), BuildTimeValue::Int(8)),
            ("take".to_owned(), BuildTimeValue::Int(8)),
            ("take".to_owned(), BuildTimeValue::Int(9)),
        ]
    );
    let arguments: Vec<_> = prepared
        .inputs()
        .into_iter()
        .map(|row| row.value)
        .filter(|value| value.starts_with("width("))
        .collect();
    assert_eq!(arguments, ["width(7u8)", "width(8u8)"]);
}

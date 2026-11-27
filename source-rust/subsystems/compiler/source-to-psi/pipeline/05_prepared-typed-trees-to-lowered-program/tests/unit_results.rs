//! A machine that declares no result type completes with Unit. When one of
//! its transition arms supplies a value, or a caller forwards its Unit
//! completion as a value result, stage 05 must reject the program and name
//! the machine whose header lacks the result type. It used to refuse both
//! with "not supported yet: typing arm value type", blaming the caller and
//! hiding the missing `-> T`. Each negative has a positive twin that differs
//! only in that declared result type.
//!
//! The spec rule: a terminal call's "result compatibility" is the ordinary
//! call-and-completion rule (`wiki/spec/language/state_contracts.md`,
//! "Terminal calls in transition arms"), and "an ordinary terminal value or
//! permitted implicit Unit completion ends the invocation and owes its
//! declared result/return contract" (`state_contracts.md`).

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::Program;

/// A writer whose nested `encode` takes eleven parameters and forwards a
/// sibling's `u8` from its arms. `result` is `encode`'s declared result.
fn writer(result: &str) -> String {
    format!(
        "public data Writer [copy] {{ status: u8; at: u64; }}
public data Outcome [copy] {{ status: u8; written: u64; }}
machine produce(out: @mutable [u8], start: u64, flag: bool) -> Outcome {{
    Outcome {{ status: 0u8, written: start }}
}}
public machine add(writer: @mutable Writer, out: @mutable [u8], pixels: @[u8], code: u8, \
            duration: u32, lossless: bool, quality: u32, scratch: @mutable [u8]) -> u8 {{
    let state: Writer = writer;
    let at: u64 = state.at & 17179869183u64;
    let start: u64 = at + 12u64;
    transition state.status == 0u8 && start <= out.length {{
        true -> encode(writer, out, pixels, code, duration, lossless, quality, scratch, state, \
            at, start),
        false -> (match state.status == 0u8 {{ true -> 7u8, false -> state.status }}),
    }}
    machine encode(writer: @mutable Writer, out: @mutable [u8], pixels: @[u8], code: u8, \
            duration: u32, lossless: bool, quality: u32, scratch: @mutable [u8], state: Writer, \
            at: u64, start: u64){result}
    requires start <= out.length {{
        let outcome: Outcome = produce(out, start, lossless);
        transition outcome.status == 0u8 {{
            true -> done(writer, outcome.written),
            false -> failed(writer, outcome.status),
        }}
    }}
    machine done(writer: @mutable Writer, at: u64) -> u8 {{ writer.at = at; 0u8 }}
    machine failed(writer: @mutable Writer, status: u8) -> u8 {{ writer.status = status; status }}
}}"
    )
}

/// Two siblings that tail-call each other through their arms. `result` is
/// `step`'s declared result; `walk` declares `-> u64`.
fn siblings(result: &str) -> String {
    format!(
        "machine read_symbol(bits: u64, code: u64, length: u32, limit: u32, first: u64, \
            count: u64, index: u64, offset: u64, table: u64) -> u64 {{
    transition {{ _ -> walk(bits, code, length, limit, first, count, index, offset, table) }}
    machine walk(bits: u64, code: u64, length: u32, limit: u32, first: u64, count: u64, \
            index: u64, offset: u64, table: u64) -> u64 {{
        transition length < limit {{
            true -> step(bits, code, length, limit, first, count, index, offset, table, 1u64),
            false -> table,
        }}
    }}
    machine step(bits: u64, code: u64, length: u32, limit: u32, first: u64, count: u64, \
            index: u64, offset: u64, table: u64, bit: u64){result}
    requires length < limit {{
        transition {{ _ -> walk(bits, code, length + 1u32, limit, first, count, index, offset, \
            bit) }}
    }}
}}"
    )
}

fn rejection(source: &str) -> String {
    let refusal = Program::try_of(&[("product/main.omg", source)])
        .err()
        .expect("a value cannot complete a Unit machine");
    assert_eq!(refusal.stage, "lowering", "{refusal:?}");
    refusal.message
}

#[test]
fn a_nested_machine_that_declares_its_result_completes_its_callers_arm() {
    Program::of(&writer(" -> u8")).judged(|_| {}).accepted();
}

#[test]
fn a_nested_machine_without_a_result_type_is_named_where_its_arm_supplies_a_value() {
    let message = rejection(&writer(""));
    // The caller is told that the callee completes with Unit, and which
    // header to change, rather than that its own arm is unsupported.
    assert!(
        message.starts_with(
            "Violation: machine `add` declares the result type `u8`, but this value is the \
             result of `add::encode`, which declares no result type and so completes with \
             Unit: declare the result in `add::encode`'s header"
        ),
        "{message}"
    );
    assert!(!message.contains("typing arm value type"), "{message}");
}

#[test]
fn sibling_machines_that_declare_their_results_call_each_other_from_arms() {
    Program::of(&siblings(" -> u64")).judged(|_| {}).accepted();
}

#[test]
fn a_sibling_without_a_result_type_is_named_in_the_cycle() {
    let message = rejection(&siblings(""));
    assert!(
        message.starts_with(
            "Violation: machine `read_symbol::walk` declares the result type `u64`, but this \
             value is the result of `read_symbol::step`, which declares no result type and so \
             completes with Unit: declare the result in `read_symbol::step`'s header"
        ),
        "{message}"
    );
    assert!(!message.contains("typing arm value type"), "{message}");
}

/// The callee's own arm is rejected too: a value cannot complete a machine
/// that declares no result.
#[test]
fn a_value_arm_in_a_machine_without_a_result_type_names_that_machine() {
    let message = rejection(
        "machine encode(flag: bool) {
    transition flag { true -> 1u8, false -> 3u8 }
}",
    );
    assert!(
        message.starts_with(
            "Violation: machine `encode` declares no result type, so it completes with Unit, \
             but this value has type `u8`: declare the result in its header (`-> u8`)"
        ),
        "{message}"
    );
}

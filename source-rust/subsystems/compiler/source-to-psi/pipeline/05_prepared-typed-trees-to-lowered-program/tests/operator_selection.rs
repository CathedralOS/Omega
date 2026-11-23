//! Stage 05 must lower each operator to the operation the operand types
//! select: a declared operator by its token or by its name calls the same
//! machine with the operands in the order written, an arithmetic-policy
//! operand on either side selects that policy's operation, and a policy cast
//! keeps the carrier or widens it first. The tests write short machines and
//! read what stage 05 produced; twins move the policy to the other operand or
//! change the policy.
//!
//! The spec rules: an operator applies when its parameter types equal the
//! operands' qualified types (`wiki/spec/language/domains.md`, "Domain
//! composition and operators"); `Wrapping`, `Saturating` and `Trapping`
//! select which suffixed operation `+` means for their operands, and a
//! comparison reads no policy (`wiki/spec/language/numeric_values.md`,
//! "Policy domains"); a domain cast keeps its carrier and a carrier cast is
//! written before the domain it adds (`domains.md`, "Domain casts").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Program, returned, scalar};
use terminal_psi::{OperationKind, TerminalMachine, Terminator};

fn formals(machine: &TerminalMachine) -> Vec<semantic_vocabulary::ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

/// `b + a` selects `pick`, the `+` declared over `Km`, and `pick(b, a)`
/// names it: both lower to one call of `pick` with `b` first, whose result
/// is returned. The `+` declared over `Metres` is not called.
#[test]
fn a_token_and_a_named_call_select_the_same_operator_with_the_operands_written() {
    let program = Program::of(
        "domain u8::Km; domain u8::Metres;
machine + other(left: u8 in Metres, right: u8 in Metres) -> u8 in Metres { left }
machine + pick(left: u8 in Km, right: u8 in Km) -> u8 in Km { right }
machine by_token(a: u8 in Km, b: u8 in Km) -> u8 in Km { b + a }
machine by_name(a: u8 in Km, b: u8 in Km) -> u8 in Km { pick(b, a) }",
    );
    let lowered = program.lower();
    let pick = lowered.machine_id("pick");
    for caller in ["by_token", "by_name"] {
        let machine = lowered.machine(caller);
        let [a, b] = formals(machine)[..] else {
            panic!("`{caller}` has two formals")
        };
        let [(_, call)] = lowered.operations(caller)[..] else {
            panic!("`{caller}` is one call: {:?}", lowered.operations(caller))
        };
        let OperationKind::Call {
            callee, arguments, ..
        } = &call.kind
        else {
            panic!("{call:?}")
        };
        assert_eq!(
            (*callee, arguments.as_slice()),
            (pick, &[b, a][..]),
            "{caller}"
        );
        assert_eq!(returned(machine), scalar(call), "{caller}");
    }
    program.judged(|_| {}).accepted();
}

/// The operation each machine lowers to, as its debug spelling, with its
/// operands.
fn only_operation(program: &Program, path: &str) -> String {
    let lowered = program.lower();
    let [(_, operation)] = lowered.operations(path)[..] else {
        panic!("`{path}` is one operation: {:?}", lowered.operations(path))
    };
    format!("{:?}", operation.kind)
}

/// Under `Wrapping`, `Saturating` and `Trapping`, `x + y`, `x - y` and
/// `x * y` lower to that policy's operation over `x` then `y`, whether the
/// policy qualifies the left operand, the right one or both. A trapping
/// operation owes its own trap; the others owe nothing. A comparison with a
/// policy operand is the plain comparison.
#[test]
fn a_policy_operand_on_either_side_selects_that_policys_operation() {
    let tokens = [("+", "Add"), ("-", "Subtract"), ("*", "Multiply")];
    for policy in ["Wrapping", "Saturating", "Trapping"] {
        for carrier in ["u8", "i13"] {
            let qualified = format!("{carrier} in {policy}");
            let ceiling = if policy == "Trapping" { "Trap" } else { "none" };
            let mut source = String::new();
            for (index, (token, _)) in tokens.iter().enumerate() {
                for (shape, left, right) in [
                    ("left", qualified.as_str(), carrier),
                    ("right", carrier, qualified.as_str()),
                    ("both", qualified.as_str(), qualified.as_str()),
                ] {
                    source.push_str(&format!(
                        "machine {shape}{index}(x: {left}, y: {right}) -> {qualified} \
                         crashes {ceiling} {{ x {token} y }}\n"
                    ));
                }
            }
            let program = Program::of(&source);
            for (index, (_, name)) in tokens.iter().enumerate() {
                for shape in ["left", "right", "both"] {
                    let path = format!("{shape}{index}");
                    let operation = only_operation(&program, &path);
                    let expected = match policy {
                        "Trapping" => format!(
                            "TrappingInteger {{ operation: {name} {{ left: ValueId(1), right: ValueId(2) }} }}"
                        ),
                        _ => format!(
                            "{policy}Integer{name} {{ left: ValueId(1), right: ValueId(2) }}"
                        ),
                    };
                    assert_eq!(operation, expected, "{policy} {carrier} {path}");
                }
            }
            program.judged(|_| {}).accepted();
        }
    }

    let program = Program::of(
        "machine eq(x: i13 in Wrapping, y: i13) -> bool crashes none { x == y }
machine lt(x: i13 in Trapping, y: i13 in Trapping) -> bool crashes none { x < y }
machine ge(x: u8 in Saturating, y: u8) -> bool crashes none { x >= y }",
    );
    assert_eq!(
        only_operation(&program, "eq"),
        "IntegerEqual { left: ValueId(1), right: ValueId(2) }"
    );
    assert_eq!(
        only_operation(&program, "lt"),
        "IntegerLessThan { left: ValueId(1), right: ValueId(2) }"
    );
    assert_eq!(
        only_operation(&program, "ge"),
        "IntegerLessOrEqual { left: ValueId(2), right: ValueId(1) }"
    );
    program.judged(|_| {}).accepted();

    let uncovered = Program::of(
        "machine trap(x: u8 in Trapping, y: u8) -> u8 in Trapping crashes none { x - y }",
    );
    assert_eq!(
        uncovered.judged(|_| {}).rejected().unproved,
        ["CrashCoverage { site: Operation(OperationId(1)), cause: Trap }"]
    );
}

/// A machine of the program named `add` is still the one `add(1u8)` calls
/// beside a `Wrapping` `+`: `x + add(1u8)` calls it, then adds its result to
/// `x` under `Wrapping`.
#[test]
fn a_machine_named_add_is_called_beside_a_policy_add() {
    let program = Program::of(
        "machine add(x: u8) -> u8 { x }
machine both(x: u8 in Wrapping) -> u8 in Wrapping crashes none { x + add(1u8) }",
    );
    let lowered = program.lower();
    let operations = lowered.operations("both");
    let [_, (_, call), (_, sum)] = operations[..] else {
        panic!("{operations:?}")
    };
    let OperationKind::Call { callee, .. } = call.kind else {
        panic!("{call:?}")
    };
    assert_eq!(callee, lowered.machine_id("add"));
    let x = formals(lowered.machine("both"))[0];
    assert_eq!(
        sum.kind,
        OperationKind::WrappingIntegerAdd {
            left: x,
            right: scalar(call),
        }
    );
    program.judged(|_| {}).accepted();
}

/// `x as +Wrapping` and `x as -Wrapping` keep the carrier: each is one
/// qualification change of `x` on the edge out of the entry, with no
/// operation. `x as u13 +Wrapping` widens `x` first and qualifies the
/// widened value.
#[test]
fn a_policy_cast_keeps_its_carrier_or_widens_first() {
    let program = Program::of(
        "machine left(x: u13) -> u13 in Wrapping { x as +Wrapping }
machine erase(x: i17 in Wrapping) -> i17 { x as -Wrapping }
machine widen(x: u8) -> u13 in Wrapping { x as u13 +Wrapping }",
    );
    let lowered = program.lower();
    let coercions = &lowered
        .elaboration
        .module
        .types
        .scalar_qualifications
        .coercions;
    for (path, widens) in [("left", false), ("erase", false), ("widen", true)] {
        let machine = lowered.machine(path);
        let x = formals(machine)[0];
        let qualified = match lowered.operations(path)[..] {
            [] if !widens => x,
            [(_, widened)] if widens => {
                assert_eq!(widened.kind, OperationKind::IntegerWiden { operand: x });
                scalar(widened)
            }
            ref other => panic!("`{path}`: {other:?}"),
        };
        let rows: Vec<_> = coercions
            .iter()
            .filter(|row| row.machine == machine.id)
            .collect();
        let [row] = rows.as_slice() else {
            panic!("`{path}` changes one qualification: {rows:?}")
        };
        assert_eq!(row.source, qualified, "{path}");
        let Terminator::Jump {
            edge, arguments, ..
        } = &machine.blocks[0].terminator
        else {
            panic!("`{path}` leaves its entry on a jump")
        };
        assert_eq!(
            (row.edge, arguments.as_slice()),
            (*edge, &[qualified][..]),
            "{path}"
        );
    }
    program.judged(|_| {}).accepted();
}

/// An element read from a plain `[u8; 4]` is a plain `u8`: returning it as
/// `u8 in Mark` is refused at the read, and the same read cast with
/// `as +Mark` is accepted.
#[test]
fn an_element_read_does_not_gain_the_result_domain() {
    let refusal = Program::try_of(&[(
        "product/main.omg",
        "domain u8::Mark; machine f(a: [u8; 4], i: u64) -> u8 in Mark requires i < 4 { a[i] }",
    )])
    .err()
    .expect("a plain element is not a member of Mark");
    assert_eq!(refusal.stage, "lowering");
    assert!(
        refusal.message.starts_with(
            "Violation: this value's type carries no domain and the machine's result declares `u8::Mark`"
        ),
        "{}",
        refusal.message
    );
    Program::of(
        "domain u8::Mark; \
         machine f(a: [u8; 4], i: u64) -> u8 in Mark requires i < 4 { a[i] as +Mark }",
    )
    .judged(|_| {})
    .accepted();
}

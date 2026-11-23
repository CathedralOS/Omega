//! Stage 05 must lower each operator to the primitive and policy its token
//! names, over the operands in the order written, and owe exactly the
//! question that primitive and policy leave open: an exact operation owes
//! its own safety, a wrapping or saturating one owes nothing more than its
//! count or divisor question, and a trapping one owes crash coverage at its
//! own site. The tests write one-line machines and read what stage 05
//! produced for each.
//!
//! The rules: `wiki/spec/language/numeric_values.md`, "Policy operators",
//! "Integer quotient and remainder" and "Shift counts"; casts in
//! "Arithmetic and comparisons"; comparisons in "Integer equality and order".

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Program, returned, scalar};
use semantic_vocabulary::{CrashCause, IntegerValue, ObligationId, ValueId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_psi::{
    ExecutionSite, OperationKind, StructuralPathSegment, TerminalMachine, TrappingIntegerOperation,
};

fn formals(machine: &TerminalMachine) -> Vec<ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

/// What `path` owes, as obligation kinds in table order.
fn owed(lowered: &program::Lowered<'_>, path: &str) -> Vec<ObligationKind> {
    let machine = lowered.machine(path);
    lowered
        .elaboration
        .tables
        .machines
        .get(machine.id)
        .unwrap()
        .obligations
        .iter()
        .map(|(_, row)| row.kind)
        .collect()
}

/// The one operation of a one-operation machine, which it returns.
fn only(lowered: &program::Lowered<'_>, path: &str) -> (String, OperationKind, ExecutionSite) {
    let [(written, operation)] = lowered.operations(path)[..] else {
        panic!("`{path}` is one operation: {:?}", lowered.operations(path))
    };
    assert_eq!(returned(lowered.machine(path)), scalar(operation), "{path}");
    (
        written.to_owned(),
        operation.kind.clone(),
        ExecutionSite::Operation(operation.id),
    )
}

fn first() -> ObligationId {
    ObligationId::new(1).unwrap()
}

#[test]
fn an_integer_operator_takes_its_primitive_and_policy_from_its_token() {
    let program = Program::of(
        "machine exact_divide(value: i8, divisor: i8) -> i8 { value / divisor }
machine exact_remainder(value: i8, divisor: i8) -> i8 { value % divisor }
machine exact_shift(value: u13, count: i8) -> u13 { value << count }
machine saturating_shift(value: u13, count: i8) -> u13 { value >>| count }
machine unsigned_divide(value: u8, divisor: u8) -> u8 { value / divisor }
machine unsigned_remainder(value: u8, divisor: u8) -> u8 { value % divisor }
machine wrapping_remainder(value: i8, divisor: i8) -> i8 { value %% divisor }
machine wrapping_shift(value: u13, count: i8) -> u13 { value <<% count }
",
    );
    let lowered = program.lower();
    let obligation = first();
    // `>>|` means the same as `>>` (numeric_values.md, "Shift counts"): a
    // right shift cannot overflow, and a `|` shift keeps the count proof.
    // A wrapping remainder still owes a nonzero divisor; a wrapping shift
    // reduces its count and owes nothing.
    type Expected = fn(ValueId, ValueId, ObligationId) -> OperationKind;
    let table: [(&str, &str, Expected, bool); 8] = [
        (
            "exact_divide",
            "/",
            |left, right, obligation| OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            },
            true,
        ),
        (
            "exact_remainder",
            "%",
            |left, right, obligation| OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            },
            true,
        ),
        (
            "exact_shift",
            "<<",
            |value, count, obligation| OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            },
            true,
        ),
        (
            "saturating_shift",
            ">>",
            |value, count, obligation| OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            },
            true,
        ),
        (
            "unsigned_divide",
            "/",
            |left, right, obligation| OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            },
            true,
        ),
        (
            "unsigned_remainder",
            "%",
            |left, right, obligation| OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            },
            true,
        ),
        (
            "wrapping_remainder",
            "%%",
            |left, right, obligation| OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation,
            },
            true,
        ),
        (
            "wrapping_shift",
            "<<%",
            |value, count, _| OperationKind::WrappingIntegerShiftLeft { value, count },
            false,
        ),
    ];
    for (path, token, expected, owes) in table {
        let [left, right] = formals(lowered.machine(path))[..] else {
            panic!("two formals")
        };
        let (written, kind, site) = only(&lowered, path);
        assert_eq!(written, token, "{path}");
        assert_eq!(kind, expected(left, right, obligation), "{path}");
        let questions = if owes {
            vec![ObligationKind::OperationSafety { site }]
        } else {
            vec![]
        };
        assert_eq!(owed(&lowered, path), questions, "{path}");
    }
}

#[test]
fn nested_operations_run_inner_first_and_keep_their_own_policies() {
    let program =
        Program::of("machine add(left: u8, right: u8) -> u8 { (left +% 1) + (right +| 2u8) }\n");
    let lowered = program.lower();
    let machine = lowered.machine("add");
    let [left, right] = formals(machine)[..] else {
        panic!("two formals")
    };
    let operations = lowered.operations("add");
    let [
        (_, one),
        (wrapping_text, wrapping),
        (_, two),
        (saturating_text, saturating),
        (exact_text, exact),
    ] = operations[..]
    else {
        panic!("two constants and three additions: {operations:?}")
    };
    assert_eq!(
        (wrapping_text, saturating_text, exact_text),
        ("+%", "+|", "+")
    );
    assert_eq!(
        [&one.kind, &two.kind],
        [
            &OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1)
            },
            &OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(2)
            }
        ]
    );
    assert_eq!(
        wrapping.kind,
        OperationKind::WrappingIntegerAdd {
            left,
            right: scalar(one)
        }
    );
    assert_eq!(
        saturating.kind,
        OperationKind::SaturatingIntegerAdd {
            left: right,
            right: scalar(two)
        }
    );
    assert_eq!(
        exact.kind,
        OperationKind::ExactIntegerAdd {
            left: scalar(wrapping),
            right: scalar(saturating),
            obligation: first()
        }
    );
    assert_eq!(
        owed(&lowered, "add"),
        [ObligationKind::OperationSafety {
            site: ExecutionSite::Operation(exact.id)
        }]
    );
    assert_eq!(returned(machine), scalar(exact));
}

#[test]
fn a_comparison_orders_its_operands_by_its_operator() {
    let program = Program::of(
        "machine greater(left: i8, right: u13) -> bool { left > right }
machine less(left: u8, right: u8) -> bool { left < right }
machine at_most(left: u8, right: u8) -> bool { left <= right }
machine at_least(left: u8, right: u8) -> bool { left >= right }
machine negated(first: bool, second: bool) -> bool { !first == second }
",
    );
    let lowered = program.lower();
    for (path, swapped, strict) in [
        ("greater", true, true),
        ("less", false, true),
        ("at_most", false, false),
        ("at_least", true, false),
    ] {
        let [left, right] = formals(lowered.machine(path))[..] else {
            panic!("two formals")
        };
        let (left, right) = if swapped {
            (right, left)
        } else {
            (left, right)
        };
        let expected = if strict {
            OperationKind::IntegerLessThan { left, right }
        } else {
            OperationKind::IntegerLessOrEqual { left, right }
        };
        assert_eq!(only(&lowered, path).1, expected, "{path}");
        assert!(owed(&lowered, path).is_empty(), "{path}");
    }

    let machine = lowered.machine("negated");
    let [first, second] = formals(machine)[..] else {
        panic!("two formals")
    };
    let [(_, not), (_, equal)] = lowered.operations("negated")[..] else {
        panic!("a negation and an equality")
    };
    assert_eq!(not.kind, OperationKind::BooleanNot { operand: first });
    assert_eq!(
        equal.kind,
        OperationKind::BooleanEqual {
            left: scalar(not),
            right: second
        }
    );
    assert_eq!(returned(machine), scalar(equal));
}

#[test]
fn a_narrowing_cast_owes_its_fit_and_a_widening_cast_owes_nothing() {
    let program = Program::of(
        "machine narrow(first: u16, selected: u16) -> u8 { selected as u8 }
machine widen(first: u8, selected: u8) -> u16 { selected as u16 }
machine fraction() -> u32 { ((5 / 2) * 2) as u32 }
",
    );
    let lowered = program.lower();
    let [_, selected] = formals(lowered.machine("narrow"))[..] else {
        panic!("two formals")
    };
    let (written, kind, site) = only(&lowered, "narrow");
    assert_eq!(written, "as");
    assert_eq!(
        kind,
        OperationKind::IntegerExactCast {
            operand: selected,
            obligation: first()
        }
    );
    assert_eq!(
        owed(&lowered, "narrow"),
        [ObligationKind::OperationSafety { site }]
    );

    let [_, selected] = formals(lowered.machine("widen"))[..] else {
        panic!("two formals")
    };
    assert_eq!(
        only(&lowered, "widen").1,
        OperationKind::IntegerWiden { operand: selected }
    );
    assert!(owed(&lowered, "widen").is_empty());

    // The cast acknowledges the exact fraction 5/2 * 2 = 5, which lands as
    // one constant.
    assert_eq!(
        only(&lowered, "fraction").1,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(5)
        }
    );
}

#[test]
fn a_trapping_operator_is_its_own_trap_site_over_its_written_operands() {
    let program = Program::of(
        "machine divide(a: u8, x: u8) -> u8 crashes Trap x == 0 { a /! x }
machine remainder(a: u8, x: u8) -> u8 crashes Trap x == 0 { a %! x }
machine multiply(a: u8, x: u8) -> u8 crashes Trap { a *! x }
machine add(a: u8, x: u8) -> u8 crashes Trap { a +! x }
machine subtract(a: u8, x: u8) -> u8 crashes Trap { x -! a }
machine shift(a: u8, x: u8) -> u8 crashes Trap { a <<! x }
",
    );
    let lowered = program.lower();
    type Expected = fn(ValueId, ValueId) -> TrappingIntegerOperation;
    let table: [(&str, &str, Expected); 6] = [
        ("divide", "/!", |left, right| {
            TrappingIntegerOperation::Divide { left, right }
        }),
        ("remainder", "%!", |left, right| {
            TrappingIntegerOperation::Remainder { left, right }
        }),
        ("multiply", "*!", |left, right| {
            TrappingIntegerOperation::Multiply { left, right }
        }),
        ("add", "+!", |left, right| TrappingIntegerOperation::Add {
            left,
            right,
        }),
        // `x -! a` subtracts in the order written.
        ("subtract", "-!", |left, right| {
            TrappingIntegerOperation::Subtract {
                left: right,
                right: left,
            }
        }),
        ("shift", "<<!", |value, count| {
            TrappingIntegerOperation::ShiftLeft { value, count }
        }),
    ];
    for (path, token, expected) in table {
        let [a, x] = formals(lowered.machine(path))[..] else {
            panic!("two formals")
        };
        let (written, kind, site) = only(&lowered, path);
        assert_eq!(written, token, "{path}");
        assert_eq!(
            kind,
            OperationKind::TrappingInteger {
                operation: expected(a, x)
            },
            "{path}"
        );
        assert_eq!(
            owed(&lowered, path),
            [ObligationKind::CrashCoverage {
                site,
                cause: CrashCause::Trap
            }],
            "{path}"
        );
    }
}

#[test]
fn a_field_of_a_call_result_or_a_literal_is_read_from_that_record() {
    let program = Program::of(
        "data Cell [copy] { v: u8; w: u8; }
machine make(v: u8, w: u8) -> Cell { Cell { v: v, w: w } }
machine from_call(first: u8, last: u8) -> u8 { make(last, first).w }
machine from_literal(first: u8, last: u8) -> u8 { Cell { v: first, w: last }.v }
",
    );
    let lowered = program.lower();
    let field = |field| {
        lowered
            .elaboration
            .module
            .identities
            .text(field)
            .unwrap()
            .to_owned()
    };
    for (path, read_field) in [("from_call", "w"), ("from_literal", "v")] {
        let machine = lowered.machine(path);
        let [first, last] = formals(machine)[..] else {
            panic!("two formals")
        };
        let [(_, record), (_, read)] = lowered.operations(path)[..] else {
            panic!("`{path}` forms a record and reads it")
        };
        match &record.kind {
            OperationKind::CallStructuralWithScalarArguments {
                callee, arguments, ..
            } => {
                assert_eq!(*callee, lowered.machine_id("make"));
                assert_eq!(arguments.as_slice(), [last, first]);
            }
            OperationKind::EstablishRecord { .. } => assert_eq!(path, "from_literal"),
            other => panic!("`{path}` forms a record: {other:?}"),
        }
        let terminal_psi::OperationResult::Structural(formed) = &record.result else {
            panic!("the record is a place")
        };
        let OperationKind::PrimitiveScalarRead {
            source,
            path: steps,
        } = &read.kind
        else {
            panic!("a field read: {read:?}")
        };
        assert_eq!(*source, formed.place, "{path}");
        let [StructuralPathSegment::Field(selected)] = steps.as_slice() else {
            panic!("one field step: {steps:?}")
        };
        assert_eq!(field(*selected), read_field, "{path}");
        assert_eq!(returned(machine), scalar(read));
    }
}

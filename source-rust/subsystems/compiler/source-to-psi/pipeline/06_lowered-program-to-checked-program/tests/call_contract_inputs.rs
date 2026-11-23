//! The checker judges a call with what stage 05 wrote on it: the guarantee
//! of the callable type a runtime call goes through, and the position of each
//! argument a boundary call passes. Each test lowers a small program, changes
//! only that fact in the lowered program, and requires the checker's verdict
//! to follow, beside a source twin that changes the same fact in the text.
//!
//! The rules: "A callable field publishes the complete contract its caller
//! may rely on" (`wiki/spec/language/machine_values.md`); a call binds each
//! actual to the formal in the same position
//! (`wiki/spec/terminal-psi/calls_and_outcomes.md`, "Call contracts").

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, operation_mut, the_operation};
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ValueId};
use terminal_check_tables::edges::Actual;
use terminal_psi::{OperationKind, TerminalMachine};

fn formals(machine: &TerminalMachine) -> Vec<ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

fn reader(bound: u32) -> String {
    format!(
        "data ByteReader = machine(offset: u32) -> u8 requires offset < 256 \
         ensures result < {bound} reaches none crashes none;
machine read(reader: ByteReader, offset: u32) -> u8 requires offset < 256 ensures result < 200 {{ reader(offset) }}
"
    )
}

/// `read` promises `result < 200` and returns what `reader(offset)`
/// returns, so only the callable type's own `ensures result < 200` proves
/// it. Raising that bound to 201, in the source or in the lowered callable
/// contract alone, leaves `read`'s promise unproved.
#[test]
fn a_runtime_call_gives_its_caller_the_guarantee_of_its_callable_type() {
    let program = Program::of(&reader(200));
    let call = the_operation(program.machine("read"), |kind| {
        matches!(kind, OperationKind::CallMachine { .. })
    });
    let OperationKind::CallMachine { contract, .. } = call.kind else {
        unreachable!()
    };
    let callables = &program.elaboration().module.execution.callables.contracts;
    let [callable] = callables.as_slice() else {
        panic!("one callable contract: {callables:?}")
    };
    assert_eq!(callable.id, contract);
    let [guarantee] = callable.proof.ensures.as_slice() else {
        panic!(
            "the callable type guarantees one clause: {:?}",
            callable.proof.ensures
        )
    };
    assert!(
        matches!(
            guarantee.proposition,
            Proposition::LessThan(
                ScalarTerm::Value { .. },
                ScalarTerm::Integer {
                    value: IntegerValue::Unsigned(200),
                    ..
                }
            )
        ),
        "{guarantee:?}"
    );
    program.judged(|_| {}).accepted();

    let twin = Program::of(&reader(201)).judged(|_| {}).rejected();
    let raised = program
        .judged(|lowered| {
            let [callable] = lowered.module.execution.callables.contracts.as_mut_slice() else {
                unreachable!()
            };
            let Proposition::LessThan(
                _,
                ScalarTerm::Integer {
                    value: IntegerValue::Unsigned(bound),
                    ..
                },
            ) = &mut callable.proof.ensures[0].proposition
            else {
                unreachable!()
            };
            *bound = 201;
        })
        .rejected();
    assert_eq!(raised.unproved, twin.unproved, "{raised:?}");
    assert!(!raised.unproved.is_empty(), "{raised:?}");
}

fn device(call: &str) -> String {
    format!(
        "boundary trait Device {{ machine read(n: u8, bytes: @[u8], last: u8) -> u8 requires n < 4 reaches Device crashes none; }}
machine run(n: u8, b: @[u8], last: u8) -> u8 requires last < 4 reaches Device {{ {call} }}
"
    )
}

/// `Device::read(last, b, n)` passes the caller's `last` as the boundary's
/// `n`, which the boundary requires below 4; the caller proves that only for
/// `last`. The two scalars have one type, so only their positions tell them
/// apart: the source twin `Device::read(n, b, last)`, and the lowered call
/// with only its two scalar actuals exchanged, both leave the requirement
/// unproved.
#[test]
fn a_boundary_call_passes_equal_typed_scalars_in_the_positions_written() {
    let program = Program::of(&device("Device::read(last, b, n)"));
    let machine = program.machine("run");
    let [n, last] = formals(machine)[..] else {
        panic!("two scalar formals")
    };
    let call = the_operation(machine, |kind| {
        matches!(kind, OperationKind::BoundaryCall { .. })
    });
    let OperationKind::BoundaryCall { arguments, .. } = &call.kind else {
        unreachable!()
    };
    assert_eq!(arguments.as_slice(), [last, n]);
    program.judged(|_| {}).accepted();

    let twin = Program::of(&device("Device::read(n, b, last)"))
        .judged(|_| {})
        .rejected();
    assert!(!twin.unproved.is_empty(), "{twin:?}");

    let id = call.id;
    let exchanged = program
        .judged(|lowered| {
            let position = lowered
                .module
                .machines
                .iter()
                .position(|candidate| candidate.id == machine.id)
                .unwrap();
            let lowered_machine = &mut lowered.module.machines[position];
            let OperationKind::BoundaryCall { arguments, .. } =
                &mut operation_mut(lowered_machine, id).kind
            else {
                unreachable!()
            };
            arguments.swap(0, 1);
            let tables = lowered.tables.machines.get_mut(machine.id).unwrap();
            let invocations: Vec<_> = tables.invocations.iter().map(|(id, _)| id).collect();
            for invocation in invocations {
                for actual in &mut tables.invocations.get_mut(invocation).unwrap().actuals {
                    if let Actual::Value(value) = actual {
                        *value = if *value == n { last } else { n };
                    }
                }
            }
        })
        .rejected();
    assert_eq!(exchanged.unproved, twin.unproved, "{exchanged:?}");
}

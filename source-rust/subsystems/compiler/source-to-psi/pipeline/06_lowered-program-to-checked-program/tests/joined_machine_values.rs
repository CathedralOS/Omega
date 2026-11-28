//! A machine that calls a machine value it received as an argument may enter
//! that argument's code, so its `invokes` row must name the formal. When a
//! `match` chooses between two such formals and the program calls the
//! result, the call may enter either one. Stage 05 infers the row of a
//! machine that writes none, and the lowered-form checker computes, for
//! each call through a machine value, which formals the value may have come
//! from and requires the row to cover them.
//!
//! The spec rule is `wiki/spec/language/effects.md`, "Direct synchronous invocation":
//! "Call of a callable argument or a statically bound callback | Yes", and
//! for a checked body with no `invokes` clause, "Infer and publish its
//! entries". A named machine selected in place is not a callable argument:
//! calling it enters what its callable type permits, and these types permit
//! nothing (`wiki/spec/language/machine_values.md`).
//!
//! The rows below number formals from zero (`flag` is 0); the checker's
//! messages count from one.

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, the_operation};
use terminal_psi::{OperationKind, TerminalCallableInvocation, Terminator};

const TYPES: &str = "data Left = machine(x: u8) -> u8 reaches none crashes none; \
    data Right = machine(x: u8) -> u8 reaches none crashes none; \
    data Chosen = machine(x: u8) -> u8 reaches none crashes none; \
    machine named(x: u8) -> u8 reaches none crashes none { x }";

fn choose(clauses: &str, arms: &str) -> Program {
    Program::of(&format!(
        "{TYPES} machine choose(flag: bool, left: Left, right: Right, n: u8) -> u8 {clauses} \
         {{ let chosen: Chosen = match flag {{ {arms} }}; chosen(n) }}"
    ))
}

/// The invocation row stage 05 published for `choose`.
fn invokes(program: &Program) -> Vec<TerminalCallableInvocation> {
    let id = program.machine("choose").id;
    program
        .elaboration()
        .module
        .execution
        .callables
        .machine_envelope(id)
        .expect("choose has a published envelope")
        .invocations
        .clone()
}

/// `chosen(n)` calls whichever of `left` and `right` the `match` chose, so
/// `choose` invokes both formals, and stage 05 publishes both for a body
/// that writes no `invokes` clause. The call takes its callee from the
/// join, and the two arms hand the join exactly the two formals. Dropping
/// `right` from the published row, as stage 05 did when it followed only
/// a value bound straight from one formal, leaves the call outside the row,
/// and the checker says which formal is missing.
#[test]
fn a_call_through_a_joined_machine_value_invokes_each_formal_that_reaches_it() {
    let program = choose("", "false -> left, true -> right,");
    assert_eq!(
        invokes(&program),
        [
            TerminalCallableInvocation::Parameter(1),
            TerminalCallableInvocation::Parameter(2),
        ]
    );
    let machine = program.machine("choose");
    let call = the_operation(machine, |kind| {
        matches!(kind, OperationKind::CallMachine { .. })
    });
    let OperationKind::CallMachine { callee, .. } = call.kind else {
        unreachable!()
    };
    let join = machine
        .blocks
        .iter()
        .find(|block| block.parameters.iter().any(|value| value.id == callee))
        .expect("the callee is a parameter of the join");
    let mut arrivals: Vec<_> = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } if *target == join.id => Some(arguments.clone()),
            _ => None,
        })
        .collect();
    arrivals.sort();
    let formal = |position: usize| machine.contract.parameters[position].id;
    assert_eq!(arrivals, [vec![formal(1)], vec![formal(2)]]);
    program.judged(|_| {}).accepted();

    let id = machine.id;
    let rejection = program
        .judged(|lowered| {
            let row = lowered
                .module
                .execution
                .callables
                .machine_envelopes
                .iter_mut()
                .flatten()
                .find(|row| row.machine == id)
                .unwrap();
            row.envelope
                .invocations
                .retain(|entry| *entry != TerminalCallableInvocation::Parameter(2));
        })
        .rejected();
    assert_eq!(
        rejection.exceeded,
        [
            "machine `choose` invokes `parameter 2` but its call of `evaluated callable \
             value 8` contributes `parameter 2 + parameter 3`, which adds `parameter 3`"
        ],
        "{rejection:?}"
    );

    // The source twin writes the same short row, and the checker refuses
    // it with the same statement.
    let written = choose("invokes left", "false -> left, true -> right,")
        .judged(|_| {})
        .rejected();
    assert_eq!(written.exceeded, rejection.exceeded, "{written:?}");
}

/// The forwarded record has two callbacks of the same type. Its caller may
/// enter only the one the helper calls, not any callback in the record.
#[test]
fn borrowing_and_forwarding_a_record_invokes_only_its_selected_field() {
    let program = Program::of(
        "data Read = machine() -> i32 reaches none crashes none; \
        data Preview { active: Read; idle: Read; } \
        machine display(operations: @Preview) -> i32 { operations.active() } \
        machine display_idle(operations: @Preview) -> i32 { operations.idle() } \
        machine choose(operations: @Preview) -> i32 { display(operations) }",
    );
    let row = invokes(&program);
    let [TerminalCallableInvocation::ProjectedParameter { parameter: 0, path }] = &row[..] else {
        panic!("forwarding lost the active field: {row:?}");
    };
    let Some(semantic_vocabulary::InvocationPathSegment::Field(name)) = path.last() else {
        panic!("expected the selected field, got {path:?}");
    };
    assert!(name.ends_with("active"), "{name}");
    program.judged(|_| {}).accepted();
    let id = program.machine("choose").id;
    use source_compiler::test_support::checking::{RouteVerdict, judge};
    let RouteVerdict::Accepted(accepted) = judge(Ok(program.elaboration().clone())).unwrap() else {
        panic!("the checked forwarding body must publish");
    };
    let module = accepted.prepare_publication().unwrap();
    let bytes = terminal_codec::encode_module(module.module()).unwrap();
    let reopened = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(
        reopened
            .execution
            .callables
            .machine_envelope(id)
            .unwrap()
            .invocations,
        row
    );
    let idle = program
        .elaboration()
        .module
        .execution
        .callables
        .machine_envelope(program.machine("display_idle").id)
        .expect("idle has its own inferred effects")
        .invocations
        .clone();
    assert_ne!(row, idle);
    let rejection = program
        .judged(|lowered| {
            let row = lowered
                .module
                .execution
                .callables
                .machine_envelopes
                .iter_mut()
                .flatten()
                .find(|row| row.machine == id)
                .unwrap();
            row.envelope.invocations = idle;
        })
        .rejected();
    assert!(
        rejection
            .exceeded
            .iter()
            .any(|error| error.contains("active")),
        "{rejection:?}"
    );
}

/// When one arm is the formal `left` and the other selects the machine
/// `named` in place, the call may enter only `left` among the formals:
/// stage 05 publishes that one, and the checker accepts it.
#[test]
fn a_joined_named_machine_adds_no_formal_to_the_invocation_row() {
    let program = choose("", "false -> left, true -> named,");
    assert_eq!(
        invokes(&program),
        [TerminalCallableInvocation::Parameter(1)]
    );
    program.judged(|_| {}).accepted();
}

fn measure(requires: &str) -> Program {
    Program::of(&format!(
        "data Measure = machine(n: u64) -> u8 requires n < 1000 ensures result < 200 \
         reaches none crashes none; \
         machine pick<'a>(flag: bool, left: @'a [u32], right: @'a [u32], f: Measure, \
         g: Measure) -> u8 requires left.length < 1000 {requires} {{ \
         let chosen: @'a [u32] = match flag {{ true -> left, false -> right, }}; \
         let m: Measure = match flag {{ true -> f, false -> g, }}; \
         m(chosen.length) }}"
    ))
}

/// A view chosen by one `match` and a machine value chosen by another meet
/// at one call. The call owes `Measure`'s `requires n < 1000` on the length
/// of whichever view arrived, and invokes whichever of `f` and `g` arrived,
/// so `pick` must bound both views' lengths and invokes both formals.
/// Without the bound on `right`, the call's requirement is unproved.
#[test]
fn a_joined_view_and_a_joined_machine_value_meet_at_one_call() {
    let program = measure("requires right.length < 1000");
    let id = program.machine("pick").id;
    assert_eq!(
        program
            .elaboration()
            .module
            .execution
            .callables
            .machine_envelope(id)
            .unwrap()
            .invocations,
        [
            TerminalCallableInvocation::Parameter(3),
            TerminalCallableInvocation::Parameter(4),
        ]
    );
    program.judged(|_| {}).accepted();

    let unbounded = measure("").judged(|_| {}).rejected();
    assert_eq!(
        unbounded.unproved,
        ["CallRequirement { invocation: InvocationId(1), clause: 0 }"],
        "{unbounded:?}"
    );
}

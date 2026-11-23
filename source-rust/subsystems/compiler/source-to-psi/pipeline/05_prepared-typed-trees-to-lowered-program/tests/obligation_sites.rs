//! Stage 05 must hand the checker each question at the operation that owes
//! it, about the declaration the source selected there: a zeroed local is
//! the record its annotation names and owes that record's own clauses where
//! it is zeroed, each closed application of a record is its own type, a call
//! owes coverage for each crash route of the machine it calls, and each
//! callback application calls its own selected machine. The tests write
//! small programs, read what stage 05 produced, and check a twin that changes
//! the one input the fact depends on.
//!
//! The spec rules: zero initialization owes the declared default domain
//! (`wiki/spec/language/dependent_values.md`, "Default domains and zero
//! initialization"); const arguments are part of an applied type's identity
//! (`wiki/spec/language/generics.md`); a caller covers each crash route of
//! its callee (`wiki/spec/language/effects.md`, "Guarded crashes"); a static
//! callback application calls the selected machine
//! (`wiki/spec/language/effects.md`, "Static binder contract dependencies").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Lowered, Program};
use semantic_vocabulary::{CrashCause, StructuralTypeId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_psi::{
    ExecutionSite, Operation, OperationKind, TerminalAffineCleanupAction, TerminalMachine,
    Terminator,
};

/// The identity text stage 05 published for a record type.
fn record_identity(lowered: &Lowered<'_>, id: StructuralTypeId) -> String {
    let module = &lowered.elaboration.module;
    let declared = module
        .types
        .structural_types
        .iter()
        .find(|declared| declared.id == id)
        .expect("the type is declared");
    module
        .identities
        .text(declared.identity)
        .unwrap()
        .to_owned()
}

/// What `path` owes, in table order.
fn owed(lowered: &Lowered<'_>, path: &str) -> Vec<ObligationKind> {
    lowered
        .elaboration
        .tables
        .machines
        .get(lowered.machine(path).id)
        .unwrap()
        .obligations
        .iter()
        .map(|(_, row)| row.kind)
        .collect()
}

/// The one zeroing in `machine`.
fn zeroing(machine: &TerminalMachine) -> &Operation {
    program::the_operation(machine, |kind| {
        matches!(kind, OperationKind::EstablishZeroedStructural {})
    })
}

/// `let cell: First;` zeroes a `First`, not the `Other` declared beside it
/// with the same field, and `init` discards that record on its way out.
/// Naming `Other` instead zeroes an `Other`.
#[test]
fn a_zeroed_local_is_the_record_its_annotation_names() {
    for (named, other) in [("First", "Other"), ("Other", "First")] {
        let program = Program::of(&format!(
            "data First {{ v: u8; }} data Other {{ v: u8; }} \
             machine init() {{ let cell: {named}; }}"
        ));
        let lowered = program.lower();
        let init = lowered.machine("init");
        let zeroed = zeroing(init);
        assert_eq!(lowered.written(init, zeroed.id), "cell");
        let result = zeroed.result.as_structural().unwrap();
        let identity = record_identity(&lowered, result.structural_type);
        assert!(identity.contains(&format!("::{named}\\)")), "{identity}");
        assert!(!identity.contains(&format!("::{other}\\)")), "{identity}");
        let [block] = init.blocks.as_slice() else {
            panic!("`init` is one block")
        };
        let Terminator::ReturnUnit {
            cleanup_actions, ..
        } = &block.terminator
        else {
            panic!("{:?}", block.terminator)
        };
        assert_eq!(
            cleanup_actions,
            &[TerminalAffineCleanupAction::DiscardRoot(result.place)]
        );
        program.judged(|_| {}).accepted();
    }
}

/// A zeroed `Small` owes each of its two clauses at the zeroing, in written
/// order, and the same record without clauses owes nothing. A `Holder` with
/// no clause of its own owes the clause of the `Inner` it holds, and nothing
/// once `Inner` has none. Whether zero meets a clause is the checker's
/// question, asked where the zeroed record is used
/// (`lowered_facts::zeroed_storage_must_meet_its_record_predicate_before_it_is_used`).
#[test]
fn a_zeroed_record_owes_its_own_clauses_where_it_is_zeroed() {
    let program = Program::of(
        "data Small requires self.value < 10 requires self.value > 2 { value: u8; } \
         machine init() { let mutable cell: Small; }",
    );
    let lowered = program.lower();
    let zeroed = zeroing(lowered.machine("init")).id;
    assert_eq!(
        owed(&lowered, "init"),
        [
            ObligationKind::DataDomain {
                operation: zeroed,
                clause: 0
            },
            ObligationKind::DataDomain {
                operation: zeroed,
                clause: 1
            },
        ]
    );

    let plain =
        Program::of("data Small { value: u8; } machine init() { let mutable cell: Small; }");
    assert_eq!(owed(&plain.lower(), "init"), []);

    for (clause, owes) in [(" requires self.v > 0", true), ("", false)] {
        let program = Program::of(&format!(
            "data Inner [copy]{clause} {{ v: u8; }} data Holder {{ inner: Inner; }}              machine init() {{ let mutable cell: Holder; }}"
        ));
        let lowered = program.lower();
        let zeroed = zeroing(lowered.machine("init")).id;
        let expected: &[ObligationKind] = if owes {
            &[ObligationKind::DataDomain {
                operation: zeroed,
                clause: 0,
            }]
        } else {
            &[]
        };
        assert_eq!(owed(&lowered, "init"), expected, "{clause:?}");
    }
}

/// `Slot<3>` and `Slot<4>` lay out the same fields, but each closed
/// application is its own record type: `a` and `c` take one type and `b`
/// another.
#[test]
fn each_closed_application_is_its_own_record_type() {
    let program = Program::of(
        "data Slot<const N: u64> { value: u8; } \
         machine a(x: Slot<3>) {} machine b(x: Slot<4>) {} machine c(x: Slot<3>) {}",
    );
    let lowered = program.lower();
    let [a, b, c] = ["a", "b", "c"].map(|path| {
        let [parameter] = lowered
            .machine(path)
            .contract
            .structural_parameters
            .as_slice()
        else {
            panic!("`{path}` takes one record")
        };
        parameter.structural_type
    });
    assert_eq!(a, c);
    assert_ne!(a, b);
    assert_ne!(record_identity(&lowered, a), record_identity(&lowered, b));
    program.judged(|_| {}).accepted();
}

const ROUTED: &str = "machine plain(x: u8) -> u8 crashes none { x }";

fn routed(callee: &str, caller: &str) -> String {
    format!(
        "{ROUTED} machine routed(x: u8, y: u8) -> u8 {callee} {{ x }} \
         machine outer(a: u8, b: u8) -> u8 {caller} {{ routed(plain(a), b) }}"
    )
}

/// `routed` may abort when its `y` is 0 and may trap. Its call in `outer`
/// passes `plain(a)` and `b`, and owes coverage for each cause at that call,
/// with the abort guard over `b`. A caller that declares `crashes none`
/// covers neither and is rejected for the abort; a callee that declares
/// `crashes none` is owed nothing.
#[test]
fn a_call_owes_coverage_for_each_route_of_its_callee() {
    let program = Program::of(&routed(
        "crashes Abort y == 0 crashes Trap",
        "crashes Abort crashes Trap",
    ));
    let lowered = program.lower();
    let outer = lowered.machine("outer");
    let b = outer.contract.parameters[1].id;
    let [(_, inner), (written, call)] = lowered.operations("outer")[..] else {
        panic!("`outer` is two calls: {:?}", lowered.operations("outer"))
    };
    assert_eq!(written, "routed(plain(a), b)");
    let OperationKind::Call {
        callee,
        arguments,
        crash_continuations,
        ..
    } = &call.kind
    else {
        panic!("{call:?}")
    };
    assert_eq!(*callee, lowered.machine_id("routed"));
    assert_eq!(arguments, &[program::scalar(inner), b]);
    let causes: Vec<_> = crash_continuations
        .iter()
        .map(|route| route.cause)
        .collect();
    assert_eq!(causes, [CrashCause::Trap, CrashCause::Abort]);
    let abort = format!("{:?}", crash_continuations[1].alternatives);
    assert!(abort.contains(&format!("{b:?}")), "{abort}");
    let site = ExecutionSite::Operation(call.id);
    assert_eq!(
        owed(&lowered, "outer"),
        [
            ObligationKind::CrashCoverage {
                site,
                cause: CrashCause::Trap
            },
            ObligationKind::CrashCoverage {
                site,
                cause: CrashCause::Abort
            },
        ]
    );
    program.judged(|_| {}).accepted();

    let uncovered = Program::of(&routed("crashes Abort y == 0", "crashes none"));
    let rejection = uncovered.judged(|_| {}).rejected();
    assert_eq!(
        rejection.unproved,
        ["CrashCoverage { site: Operation(OperationId(2)), cause: Abort }"]
    );

    let calm = Program::of(&routed("crashes none", "crashes none"));
    assert_eq!(owed(&calm.lower(), "outer"), []);
    calm.judged(|_| {}).accepted();
}

/// `apply::<one>(a)` and `apply::<two>(a)` call two applications of
/// `apply`, and each application calls the machine its own call selected.
#[test]
fn each_callback_application_calls_its_own_selected_machine() {
    let program = Program::of(
        "data ByteStep = machine(x: u8) -> u8; \
         machine one(x: u8) -> u8 { x } machine two(x: u8) -> u8 { x } \
         machine apply<const F: ByteStep>(v: u8) -> u8 { F(v) } \
         machine first(a: u8) -> u8 { apply::<one>(a) } \
         machine second(a: u8) -> u8 { apply::<two>(a) }",
    );
    let lowered = program.lower();
    let mut applications = Vec::new();
    for (caller, selected) in [("first", "one"), ("second", "two")] {
        let [(_, call)] = lowered.operations(caller)[..] else {
            panic!("`{caller}` is one call")
        };
        let OperationKind::Call {
            callee, arguments, ..
        } = &call.kind
        else {
            panic!("{call:?}")
        };
        assert_eq!(
            arguments,
            &[lowered.machine(caller).contract.parameters[0].id]
        );
        let application = lowered
            .elaboration
            .module
            .machines
            .iter()
            .find(|machine| machine.id == *callee)
            .expect("the application is lowered");
        let [inside] = application.operations().collect::<Vec<_>>()[..] else {
            panic!("the application is one call")
        };
        let OperationKind::Call {
            callee: target,
            arguments: forwarded,
            ..
        } = &inside.kind
        else {
            panic!("{inside:?}")
        };
        assert_eq!(*target, lowered.machine_id(selected), "{caller}");
        assert_eq!(forwarded, &[application.contract.parameters[0].id]);
        applications.push(*callee);
    }
    assert_ne!(applications[0], applications[1]);
    program.judged(|_| {}).accepted();
}

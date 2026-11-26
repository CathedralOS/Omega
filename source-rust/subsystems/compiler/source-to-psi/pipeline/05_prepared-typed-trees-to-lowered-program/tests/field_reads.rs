//! Stage 05 must read a field from the record the source names, through the
//! exact path written, and must dispose of an owned receiver only after the
//! read, with the cleanup its type selects. When a `match` chooses the
//! record, the read must take the record the chosen arm hands the join. The
//! tests write small programs, read the operations and edges stage 05
//! produced, and change one input at a time.
//!
//! The spec rules: a field read names its receiver and field
//! (`wiki/spec/language/expressions.md`); a value-position `match` has the
//! one type of its arms and evaluates only the arm that matches
//! (`expressions.md`, "Evaluation schedule"); a record `dropped by` a
//! machine is disposed of by that machine (`wiki/spec/language/ownership.md`).

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Lowered, Program, returned, scalar};
use semantic_vocabulary::PlaceId;
use terminal_psi::{
    Operation, OperationKind, StructuralPathSegment, TerminalAffineCleanupAction,
    TerminalIdentityIndex, TerminalMachine, Terminator,
};

/// The read `machine` returns, as `(place, path)`.
fn returned_read(machine: &TerminalMachine) -> (PlaceId, Vec<StructuralPathSegment>) {
    let value = returned(machine);
    let read = machine
        .operations()
        .find(|operation| operation.result.scalar_ref().map(|result| result.id) == Some(value))
        .expect("the returned value is computed");
    let OperationKind::PrimitiveScalarRead { source, path } = &read.kind else {
        panic!("the machine returns a field read: {read:?}")
    };
    (*source, path.clone())
}

/// The identity of the field a path segment names, by its declared name.
fn field_named(lowered: &Lowered<'_>, name: &str) -> TerminalIdentityIndex {
    let module = &lowered.elaboration.module;
    let found: Vec<_> = module
        .types
        .structural_types
        .iter()
        .flat_map(|declared| match &declared.shape {
            terminal_psi::StructuralTypeShape::Record { fields, .. } => fields.clone(),
            _ => Vec::new(),
        })
        .filter(|field| module.identities.text(field.identity) == Some(name))
        .map(|field| field.identity)
        .collect();
    let [field] = found[..] else {
        panic!("one field `{name}`: {found:?}")
    };
    field
}

fn return_cleanup(machine: &TerminalMachine) -> &[TerminalAffineCleanupAction] {
    let found: Vec<_> = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Return {
                cleanup_actions, ..
            } => Some(cleanup_actions.as_slice()),
            _ => None,
        })
        .collect();
    let [cleanup] = found[..] else {
        panic!("one return")
    };
    cleanup
}

/// `p.right.w` reads through `right` and then `w` from the formal `p`; the
/// twin `p.left.w` changes only the first step.
#[test]
fn a_nested_read_keeps_the_whole_path_from_its_record() {
    for (outer, inner) in [("right", "w"), ("left", "w"), ("left", "v")] {
        let program = Program::of(&format!(
            "data Cell [copy] {{ v: u8; w: u8; }} data Pair [copy] {{ left: Cell; right: Cell; }} \
             machine read(p: Pair) -> u8 {{ p.{outer}.{inner} }}"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("read");
        let (place, path) = returned_read(machine);
        assert_eq!(place, machine.contract.structural_parameters[0].place);
        assert_eq!(
            path,
            [
                StructuralPathSegment::Field(field_named(&lowered, outer)),
                StructuralPathSegment::Field(field_named(&lowered, inner)),
            ],
            "p.{outer}.{inner}"
        );
    }
}

/// An owned receiver, whether a formal, a call's result or a record built
/// in place, is read first and disposed of on the return edge after the
/// read. `make(last, first)` passes its operands in that order, so the read
/// returns `first`.
#[test]
fn an_owned_receiver_is_disposed_of_after_its_field_is_read() {
    let cell = "data Cell { v: u8; w: u8; }";
    for (body, formal) in [
        ("machine read(cell: Cell) -> u8 { cell.w }", true),
        (
            "machine make(v: u8, w: u8) -> Cell { Cell { v: v, w: w } } \
             machine read(first: u8, last: u8) -> u8 { make(last, first).w }",
            false,
        ),
        (
            "machine read(first: u8, last: u8) -> u8 { (Cell { v: last, w: first }).w }",
            false,
        ),
    ] {
        let program = Program::of(&format!("{cell} {body}"));
        let lowered = program.lower();
        let machine = lowered.machine("read");
        let (place, path) = returned_read(machine);
        assert_eq!(
            path,
            [StructuralPathSegment::Field(field_named(&lowered, "w"))],
            "{body}"
        );
        if formal {
            assert_eq!(place, machine.contract.structural_parameters[0].place);
        } else {
            let built = machine
                .operations()
                .find(|operation| {
                    operation
                        .result
                        .as_structural()
                        .is_some_and(|result| result.place == place)
                })
                .expect("the receiver is built in the machine");
            let first = machine.contract.parameters[0].id;
            let last = machine.contract.parameters[1].id;
            match &built.kind {
                OperationKind::CallStructuralWithScalarArguments { arguments, .. } => {
                    assert_eq!(arguments.as_slice(), [last, first]);
                }
                OperationKind::EstablishRecord { .. } => {}
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(
            return_cleanup(machine),
            [TerminalAffineCleanupAction::DiscardRoot(place)],
            "{body}"
        );
        program.judged(|_| {}).accepted();
    }
}

fn hooked(hook: &str) -> Program {
    Program::of(&format!(
        "data Cell dropped by {hook} {{ mark: u8; }} \
         machine finish(x: Cell) {{ let Cell {{ mark as _ }} = x; }} \
         machine release(x: Cell) {{ let Cell {{ mark as _ }} = x; }} \
         machine keep(n: u8) -> u8 {{ n }} \
         machine read(n: u8) -> u8 {{ keep((Cell {{ mark: n }}).mark) }}"
    ))
}

/// The field of a temporary record is passed to `keep`, and the temporary
/// is then disposed of by the machine its type names: `release`, or in the
/// twin `finish`.
#[test]
fn a_temporary_receiver_is_disposed_of_by_its_own_cleanup_after_the_call() {
    for hook in ["release", "finish"] {
        let program = hooked(hook);
        let lowered = program.lower();
        let machine = lowered.machine("read");
        let operations: Vec<&Operation> = machine.operations().collect();
        let [built, read, call] = operations[..] else {
            panic!("construction, read and call: {operations:?}")
        };
        assert!(matches!(built.kind, OperationKind::EstablishRecord { .. }));
        assert!(matches!(
            &call.kind,
            OperationKind::Call { callee, arguments, .. }
                if *callee == lowered.machine_id("keep") && arguments.as_slice() == [scalar(read)]
        ));
        let [TerminalAffineCleanupAction::InvokeOwned(cleanup)] = return_cleanup(machine) else {
            panic!("{:?}", return_cleanup(machine))
        };
        assert_eq!(
            cleanup.subject.place,
            built.result.as_structural().unwrap().place
        );
        assert_eq!(cleanup.cleanup_machine, lowered.machine_id(hook), "{hook}");
        program.judged(|_| {}).accepted();
    }
}

fn joined(selection: &str) -> Program {
    Program::of(&format!(
        "data Cell [copy] {{ v: u8; w: u8; }} \
         machine read(flag: bool, left: Cell, right: Cell) -> u8 {{ {selection} }}"
    ))
}

/// A field read of a `match` result reads the record the join receives,
/// whether the result is bound to a local without a type or read in place.
/// The arm taken when `flag` is true hands the join the formal it names, so
/// exchanging the arms exchanges the formals.
#[test]
fn a_field_of_a_joined_record_reads_the_record_the_chosen_arm_hands_over() {
    for (selection, on_true) in [
        (
            "let chosen: Cell = match flag { true -> left, false -> right, }; chosen.w",
            1,
        ),
        (
            "let chosen: Cell = match flag { true -> right, false -> left, }; chosen.w",
            2,
        ),
        ("(match flag { true -> left, false -> right, }).w", 1),
        ("(match flag { true -> right, false -> left, }).w", 2),
    ] {
        let program = joined(selection);
        let lowered = program.lower();
        let machine = lowered.machine("read");
        let (place, path) = returned_read(machine);
        assert_eq!(
            path,
            [StructuralPathSegment::Field(field_named(&lowered, "w"))],
            "{selection}"
        );
        let join = machine
            .blocks
            .iter()
            .find(|block| {
                block
                    .structural_parameters
                    .iter()
                    .any(|joined| joined.place == place)
            })
            .unwrap_or_else(|| panic!("the read takes a joined record: {selection}"))
            .id;
        let Terminator::Conditional { when_true, .. } = &machine.blocks[0].terminator else {
            panic!("the match branches on `flag`")
        };
        let true_arm = machine
            .blocks
            .iter()
            .find(|block| block.id == when_true.target)
            .unwrap();
        let Terminator::Jump {
            target,
            structural_arguments,
            ..
        } = &true_arm.terminator
        else {
            panic!("the arm jumps to the join")
        };
        assert_eq!(*target, join);
        let [handed] = structural_arguments.as_slice() else {
            panic!("the arm hands over one record")
        };
        let copied = true_arm
            .operations
            .iter()
            .find(|operation| {
                operation
                    .result
                    .as_structural()
                    .is_some_and(|result| result.place == handed.place)
            })
            .unwrap();
        let formal = machine.contract.structural_parameters[on_true - 1].place;
        assert!(
            matches!(copied.kind, OperationKind::StructuralLeafCopy { source, .. } if source == formal),
            "{selection}: {copied:?}"
        );
        program.judged(|_| {}).accepted();
    }
}

/// A field read still observes the element's declared bound at its actual
/// index. Unread neighbors add no storage snapshots, regardless of capacity.
#[test]
fn a_readers_snapshots_depend_on_its_reads_not_on_the_array_capacity() {
    let [small, large] = [32, 65_536].map(|length| {
        let program = Program::of(&format!(
            "data Cell requires value <= 31, {{ value: u64; }}
             data Table {{ rows: [Cell; {length}]; }}
             machine read(table: @Table) -> u64 crashes none {{ 31 -! table.rows[3].value }}"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("read");
        let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
        (tables.places.len(), tables.values.len())
    });
    assert!(small.0 > 0);
    assert_eq!(small, large, "an unread index acquired a snapshot");
}

/// The callee may mutate the entire field it receives. Its caller keeps the
/// sibling array, including every element's default. The array's capacity
/// should have no bearing on the exact places needed by this call.
#[test]
fn passing_one_field_keeps_the_sibling_arrays_out_of_the_call_snapshot() {
    let [small, large] = [32, 4096].map(|length| {
        let program = Program::of(&format!(
            "data Cell requires value <= 31, {{ value: u64; }}
             data Counter {{ count: u64; }}
             data Table requires stamp <= 9, {{ stamp: u64; active: Counter; rows: [Cell; {length}]; }}
             machine touch(counter: @mutable Counter) {{ counter.count = 7; }}
             machine projected(table: @mutable Table) {{ touch(@mutable table.active); }}
             machine whole(table: @mutable Table) {{ projected(table); }}"
        ));
        let lowered = program.lower();
        ["projected", "whole"].map(|name| {
            let machine = lowered.machine(name);
            let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
            (tables.places.len(), tables.values.len())
        })
    });
    assert_eq!(small[0], large[0]);
    assert!(large[1].0 > small[1].0);
    assert!(large[1].1 > small[1].1);
}

/// Both fields of the enclosing record matter when its own invariant relates
/// them. The selected write must satisfy that predicate on return.
#[test]
fn a_field_handoff_still_checks_the_sibling_named_by_its_owner() {
    let program = |stored| {
        Program::of(&format!(
            "data Cell requires value <= 31, {{ value: u64; }}
             data Counter {{ count: u64; }}
             data Table requires active.count <= rows[7].value, {{ active: Counter; rows: [Cell; 8]; }}
             machine touch(counter: @mutable Counter) ensures counter.count == {stored} {{ counter.count = {stored}; }}
             machine projected(table: @mutable Table) {{ touch(@mutable table.active); }}"
        ))
    };
    program(0).judged(|_| {}).accepted();
    assert!(matches!(
        program(32).judged(|_| {}),
        program::Verdict::Rejected(_)
    ));
}

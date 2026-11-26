//! Stage 05 must say which input a returned reference comes from, through
//! which path, and release each reference a dying record still holds. The
//! lowered-form checker relies on that origin when it judges what the
//! caller's loan covers. These tests lower programs whose twins return or
//! keep the same-typed sibling, assert the origin stage 05 published, and
//! change only that origin in the lowered program.
//!
//! The spec rules: "Source and result claims keep their provenance through
//! calls, terminal transfers, wrappers, and generic substitutions"
//! (`wiki/spec/language/ownership.md`, "Paths and control flow"); checking
//! "checks the result mapping against the returned carrier", and
//! "`ReleaseReference` names an owned carrier root and its exact finite
//! field/fixed-index path ... It neither crosses a reference nor disposes
//! the referent" (`wiki/spec/terminal-psi/structural_access.md`).

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, operation_mut, the_operation};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    OperationKind, ReferenceOriginProjection, StructuralAccess, StructuralPathSegment,
    StructuralReferenceOrigin, TerminalMachine, TerminalMachineResult,
};

fn formal(machine: &TerminalMachine, position: u32) -> PlaceId {
    machine
        .structural_places
        .iter()
        .find(|place| place.kind == StructuralPlaceKind::Parameter { position })
        .unwrap_or_else(|| panic!("no structural formal at {position}"))
        .id
}

fn steps(program: &Program, path: &[StructuralPathSegment]) -> Vec<String> {
    path.iter()
        .map(|segment| match segment {
            StructuralPathSegment::Field(field) => program
                .elaboration()
                .module
                .identities
                .text(*field)
                .unwrap()
                .to_owned(),
            StructuralPathSegment::Referent => "*".to_owned(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The one origin `machine` publishes for its returned reference: the
/// formal, the path from it and the access.
fn returned_origin(
    program: &Program,
    machine: &TerminalMachine,
) -> (PlaceId, Vec<String>, StructuralAccess) {
    let TerminalMachineResult::Structural(result) = &machine.contract.result else {
        panic!(
            "the machine returns a reference: {:?}",
            machine.contract.result
        )
    };
    let [returned] = result.reference_sources.as_slice() else {
        panic!("one returned reference: {:?}", result.reference_sources)
    };
    let [StructuralReferenceOrigin::Parameter(origin)] = returned.sources.as_slice() else {
        panic!("one input origin: {:?}", returned.sources)
    };
    let path: Vec<_> = origin
        .path
        .iter()
        .map(|projection| match projection {
            ReferenceOriginProjection::Exact(segment) => segment.clone(),
            other => panic!("an exact path: {other:?}"),
        })
        .collect();
    (origin.place, steps(program, &path), origin.access)
}

const CELL: &str = "data Cell { value: u8; }";

/// The checker recomputes a returned reference's origin from the borrow
/// the body returns and refuses a published origin that differs.
const REFERENCE_ORIGIN: &str = "reference result origins do not match its actual returned backing";

/// Each reference a record holds must be released or moved by name where
/// the record dies.
const DEAD_REFERENCE: &str = "a dead stored reference has no explicit last-use disposition";

/// A formed view must match the loan row stage 05 recorded for it.
const LOAN_ROW: &str = "reference formation has no exact Loan occurrence row";

/// `@a.value` borrows that field of formal `a` and the result publishes it
/// as its origin; the twin `@b.value` names `b`. Publishing `b` as the
/// origin of a result that borrows `a` is refused.
#[test]
fn a_returned_borrow_names_the_formal_and_field_it_borrows() {
    let mut lowered = Vec::new();
    for (position, name) in [(0, "a"), (1, "b")] {
        let program = Program::of(&format!(
            "{CELL} machine select<'r>(a: @'r Cell, b: @'r Cell) -> @'r u8 {{ @{name}.value }}"
        ));
        let select = program.machine("select");
        let borrow = the_operation(select, |kind| {
            matches!(kind, OperationKind::EstablishReference { .. })
        });
        let OperationKind::EstablishReference { source, .. } = &borrow.kind else {
            unreachable!()
        };
        assert_eq!(
            (source.place, steps(&program, &source.path), source.access),
            (
                formal(select, position),
                vec!["value".to_owned()],
                StructuralAccess::SharedBorrow
            )
        );
        assert_eq!(
            returned_origin(&program, select),
            (
                formal(select, position),
                vec!["value".to_owned()],
                StructuralAccess::SharedBorrow
            )
        );
        program.judged(|_| {}).accepted();
        lowered.push(program);
    }
    let b = formal(lowered[0].machine("select"), 1);
    let refusal = lowered[0]
        .judged_in("select", |machine| {
            let TerminalMachineResult::Structural(result) = &mut machine.contract.result else {
                unreachable!()
            };
            let StructuralReferenceOrigin::Parameter(origin) =
                &mut result.reference_sources[0].sources[0]
            else {
                unreachable!()
            };
            origin.place = b;
        })
        .refused();
    assert!(refusal.detail.contains(REFERENCE_ORIGIN), "{refusal:?}");
}

const NODE: &str = "data Leaf { v: u8; } \
    data Node<'a> { next: @'a mutable Leaf; other: @'a mutable Leaf; }";

/// Returning `x.next` from an owned `Node` moves the stored exclusive
/// reference out of that field, releases the sibling `other`, and the
/// result names `x.next`'s referent as its origin. The twin `x.other`
/// swaps the moved and the released field.
#[test]
fn returning_a_stored_exclusive_reference_moves_it_and_releases_the_sibling() {
    for (returned, released) in [("next", "other"), ("other", "next")] {
        let program = Program::of(&format!(
            "{NODE} machine read<'a>(x: Node<'a>) -> @'a mutable Leaf {{ x.{returned} }}"
        ));
        let read = program.machine("read");
        let x = formal(read, 0);
        let moved = the_operation(read, |kind| {
            matches!(kind, OperationKind::MoveStructuralValue { .. })
        });
        let OperationKind::MoveStructuralValue { source } = &moved.kind else {
            unreachable!()
        };
        assert_eq!(
            (source.place, steps(&program, &source.path)),
            (x, vec![returned.to_owned()])
        );
        let release = the_operation(read, |kind| {
            matches!(kind, OperationKind::ReleaseReference { .. })
        });
        let OperationKind::ReleaseReference { source, path } = &release.kind else {
            unreachable!()
        };
        assert_eq!(
            (*source, steps(&program, path)),
            (x, vec![released.to_owned()])
        );
        assert_eq!(
            returned_origin(&program, read),
            (
                x,
                vec![returned.to_owned(), "*".to_owned()],
                StructuralAccess::MutableBorrow
            )
        );
        program.judged(|_| {}).accepted();

        // The lowered machine releases the field it returned instead.
        let (id, moved_path) = (release.id, source_path(&moved.kind));
        let refusal = program
            .judged_in("read", |machine| {
                let OperationKind::ReleaseReference { path, .. } =
                    &mut operation_mut(machine, id).kind
                else {
                    unreachable!()
                };
                *path = moved_path.clone();
            })
            .refused();
        assert!(refusal.detail.contains(DEAD_REFERENCE), "{refusal:?}");
    }
}

fn source_path(kind: &OperationKind) -> Vec<StructuralPathSegment> {
    let OperationKind::MoveStructuralValue { source } = kind else {
        unreachable!()
    };
    source.path.clone()
}

/// An owned record that dies holding two references releases each one at
/// its own field, whether the reference is exclusive or shared, and runs no
/// cleanup of the referents. Leaving one release out is refused.
#[test]
fn a_dying_record_releases_each_reference_it_holds() {
    for first in ["@'a mutable Token", "@'a Token"] {
        let program = Program::of(&format!(
            "data Token {{ v: u8; }} data Holder<'a> {{ r: {first}; s: @'a Token; }} \
             machine finish<'a>(h: Holder<'a>) {{}}"
        ));
        let finish = program.machine("finish");
        let h = formal(finish, 0);
        let released: Vec<_> = finish
            .operations()
            .filter_map(|operation| match &operation.kind {
                OperationKind::ReleaseReference { source, path } => {
                    Some((operation.id, *source, steps(&program, path)))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            released
                .iter()
                .map(|(_, place, path)| (*place, path.clone()))
                .collect::<Vec<_>>(),
            [(h, vec!["r".to_owned()]), (h, vec!["s".to_owned()])],
            "{first}"
        );
        program.judged(|_| {}).accepted();

        let dropped = released[0].0;
        let refusal = program
            .judged_in("finish", |machine| {
                for block in &mut machine.blocks {
                    block.operations.retain(|operation| operation.id != dropped);
                }
            })
            .refused();
        assert!(refusal.detail.contains(DEAD_REFERENCE), "{refusal:?}");
    }
}

/// `let chosen = left;` on `left: @mutable [u32]` forms a new exclusive
/// view of `left` for the local, and `chosen.length` measures the local;
/// the twin `let chosen = right;` forms it from `right`. Forming the
/// local's view from the other formal is refused.
#[test]
fn a_local_exclusive_view_is_formed_from_the_formal_it_names() {
    for (position, name) in [(0, "left"), (1, "right")] {
        let program = Program::of(&format!(
            "machine pick(left: @mutable [u32], right: @mutable [u32]) -> u64 \
             {{ let chosen: @mutable [u32] = {name}; chosen.length }}"
        ));
        let pick = program.machine("pick");
        let view = the_operation(pick, |kind| {
            matches!(kind, OperationKind::EstablishMutableElementView { .. })
        });
        let OperationKind::EstablishMutableElementView { source, .. } = &view.kind else {
            unreachable!()
        };
        assert_eq!(
            (source.place, source.access),
            (formal(pick, position), StructuralAccess::MutableBorrow)
        );
        program.judged(|_| {}).accepted();

        let other = formal(pick, 1 - position);
        let id = view.id;
        let refusal = program
            .judged_in("pick", |machine| {
                let OperationKind::EstablishMutableElementView { source, .. } =
                    &mut operation_mut(machine, id).kind
                else {
                    unreachable!()
                };
                source.place = other;
            })
            .refused();
        assert!(refusal.detail.contains(LOAN_ROW), "{refusal:?}");
    }
}

/// The formal each arm of `match flag` hands to the join, by the Boolean
/// that selects the arm: the arm's block forms a view of that formal (or
/// copies it, for a shared view) and passes it to the join on its jump.
fn arrivals(machine: &TerminalMachine) -> Vec<(bool, PlaceId, StructuralAccess)> {
    use terminal_psi::Terminator;
    let Some(Terminator::Conditional {
        when_true,
        when_false,
        ..
    }) = machine
        .blocks
        .iter()
        .map(|block| &block.terminator)
        .find(|terminator| matches!(terminator, Terminator::Conditional { .. }))
    else {
        panic!("the match tests `flag`")
    };
    [(true, when_true.target), (false, when_false.target)]
        .into_iter()
        .map(|(taken, target)| {
            let arm = machine
                .blocks
                .iter()
                .find(|block| block.id == target)
                .unwrap();
            let Terminator::Jump {
                structural_arguments,
                ..
            } = &arm.terminator
            else {
                panic!("the arm jumps to the join")
            };
            let [passed] = structural_arguments.as_slice() else {
                panic!("the arm passes one view: {structural_arguments:?}")
            };
            let formed = arm
                .operations
                .iter()
                .find_map(|operation| match &operation.kind {
                    OperationKind::EstablishMutableElementView { source, .. }
                        if operation.result.as_structural().map(|result| result.place)
                            == Some(passed.place) =>
                    {
                        Some((source.place, source.access))
                    }
                    OperationKind::StructuralLeafCopy { source, .. }
                        if operation.result.as_structural().map(|result| result.place)
                            == Some(passed.place) =>
                    {
                        Some((*source, StructuralAccess::SharedBorrow))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("the arm forms the view it passes: {arm:?}"));
            (taken, formed.0, formed.1)
        })
        .collect()
}

/// `let chosen: @'a mutable [u32] = match flag { true -> left, false ->
/// right, };` forms an exclusive view of `left` on the `true` arm and of
/// `right` on the `false` arm, and each arm hands its own view to the join;
/// swapping the arms swaps the formals. A shared `@'a [u32]` join copies
/// each arm's own formal the same way. Forming the `true` arm's view from
/// `right` in the lowered program is refused.
#[test]
fn each_arm_of_a_reference_join_hands_the_join_its_own_formal() {
    for (access, view) in [
        (StructuralAccess::MutableBorrow, "@'a mutable [u32]"),
        (StructuralAccess::SharedBorrow, "@'a [u32]"),
    ] {
        for (on_true, on_false) in [("left", "right"), ("right", "left")] {
            let program = Program::of(&format!(
                "machine pick<'a>(flag: bool, left: {view}, right: {view}) -> u64 {{ \
                 let chosen: {view} = match flag {{ true -> {on_true}, false -> {on_false}, }}; \
                 chosen.length }}"
            ));
            let pick = program.machine("pick");
            let place = |name: &str| formal(pick, if name == "left" { 0 } else { 1 });
            assert_eq!(
                arrivals(pick),
                [
                    (true, place(on_true), access),
                    (false, place(on_false), access)
                ],
                "{view}: true -> {on_true}"
            );
            program.judged(|_| {}).accepted();
        }
    }
    let program = Program::of(
        "machine pick<'a>(flag: bool, left: @'a mutable [u32], right: @'a mutable [u32]) \
         -> u64 { let chosen: @'a mutable [u32] = match flag { true -> left, false -> right, }; \
         chosen.length }",
    );
    let pick = program.machine("pick");
    let right = formal(pick, 1);
    let left_view = pick
        .operations()
        .find(|operation| {
            matches!(&operation.kind,
                OperationKind::EstablishMutableElementView { source, .. }
                    if source.place == formal(pick, 0))
        })
        .unwrap()
        .id;
    let refusal = program
        .judged_in("pick", |machine| {
            let OperationKind::EstablishMutableElementView { source, .. } =
                &mut operation_mut(machine, left_view).kind
            else {
                unreachable!()
            };
            source.place = right;
        })
        .refused();
    assert!(refusal.detail.contains(LOAN_ROW), "{refusal:?}");
}

#[test]
fn an_indexed_borrow_only_holds_the_owner_through_its_last_use() {
    let source = r#"
        data Item { value: i32; }
        machine clear_one(cell: @mutable Item) terminates { cell.value = 1; }
        machine main() crashes Trap {
            let mutable items: [Item; 2] = [Item { value: 0 }, Item { value: 0 }];
            let view: @mutable [Item] = @mutable items[..];
            clear_one(@mutable view[0]);
            clear_one(@mutable items[0]);
        }
    "#;
    Program::of(source).judged(|_| {}).accepted();

    let retained = source.replace(
        "clear_one(@mutable items[0]);",
        "clear_one(@mutable items[0]); clear_one(@mutable view[0]);",
    );
    match Program::of(&retained).judged(|_| {}) {
        program::Verdict::Refused(refusal) => assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Conflict
        ),
        program::Verdict::Rejected(rejection) => assert!(!rejection.conflicts.is_empty()),
        other => panic!("expected overlapping exclusive loans, got {other:?}"),
    }
}

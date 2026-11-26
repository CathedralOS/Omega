//! Stage 05 must write each move, store and indexed read at the exact place
//! the source names, and the lowered-form checker must judge the program
//! from those places. Each test lowers a program and its twin that names a
//! sibling instead (another field, another element, another formal), and
//! asserts the place stage 05 wrote. Then it changes only that place in the
//! lowered program, as a faulty stage 05 would, and requires the checker's
//! verdict to follow.
//!
//! The spec rules are in `wiki/spec/language/ownership.md`: "Partial moves
//! retain unselected siblings" ("Paths and control flow"); an owned move out
//! of `@mutable` storage opens a window that "Assignment of a valid owned
//! replacement to the exact vacant place closes", and "There is no old
//! field value to dispose at that store" ("Borrowed-storage invariant
//! windows"). An element read needs "an observation of the same
//! descriptor's length immediately before the read" and "checked `index <
//! length`" (`wiki/spec/terminal-psi/byte_views.md`, `ElementViewRead`).

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, Verdict, operation_mut, the_operation};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, ValueId};
use terminal_check_tables::RefusalReason;
use terminal_psi::{
    OperationKind, StructuralPathSegment, StructuralStoreDisposition, TerminalAffineCleanupAction,
    TerminalMachine, Terminator,
};

const PAIR: &str = "data Token { value: u8; } data Pair { left: Token; right: Token; }";

/// The place of the machine's formal at `position` in its parameter list.
fn formal(machine: &TerminalMachine, position: u32) -> PlaceId {
    machine
        .structural_places
        .iter()
        .find(|place| place.kind == StructuralPlaceKind::Parameter { position })
        .unwrap_or_else(|| panic!("no structural formal at {position}"))
        .id
}

/// The value of the machine's scalar formal at `position` among its scalar
/// formals.
fn scalar_formal(machine: &TerminalMachine, position: usize) -> ValueId {
    machine.contract.parameters[position].id
}

/// The field names a path steps through, with `[n]` for a fixed index.
fn steps(program: &Program, path: &[StructuralPathSegment]) -> Vec<String> {
    path.iter()
        .map(|segment| match segment {
            StructuralPathSegment::Field(field) => program
                .elaboration()
                .module
                .identities
                .text(*field)
                .unwrap_or_else(|| panic!("unnamed field {field:?}"))
                .to_owned(),
            StructuralPathSegment::FixedIndex(index) => format!("[{index}]"),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The one return of `machine` and the cleanup it runs.
fn return_cleanup(machine: &TerminalMachine) -> &[TerminalAffineCleanupAction] {
    let mut returns = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::ReturnStructural {
                cleanup_actions, ..
            }
            | Terminator::Return {
                cleanup_actions, ..
            }
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.as_slice()),
            _ => None,
        });
    let cleanup = returns.next().expect("the machine returns");
    assert!(returns.next().is_none(), "the machine returns once");
    cleanup
}

fn return_cleanup_mut(machine: &mut TerminalMachine) -> &mut Vec<TerminalAffineCleanupAction> {
    machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::ReturnStructural {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        })
        .expect("the machine returns a structural value")
}

/// The residuals the cleanup discards, by the place and the path to each.
fn residuals(
    program: &Program,
    cleanup: &[TerminalAffineCleanupAction],
) -> Vec<(PlaceId, Vec<String>)> {
    cleanup
        .iter()
        .filter_map(|action| match action {
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                Some((discard.place, steps(program, &discard.path)))
            }
            _ => None,
        })
        .collect()
}

/// The checker recomputes the disposal each edge owes from the moves and
/// stores before it, and refuses a lowered program whose cleanup differs.
const SCHEDULE: &str = "normal edge affine disposal differs from its canonical schedule";

/// A store that claims to replace a live value at a place the program has
/// already emptied.
const VACANT_AS_LIVE: &str = "an occupied replacement store targets vacated affine contents";

fn refused_because(verdict: Verdict) -> String {
    let refusal = verdict.refused();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    refusal.detail
}

/// `move p.left` transfers that field of the formal and the return
/// disposes only `p.right`; the twin `move p.right` swaps both. A lowered
/// program whose return disposes the moved field instead is refused.
#[test]
fn moving_one_field_returns_it_and_disposes_only_the_other() {
    for (moved, other) in [("left", "right"), ("right", "left")] {
        let program = Program::of(&format!(
            "{PAIR} machine pick(p: Pair) -> Token {{ move p.{moved} }}"
        ));
        let pick = program.machine("pick");
        let p = formal(pick, 0);
        let OperationKind::MoveStructuralValue { source } = &the_operation(pick, |kind| {
            matches!(kind, OperationKind::MoveStructuralValue { .. })
        })
        .kind
        else {
            unreachable!()
        };
        assert_eq!(
            (source.place, steps(&program, &source.path)),
            (p, vec![moved.to_owned()])
        );
        let cleanup = return_cleanup(pick);
        assert_eq!(residuals(&program, cleanup), [(p, vec![other.to_owned()])]);
        assert!(
            cleanup.contains(&TerminalAffineCleanupAction::RetireHome(p)),
            "{cleanup:?}"
        );
        program.judged(|_| {}).accepted();

        // The return disposes the field that was moved out, and leaves the
        // remaining field undisposed.
        let moved_path = source.path.clone();
        let detail = refused_because(program.judged_in("pick", |machine| {
            for action in return_cleanup_mut(machine) {
                if let TerminalAffineCleanupAction::DiscardResidual(discard) = action {
                    discard.path = moved_path.clone();
                }
            }
        }));
        assert!(detail.contains(SCHEDULE), "{detail}");
    }
}

const CELLS: &str = "data Cell { v: u8; } data Pair { left: Cell; right: Cell; }";

/// `let old = move p.left;` through `p: @mutable Pair` moves that field out
/// of borrowed storage, and `p.left = move old;` stores into the vacancy it
/// left: one place, one path, disposition `Vacant`. Storing into `p.right`
/// instead leaves `p.left` empty at the return, and the checker rejects the
/// program. In the lowered program, pointing only the store at `right` is
/// refused because that field still holds its value, and so is calling the
/// vacancy live.
#[test]
fn a_field_moved_out_of_borrowed_storage_is_stored_back_at_the_same_path() {
    let restored = Program::of(&format!(
        "{CELLS} machine edit(p: @mutable Pair) {{ let old: Cell = move p.left; p.left = move old; }}"
    ));
    let edit = restored.machine("edit");
    let p = formal(edit, 0);
    let OperationKind::MoveStructuralField { source, path, .. } = &the_operation(edit, |kind| {
        matches!(kind, OperationKind::MoveStructuralField { .. })
    })
    .kind
    else {
        unreachable!()
    };
    assert_eq!(
        (*source, steps(&restored, path)),
        (p, vec!["left".to_owned()])
    );
    let into_p: Vec<_> = edit
        .operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::StoreStructuralField {
                disposition,
                destination,
                path,
                ..
            } if *destination == p => Some((operation.id, *disposition, steps(&restored, path))),
            _ => None,
        })
        .collect();
    let [(store, disposition, store_path)] = into_p.as_slice() else {
        panic!("one store into `p`: {into_p:?}")
    };
    assert_eq!(
        (*disposition, store_path.as_slice()),
        (
            StructuralStoreDisposition::Vacant,
            ["left".to_owned()].as_slice()
        )
    );
    restored.judged(|_| {}).accepted();

    const NOT_RESTORED: &str =
        "the place does not hold a value again where the machine returns the borrow";
    let sibling = Program::of(&format!(
        "{CELLS} machine edit(p: @mutable Pair) {{ let old: Cell = move p.left; p.right = move old; }}"
    ));
    let rejection = sibling.judged(|_| {}).rejected();
    assert!(
        rejection
            .unestablished
            .iter()
            .any(|detail| detail.contains(NOT_RESTORED)),
        "{rejection:?}"
    );

    let right = sibling
        .machine("edit")
        .operations()
        .find_map(|operation| match &operation.kind {
            OperationKind::StoreStructuralField { path, .. } if !path.is_empty() => {
                Some(path.clone())
            }
            _ => None,
        });
    let right = right.expect("the twin stores into `p.right`");
    let store = *store;
    let refusal = restored
        .judged_in("edit", |machine| {
            let OperationKind::StoreStructuralField { path, .. } =
                &mut operation_mut(machine, store).kind
            else {
                unreachable!()
            };
            *path = right.clone();
        })
        .refused();
    assert!(
        refusal
            .detail
            .contains("a vacant store displaces established affine contents"),
        "{refusal:?}"
    );

    let detail = refused_because(restored.judged_in("edit", |machine| {
        let OperationKind::StoreStructuralField { disposition, .. } =
            &mut operation_mut(machine, store).kind
        else {
            unreachable!()
        };
        *disposition = StructuralStoreDisposition::ReplaceDiscardable;
    }));
    assert!(detail.contains(VACANT_AS_LIVE), "{detail}");
}

/// After `let old = move q.left;`, storing into `q.left` fills the vacancy
/// (`Vacant`), and storing into `q.right` replaces a live value
/// (`ReplaceDiscardable`) whose old value is disposed. A lowered program that
/// calls the vacant store a replacement is refused.
#[test]
fn a_store_fills_the_vacant_field_and_replaces_the_live_sibling() {
    let replace = |destination: &str| {
        Program::of(&format!(
            "{PAIR} machine replace(p: Pair, new: Token) -> Token {{ \
             let mutable q: Pair = move p; let old: Token = move q.left; q.{destination} = move new; old }}"
        ))
    };
    let mut vacant_store = None;
    for (destination, expected) in [
        ("left", StructuralStoreDisposition::Vacant),
        ("right", StructuralStoreDisposition::ReplaceDiscardable),
    ] {
        let program = replace(destination);
        let machine = program.machine("replace");
        let stores: Vec<_> = machine
            .operations()
            .filter_map(|operation| match &operation.kind {
                OperationKind::StoreStructuralField {
                    disposition, path, ..
                } if !path.is_empty() => Some((operation.id, *disposition, steps(&program, path))),
                _ => None,
            })
            .collect();
        let [(store, disposition, path)] = stores.as_slice() else {
            panic!("one field store: {stores:?}")
        };
        assert_eq!(
            (*disposition, path.as_slice()),
            (expected, [destination.to_owned()].as_slice())
        );
        program.judged(|_| {}).accepted();
        if expected == StructuralStoreDisposition::Vacant {
            vacant_store = Some((program, *store));
        }
    }
    let (program, store) = vacant_store.unwrap();
    let detail = refused_because(program.judged_in("replace", |machine| {
        let OperationKind::StoreStructuralField { disposition, .. } =
            &mut operation_mut(machine, store).kind
        else {
            unreachable!()
        };
        *disposition = StructuralStoreDisposition::ReplaceDiscardable;
    }));
    assert!(detail.contains(VACANT_AS_LIVE), "{detail}");
}

/// `move items[0]` returns element 0 and disposes only element 1; the twin
/// `move items[1]` swaps both. A return that disposes the moved element is
/// refused.
#[test]
fn moving_one_array_element_disposes_only_the_other() {
    for (moved, other) in [(0, 1), (1, 0)] {
        let program = Program::of(&format!(
            "data Token {{ v: u8; }} machine pick(items: [Token; 2]) -> Token {{ move items[{moved}] }}"
        ));
        let pick = program.machine("pick");
        let items = formal(pick, 0);
        let OperationKind::MoveStructuralValue { source } = &the_operation(pick, |kind| {
            matches!(kind, OperationKind::MoveStructuralValue { .. })
        })
        .kind
        else {
            unreachable!()
        };
        assert_eq!(
            (source.place, steps(&program, &source.path)),
            (items, vec![format!("[{moved}]")])
        );
        assert_eq!(
            residuals(&program, return_cleanup(pick)),
            [(items, vec![format!("[{other}]")])]
        );
        program.judged(|_| {}).accepted();
        let moved_path = source.path.clone();
        let detail = refused_because(program.judged_in("pick", |machine| {
            for action in return_cleanup_mut(machine) {
                if let TerminalAffineCleanupAction::DiscardResidual(discard) = action {
                    discard.path = moved_path.clone();
                }
            }
        }));
        assert!(detail.contains(SCHEDULE), "{detail}");
    }
}

/// The checker matches each read against the projection the lowered
/// program's place table records for it, so a read whose selector or view
/// was changed alone is refused before any bound is judged.
const UNHELD_PROJECTION: &str = "a read names a projection the place table does not hold";

/// `cells[left].v` reads the element its selector `left` chooses, with the
/// bound obligation at that selector. Only `left` is bounded by the
/// requirement, so `cells[right].v` is rejected. Changing only the lowered
/// read's selector from `left` to `right` is refused.
#[test]
fn an_indexed_field_read_uses_its_own_selector_and_bound() {
    let read = |selector: &str| {
        Program::of(&format!(
            "data Cell [copy] {{ v: u8; w: u8; }} \
             machine read(cells: [Cell; 4], left: u64, right: u64) -> u8 \
             requires left < 4 {{ cells[{selector}].v }}"
        ))
    };
    let program = read("left");
    let machine = program.machine("read");
    let (left, right) = (scalar_formal(machine, 0), scalar_formal(machine, 1));
    let operation = the_operation(machine, |kind| {
        matches!(kind, OperationKind::PrimitiveScalarRead { .. })
    });
    let OperationKind::PrimitiveScalarRead { source, path } = &operation.kind else {
        unreachable!()
    };
    let [
        StructuralPathSegment::RuntimeIndex { index, .. },
        StructuralPathSegment::Field(field),
    ] = path.as_slice()
    else {
        panic!("one selector then one field: {path:?}")
    };
    assert_eq!(*source, formal(machine, 0));
    assert_eq!(*index, left);
    assert_eq!(
        program.elaboration().module.identities.text(*field),
        Some("v")
    );
    program.judged(|_| {}).accepted();

    let rejection = read("right").judged(|_| {}).rejected();
    assert!(
        rejection
            .unproved
            .iter()
            .any(|kind| kind.starts_with("OperationSafety")),
        "{rejection:?}"
    );

    let id = operation.id;
    let refusal = program
        .judged_in("read", |machine| {
            let OperationKind::PrimitiveScalarRead { path, .. } =
                &mut operation_mut(machine, id).kind
            else {
                unreachable!()
            };
            let StructuralPathSegment::RuntimeIndex { index, .. } = &mut path[0] else {
                unreachable!()
            };
            *index = right;
        })
        .refused();
    assert!(refusal.detail.contains(UNHELD_PROJECTION), "{refusal:?}");
}

/// `left[i]` on `left: @mutable [u32]` measures `left` and reads `left` with
/// that length and index `i`. The requirement bounds `i` by `left.length`
/// only, so the twin `right[i]` is rejected. Changing only the lowered
/// measurement and read to `right` is refused.
#[test]
fn an_element_view_read_measures_and_reads_its_own_view() {
    let get = |view: &str| {
        Program::of(&format!(
            "machine get(left: @mutable [u32], right: @mutable [u32], i: u64) -> u32 \
             requires i < left.length {{ {view}[i] }}"
        ))
    };
    let program = get("left");
    let machine = program.machine("get");
    let (left, right) = (formal(machine, 0), formal(machine, 1));
    let length = the_operation(machine, |kind| {
        matches!(kind, OperationKind::ElementViewLength { .. })
    });
    let OperationKind::ElementViewLength { source } = &length.kind else {
        unreachable!()
    };
    assert_eq!(source.place, left);
    let read = the_operation(machine, |kind| {
        matches!(kind, OperationKind::ElementViewRead { .. })
    });
    let OperationKind::ElementViewRead {
        source,
        index,
        length: measured,
        ..
    } = &read.kind
    else {
        unreachable!()
    };
    assert_eq!(source.place, left);
    assert_eq!(*index, scalar_formal(machine, 0));
    assert_eq!(
        Some(*measured),
        length.result.scalar().map(|value| value.id)
    );
    program.judged(|_| {}).accepted();

    let rejection = get("right").judged(|_| {}).rejected();
    assert!(
        rejection
            .unproved
            .iter()
            .any(|kind| kind.starts_with("OperationSafety")),
        "{rejection:?}"
    );

    let (length, read) = (length.id, read.id);
    let verdict = program.judged_in("get", |machine| {
        for id in [length, read] {
            match &mut operation_mut(machine, id).kind {
                OperationKind::ElementViewLength { source }
                | OperationKind::ElementViewRead { source, .. } => source.place = right,
                _ => unreachable!(),
            }
        }
    });
    let refusal = verdict.refused();
    assert!(refusal.detail.contains(UNHELD_PROJECTION), "{refusal:?}");
}

//! Restoration windows at a suspension
//! (`wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration`).
//!
//! `move p.t` through `p: @mut Cell` vacates `p.t` and leaves restoration debt
//! there. When the call parks before `p.t = move old`, the crossing must name
//! that open window, keep the displaced value in caller custody and keep the
//! exclusive lender, because resuming cannot repeat the move. When the store
//! closes the window before the call, the crossing names no window. The
//! checker judges both; these tests compare what Psi publishes with an
//! independent Psi-only reader.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{CallSite, OperationKind, OwnedCustody, PlaceProjection, ProjectedPlace, ReferenceAccess, StructuralStoreDisposition};
use psi_handoff_consumer::{ParkedWindow, StoreKind, parked_windows, window_stores};

const OPEN: &str = include_str!("fixtures/package_cases/open-restoration-window-at-suspension/case/case.omg");
const OPEN_CONTROL: &str = include_str!("fixtures/package_cases/open-restoration-window-at-suspension/control/case.omg");
const CLOSED: &str = include_str!("fixtures/package_cases/suspension-after-a-completed-restoration/case/case.omg");
const CLOSED_CONTROL: &str = include_str!("fixtures/package_cases/suspension-after-a-completed-restoration/control/case.omg");

fn machine<'a>(
    package: &'a omega_lowering::LoweredPackage,
    name: &str,
) -> &'a omega_lowering::LoweredMachine {
    package.machines.iter().find(|m| m.declaration.path == name).unwrap()
}

/// The `BorrowedMove` of `p.t` and the place it vacated.
fn the_move(package: &omega_lowering::LoweredPackage) -> (omega_lowering::OperationId, ProjectedPlace) {
    let body = &machine(package, "f").body;
    let mut moves = body.operations.iter().filter_map(|op| match &op.kind {
        OperationKind::BorrowedMove { source } => Some((op.id, source.clone())),
        _ => None,
    });
    let moved = moves.next().expect("one borrowed move");
    assert!(moves.next().is_none(), "one borrowed move");
    moved
}

#[test]
fn a_suspension_inside_an_open_window_names_the_window_and_its_custody() {
    let pkg = source::lower(&[("case.omg", OPEN)]).unwrap();
    let body = &machine(&pkg, "f").body;
    let omega_lowering::Parameter::Structural(p) = body.parameters[0] else {
        panic!("p is a structural formal")
    };
    let (opened, place) = the_move(&pkg);
    let omega_lowering::OperationResult::Structural(old) = body.operations[opened.index()].result else {
        panic!("the move produces the displaced value")
    };
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    let CallSite::Operation(call) = site.call else {
        panic!("an ordinary call parks")
    };
    let restore = body
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::StructuralStore { .. }))
        .unwrap();
    // `p.t = move old` stores the value that `move old` transferred out of `old`.
    let OperationKind::StructuralStore { value: restored, .. } = restore.kind else { unreachable!() };
    let omega_lowering::PlaceOrigin::Operation(transfer) = body.places[restored.index()].origin else {
        panic!("the restored value is a completed transfer")
    };
    spec_corpus::observe_handoff!(psi "psi-output/open-restoration-window-at-suspension", assert_eq!(
        (
            (place.root == p, place.path.as_slice().len(), matches!(place.path[0], PlaceProjection::Referent)),
            (opened.index() < call.index(), call.index() < transfer.index(), transfer.index() < restore.id.index()),
            (&body.operations[transfer.index()].kind, &restore.kind),
            parked_windows(&pkg, "f"),
            site.retained_loans.iter().map(|l| (l.descriptor, l.access, l.needed_by_callee, l.needed_after_completion)).collect::<Vec<_>>(),
        ),
        (
            (true, 2, true),
            (true, true, true),
            (
                &OperationKind::MoveStructural {
                    source: ProjectedPlace { root: old, path: vec![] },
                },
                &OperationKind::StructuralStore {
                    destination: place.clone(),
                    value: restored,
                    disposition: StructuralStoreDisposition::FillVacancy,
                },
            ),
            Ok(vec![(
                site.call,
                vec![ParkedWindow {
                    opened,
                    place: place.clone(),
                    displaced: old,
                    displaced_custody: Some(OwnedCustody::Caller),
                    lender: Some(ReferenceAccess::Mutable),
                }]
            )]),
            vec![(p, ReferenceAccess::Mutable, false, true)],
        )
    ));
    // The displaced value's owner row has nothing moved out of it.
    let owner = site.retained_owners.iter().find(|o| o.place == old).unwrap();
    assert!(owner.moved.is_empty());

    // The suspend-only control parks with no window and no retained lender.
    let control = source::lower(&[("case.omg", OPEN_CONTROL)]).unwrap();
    let [site] = machine(&control, "f").body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    assert_eq!(parked_windows(&control, "f"), Ok(vec![(site.call, vec![])]));
    assert!(site.retained_loans.is_empty() && site.retained_owners.is_empty());
}

#[test]
fn a_suspension_after_the_restoring_store_names_no_window() {
    let pkg = source::lower(&[("case.omg", CLOSED)]).unwrap();
    let body = &machine(&pkg, "f").body;
    let (opened, place) = the_move(&pkg);
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    let CallSite::Operation(call) = site.call else {
        panic!("an ordinary call parks")
    };
    let stores = window_stores(&pkg, "f").unwrap();
    let [(store, stored, kind)] = stores.as_slice() else {
        panic!("one store")
    };
    let (store, kind) = (*store, *kind);
    spec_corpus::observe_handoff!(psi "psi-output/suspension-after-a-completed-restoration", assert_eq!(
        (
            (opened.index() < store.index(), store.index() < call.index()),
            (stored == &place, kind),
            parked_windows(&pkg, "f"),
            (site.retained_loans.is_empty(), site.retained_owners.is_empty()),
        ),
        ((true, true), (true, StoreKind::Fill), Ok(vec![(site.call, vec![])]), (true, true))
    ));
    let control = source::lower(&[("case.omg", CLOSED_CONTROL)]).unwrap();
    let [site] = machine(&control, "f").body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    assert_eq!(parked_windows(&control, "f"), Ok(vec![(site.call, vec![])]));
}

#[test]
fn displacing_an_occupied_field_before_the_crossing_leaves_no_window() {
    // Whole-value replacement through `@mut` displaces the old value rather
    // than filling a vacancy (ownership.md#borrowed-storage-restoration). The
    // displaced `T` is affine, so lowering extracts and disposes it, then fills
    // the place. Nothing is open at the call, and the store differs from the
    // restoration above only in what it displaced.
    let text = "data T{v:u8;} data Cell{t:T;} machine wait() suspends {} machine f(p:@mut Cell) suspends { p.t = T{v:1u8}; suspend wait(); }";
    let pkg = source::lower(&[("displace.omg", text)]).unwrap();
    let body = &machine(&pkg, "f").body;
    let (opened, place) = the_move(&pkg);
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    let stores = window_stores(&pkg, "f").unwrap();
    let [(_, stored, StoreKind::Fill)] = stores.as_slice() else {
        panic!("the extraction is filled once: {stores:?}")
    };
    assert_eq!(stored, &place);
    assert_eq!(parked_windows(&pkg, "f"), Ok(vec![(site.call, vec![])]));
    // The displaced value is disposed before the call, not carried across it.
    assert!(site.retained_owners.is_empty(), "{:?}", site.retained_owners);
    assert!(opened.index() < body.operations.len());
}

#[test]
fn the_reader_rejects_a_missing_redirected_or_invented_window() {
    let mut open = source::lower(&[("case.omg", OPEN)]).unwrap();
    let (_, place) = the_move(&open);
    let f = open.machines.iter_mut().find(|m| m.declaration.path == "f").unwrap();
    let window = f.body.suspension_sites[0].open_windows.pop().unwrap();
    assert_eq!(parked_windows(&open, "f"), Err("the published windows disagree with the operations before the call"));
    let f = open.machines.iter_mut().find(|m| m.declaration.path == "f").unwrap();
    let mut redirected = window.clone();
    redirected.place = ProjectedPlace {
        root: place.root,
        path: vec![PlaceProjection::Referent],
    };
    f.body.suspension_sites[0].open_windows.push(redirected);
    assert_eq!(parked_windows(&open, "f"), Err("the published windows disagree with the operations before the call"));

    let mut closed = source::lower(&[("case.omg", CLOSED)]).unwrap();
    let f = closed.machines.iter_mut().find(|m| m.declaration.path == "f").unwrap();
    f.body.suspension_sites[0].open_windows.push(window);
    assert_eq!(parked_windows(&closed, "f"), Err("the published windows disagree with the operations before the call"));
}

#[test]
fn a_store_closes_only_the_window_at_its_own_captured_place() {
    // Two fields move out; restoring `p.t` before the call closes only that
    // window. The crossing names the `p.u` window alone, and the displaced `b`
    // stays in caller custody until its own store after completion.
    let text = "data T{v:u8;} data Cell{t:T; u:T;} machine wait() suspends {} machine f(p:@mut Cell) suspends { let a = move p.t; let b = move p.u; p.t = move a; suspend wait(); p.u = move b; }";
    let pkg = source::lower(&[("two.omg", text)]).unwrap();
    let body = &machine(&pkg, "f").body;
    let moves: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::BorrowedMove { source }, omega_lowering::OperationResult::Structural(value)) => Some((op.id, source.clone(), value)),
            _ => None,
        })
        .collect();
    let [_, (opened, place, displaced)] = moves.as_slice() else {
        panic!("two moves")
    };
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    assert_eq!(
        parked_windows(&pkg, "f"),
        Ok(vec![(
            site.call,
            vec![ParkedWindow {
                opened: *opened,
                place: place.clone(),
                displaced: *displaced,
                displaced_custody: Some(OwnedCustody::Caller),
                lender: Some(ReferenceAccess::Mutable),
            }]
        )])
    );
    assert!(matches!(place.path.last(), Some(PlaceProjection::Field(field)) if field.path == "Cell::u"));
}

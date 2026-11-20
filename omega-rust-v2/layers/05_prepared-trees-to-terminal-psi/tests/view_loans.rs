//! Whole-element windows through an exclusive element view `@mut [T]`.
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CleanupAction, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceProjection, ProjectedPlace, RuntimeIndex,
    StructuralStoreDisposition, TypeShape, ValueId,
};

const CELL: &str = "data Cell{v:u8;}";

/// The view-rooted element of one occurrence: its selector, bound obligation and length.
fn element(
    place: &ProjectedPlace,
    view: PlaceId,
) -> (RuntimeIndex, ValueId) {
    assert_eq!(place.root, view, "the window stays rooted at the view itself");
    let [
        PlaceProjection::Referent,
        PlaceProjection::ViewElement { index, length },
    ] = place.path.as_slice()
    else {
        panic!("expected one runtime element below the view's referent: {:?}", place.path);
    };
    (*index, *length)
}

fn lengths(
    body: &MachineBody,
    view: PlaceId,
) -> Vec<ValueId> {
    body.operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::ElementViewLength { source }, OperationResult::Scalar(length)) if *source == view => Some(length),
            _ => None,
        })
        .collect()
}

fn moves(body: &MachineBody) -> Vec<(&ProjectedPlace, OperationResult)> {
    body.operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::BorrowedMove { source } => Some((source, op.result)),
            _ => None,
        })
        .collect()
}

fn stores(body: &MachineBody) -> Vec<(&ProjectedPlace, PlaceId, &StructuralStoreDisposition)> {
    body.operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::StructuralStore {
                destination,
                value,
                disposition,
            } => Some((destination, *value, disposition)),
            _ => None,
        })
        .collect()
}

#[test]
fn moving_an_element_out_of_an_exclusive_view_and_restoring_it_fills_the_same_element() {
    let src = format!("{CELL} machine edit(p:@mut [Cell],i:u64) {{let old=move p[i]; p[i]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let machine = &pkg.machines[0];
    let body = &machine.body;
    let [Parameter::Structural(view), Parameter::Scalar(i)] = body.parameters[..] else {
        panic!("expected a view descriptor and a scalar selector");
    };
    let Some(&TypeShape::MutableElementView { element: cell }) = pkg.types.get(body.places[view.index()].ty) else {
        panic!("the exclusive view needs its nominal element carrier");
    };
    assert_eq!(pkg.data[cell.get() - 1].declaration.path, "Cell");
    assert_eq!(machine.parameter_types[0], body.places[view.index()].ty);
    // Each occurrence observes the view's own length once.
    let [moved_length, stored_length] = lengths(body, view)[..] else {
        panic!("expected one length observation per occurrence");
    };
    let [(taken, old)] = moves(body)[..] else {
        panic!("a vacancy repair extracts nothing else");
    };
    let (taken_index, taken_length) = element(taken, view);
    assert_eq!(taken_index.selector, i);
    assert_eq!(taken_length, moved_length);
    let [(written, value, disposition)] = stores(body)[..] else {
        panic!("one establishing store");
    };
    let (written_index, written_length) = element(written, view);
    assert_eq!(written_index.selector, i);
    assert_eq!(written_length, stored_length);
    // Bounds evidence belongs to each occurrence, never shared.
    assert_ne!(taken_index.obligation, written_index.obligation);
    assert_eq!(body.diagnostics.bounds.len(), 2);
    assert_eq!(*disposition, StructuralStoreDisposition::FillVacancy);
    // The stored value is the extracted element's own custody.
    let supplied = OperationResult::Structural(value);
    assert!(
        supplied == old
            || body.operations.iter().any(|op| op.result == supplied
                && matches!(&op.kind, OperationKind::MoveStructural { source } if source.path.is_empty() && OperationResult::Structural(source.root) == old))
    );
    // Filling the vacancy disposes nothing.
    for block in &body.blocks {
        match &block.terminator {
            BlockTerminator::Return { cleanup, .. } => assert!(cleanup.is_empty()),
            BlockTerminator::Jump(edge) => assert!(edge.cleanup.is_empty()),
            _ => {}
        }
    }
}

#[test]
fn equal_literal_selectors_repair_the_extracted_element() {
    let src = format!("{CELL} machine edit(p:@mut [Cell]) {{let old=move p[0]; p[0]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [Parameter::Structural(view)] = body.parameters[..] else {
        panic!("expected the view descriptor");
    };
    let [(taken, _)] = moves(body)[..] else {
        panic!("one extraction");
    };
    let [(written, _, disposition)] = stores(body)[..] else {
        panic!("one store");
    };
    // A literal selector still names a runtime element owed against the view's length.
    let (taken, _) = element(taken, view);
    let (written, _) = element(written, view);
    assert_ne!(taken.selector, written.selector, "each occurrence evaluates its own selector");
    assert_eq!(*disposition, StructuralStoreDisposition::FillVacancy);
}

#[test]
fn a_different_selector_never_repairs_the_extracted_element() {
    // Distinct literals are provably disjoint; an unrelated value is not proved equal.
    for (params, taken, written) in [
        (",i:u64,j:u64", "i", "j"),
        (",i:u64", "i", "0"),
        (",i:u64", "0", "i"),
    ] {
        let src = format!("{CELL} machine edit(p:@mut [Cell]{params}) {{let old=move p[{taken}]; p[{written}]=move old;}}");
        let error = sources::lower(&[("view.omg", &src)]).unwrap_err();
        assert_eq!(
            error.message, "element restoration needs the identical selector value of the extracted element",
            "{src}"
        );
    }
}

#[test]
fn replacing_an_occupied_element_displaces_and_disposes_the_old_value_at_the_same_occurrence() {
    let src = format!("{CELL} machine put(p:@mut [Cell],i:u64,c:Cell) {{p[i]=c;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let Parameter::Structural(view) = body.parameters[0] else {
        panic!("expected the view descriptor");
    };
    let [(displaced, old)] = moves(body)[..] else {
        panic!("the occupied element is extracted once");
    };
    let [(written, _, disposition)] = stores(body)[..] else {
        panic!("one store");
    };
    // Same occurrence: identical selector, obligation and length observation.
    assert_eq!(displaced, written);
    element(written, view);
    assert_eq!(lengths(body, view).len(), 1);
    assert_eq!(*disposition, StructuralStoreDisposition::FillVacancy);
    let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
        panic!("the displaced value needs a cleanup continuation");
    };
    assert!(matches!(edge.cleanup.as_slice(), [CleanupAction::DiscardRoot(place)] if OperationResult::Structural(*place) == old));
}

#[test]
fn swapping_between_same_typed_views_repairs_each_vacancy_on_its_own_root() {
    let src = format!("{CELL} machine swap(a:@mut [Cell],b:@mut [Cell],i:u64) {{let left=move a[i]; let right=move b[i]; a[i]=move right; b[i]=move left;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [
        Parameter::Structural(a),
        Parameter::Structural(b),
        Parameter::Scalar(i),
    ] = body.parameters[..]
    else {
        panic!("expected two views and a selector");
    };
    // Twins share one descriptor type but never one element place.
    assert_eq!(body.places[a.index()].ty, body.places[b.index()].ty);
    let [(from_a, _), (from_b, _)] = moves(body)[..] else {
        panic!("each view is extracted once");
    };
    let [(into_a, _, first), (into_b, _, second)] = stores(body)[..] else {
        panic!("each view is repaired once");
    };
    for (place, view) in [(from_a, a), (from_b, b), (into_a, a), (into_b, b)] {
        assert_eq!(element(place, view).0.selector, i);
    }
    assert_eq!(*first, StructuralStoreDisposition::FillVacancy);
    assert_eq!(*second, StructuralStoreDisposition::FillVacancy);
    assert_eq!(lengths(body, a).len(), 2);
    assert_eq!(lengths(body, b).len(), 2);
}

#[test]
fn recomputing_a_selector_does_not_establish_that_it_repairs_the_active_window() {
    let src = format!("{CELL} machine edit(p:@mut [Cell],i:u64) {{let old=move p[i+1]; p[i+1]=move old;}}");
    assert_eq!(
        sources::lower(&[("view.omg", &src)]).unwrap_err().message,
        "element restoration needs the identical selector value of the extracted element"
    );
}

#[test]
fn assignment_captures_the_selected_element_before_evaluating_its_replacement() {
    let src = format!(
        "{CELL} machine index(i:u64)->u64 {{i}} machine value(c:Cell)->Cell {{move c}} machine put(p:@mut [Cell],i:u64,c:Cell) {{p[index(i)]=value(move c);}}"
    );
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let machine = |name: &str| {
        pkg.machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let body = &machine("put").body;
    let Parameter::Structural(view) = body.parameters[0] else {
        panic!("expected an exclusive view")
    };
    let calls: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::Call {
                callee: omega_lowering::CallTarget::Machine(id),
                ..
            } => Some((*id, op.result)),
            _ => None,
        })
        .collect();
    let [
        (index, OperationResult::Scalar(selector)),
        (value, OperationResult::Structural(replacement)),
    ] = calls[..]
    else {
        panic!("expected the selector call then the value call, exactly once each, saw {calls:?}")
    };
    assert_eq!((index, value), (machine("index").id, machine("value").id));
    let [(written, supplied, _)] = stores(body)[..] else {
        panic!("expected one store")
    };
    assert_eq!(element(written, view).0.selector, selector);
    assert_eq!(supplied, replacement);
    assert_eq!(lengths(body, view).len(), 1);
    assert_eq!(moves(body)[0].0, written);
}

//! Whole-element windows through an exclusive element view `@mut [T]`.
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CleanupAction, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceProjection, PredicateOperator, PredicateStep,
    ProjectedPlace, RuntimeIndex, StructuralStoreDisposition, TypeShape, ValueId,
};

const CELL: &str = "data Cell{v:u8;}";
const UNPROVED_SELECTOR: &str = "cannot prove the store selector equal to or distinct from the selector of an open vacancy";

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
    // None of these pairs is known either equal or disjoint.
    for (params, taken, written) in [
        (",i:u64,j:u64", "i", "j"),
        (",i:u64", "i", "0"),
        (",i:u64", "0", "i"),
        // The same operation over a different operand is not congruent.
        (",i:u64,j:u64", "i+1", "j+1"),
    ] {
        let src = format!("{CELL} machine edit(p:@mut [Cell]{params}) {{let old=move p[{taken}]; p[{written}]=move old;}}");
        let destination = format!("p[{written}]");
        sources::refuse(
            &[("view.omg", &src)],
            "view.omg",
            sources::span_within(&src, &format!("{destination}=move old"), &destination),
            UNPROVED_SELECTOR,
        );
    }
    assert_unrelated_premises_leave_the_store_unresolved();
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
    spec_corpus::observe_handoff!(
        psi "psi-output/replaced-element-cleanup",
        assert!(matches!(edge.cleanup.as_slice(), [CleanupAction::DiscardRoot(place)] if OperationResult::Structural(*place) == old)),
    );
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
fn reusing_a_computed_selector_repairs_the_window_with_separate_bounds_obligations() {
    let src = format!("{CELL} machine edit(p:@mut [Cell],i:u64) {{let k=i+1; let old=move p[k]; p[k]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let Parameter::Structural(view) = body.parameters[0] else {
        panic!("expected a view");
    };
    let mut sums = body.operations.iter().filter(|op| {
        matches!(&op.kind, OperationKind::Integer { operation, .. } if operation.primitive == omega_lowering::IntegerPrimitive::Add)
    });
    let sum = sums.next().expect("the captured i+1").result;
    assert!(sums.next().is_none(), "the selector must not be recomputed");
    let [(taken, _)] = moves(body)[..] else {
        panic!("expected only the opening move");
    };
    let [(written, _, disposition)] = stores(body)[..] else {
        panic!("expected one repair");
    };
    let (taken, taken_length) = element(taken, view);
    let (written, written_length) = element(written, view);
    assert_eq!(OperationResult::Scalar(taken.selector), sum);
    assert_eq!(written.selector, taken.selector);
    assert_ne!(written.obligation, taken.obligation);
    assert_ne!(written_length, taken_length);
    assert_eq!(*disposition, StructuralStoreDisposition::FillVacancy);
}

#[test]
fn recomputing_a_selector_from_the_same_values_repairs_the_active_window() {
    let src = format!("{CELL} machine edit(p:@mut [Cell],i:u64) {{let old=move p[i+1]; p[i+1]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [Parameter::Structural(view), Parameter::Scalar(i)] = body.parameters[..] else {
        panic!("expected a view descriptor and a scalar selector");
    };
    // Each occurrence evaluates its own i+1: the same operation over i and an equal literal.
    let sums: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::Integer { operation, operands }, OperationResult::Scalar(sum)) => Some((operation, operands.as_slice(), sum)),
            _ => None,
        })
        .collect();
    let [(first, [a, one], taken_sum), (second, [b, other], written_sum)] = sums[..] else {
        panic!("expected one binary integer operation per occurrence, saw {sums:?}");
    };
    assert_eq!(first.primitive, omega_lowering::IntegerPrimitive::Add);
    assert_eq!(first, second);
    assert_eq!((*a, *b), (i, i));
    let literal = |value: ValueId| match &body.values[value.index()].origin {
        omega_lowering::ValueOrigin::Operation(op) => match &body.operations[op.index()].kind {
            OperationKind::Constant(literal) => Some(literal.clone()),
            _ => None,
        },
        _ => None,
    };
    assert_ne!(one, other, "each occurrence evaluates its own literal");
    assert_eq!(literal(*one), Some(omega_lowering::ScalarLiteral::Integer(1.into())));
    assert_eq!(literal(*one), literal(*other));
    let [(taken, _)] = moves(body)[..] else {
        panic!("filling the vacancy extracts nothing else");
    };
    let [(written, _, disposition)] = stores(body)[..] else {
        panic!("one repairing store");
    };
    let (taken, taken_length) = element(taken, view);
    let (written, written_length) = element(written, view);
    // Lowering hands the checker both evaluations, never a merged selector.
    assert_eq!((taken.selector, written.selector), (taken_sum, written_sum));
    assert_ne!(written.selector, taken.selector);
    assert_ne!(written.obligation, taken.obligation);
    assert_ne!(written_length, taken_length);
    spec_corpus::observe_handoff!(
        psi "psi-output/recomputed-selector-fill",
        assert_eq!(*disposition, StructuralStoreDisposition::FillVacancy),
    );
    for block in &body.blocks {
        match &block.terminator {
            BlockTerminator::Return { cleanup, .. } => assert!(cleanup.is_empty()),
            BlockTerminator::Jump(edge) => assert!(edge.cleanup.is_empty()),
            _ => {}
        }
    }
}

/// Formals of every premise fixture: the view, a spare cell and two selectors.
const PREMISED: &str = "machine edit(p:@mut [Cell],c:Cell,i:u64,j:u64)";

/// The entry clause `Formal(2) operator Formal(3)` that each premise fixture publishes.
fn selector_clause(operator: PredicateOperator) -> Vec<PredicateStep> {
    vec![PredicateStep::Formal(2), PredicateStep::Formal(3), PredicateStep::Apply(operator)]
}

/// The extracted value reaches the store only through a whole move of the extraction's result.
fn moved_from(
    body: &MachineBody,
    value: PlaceId,
    extracted: OperationResult,
) -> bool {
    body.operations.iter().any(|op| {
        op.result == OperationResult::Structural(value)
            && matches!(&op.kind, OperationKind::MoveStructural { source } if source.path.is_empty() && OperationResult::Structural(source.root) == extracted)
    })
}

#[test]
fn an_authored_selector_premise_chooses_the_store_the_checker_must_justify() {
    // Each requirement decides the relation, and each body is valid under it:
    // equal selectors repair the vacancy, distinct ones displace a sibling and
    // repair the vacancy later. Psi fixes the disposition before the checker
    // runs, so the clause picks it. The clause stays published and both
    // selectors stay separate values: the checker must establish the relation.
    let equal = format!("{CELL} {PREMISED} requires i == j {{let old=move p[i]; p[j]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &equal)]).unwrap();
    let machine = &pkg.machines[0];
    let body = &machine.body;
    let [
        Parameter::Structural(view),
        Parameter::Structural(c),
        Parameter::Scalar(i),
        Parameter::Scalar(j),
    ] = body.parameters[..]
    else {
        panic!("expected a view, a cell and two selectors");
    };
    let [clause] = &machine.contract.requires[..] else {
        panic!("the authored premise is the machine's one entry clause");
    };
    assert_eq!(clause.predicate, selector_clause(PredicateOperator::Equal));
    let [(taken, old)] = moves(body)[..] else {
        panic!("a premised repair extracts nothing else");
    };
    let [(written, value, disposition)] = stores(body)[..] else {
        panic!("one store");
    };
    let (taken, taken_length) = element(taken, view);
    let (written, written_length) = element(written, view);
    assert_eq!((taken.selector, written.selector), (i, j), "each occurrence keeps its own formal");
    assert_ne!(taken.obligation, written.obligation);
    assert_ne!(taken_length, written_length);
    assert!(moved_from(body, value, old));
    spec_corpus::observe_handoff!(
        psi "psi-output/premised-equal-selector-fill",
        assert_eq!(*disposition, StructuralStoreDisposition::FillVacancy),
    );
    // Only the unused spare cell is disposed; the repaired element is not.
    let cleanups: Vec<_> = body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Return { cleanup, .. } => Some(cleanup.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(cleanups, [&[CleanupAction::DiscardRoot(c)][..]]);
    psi_handoff_consumer::product_witness::premised_vacancy_fill(machine, 0, 2, 3).unwrap();

    let distinct = format!("{CELL} {PREMISED} requires i != j {{let old=move p[i]; p[j]=move c; p[i]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &distinct)]).unwrap();
    let machine = &pkg.machines[0];
    let body = &machine.body;
    let [
        Parameter::Structural(view),
        Parameter::Structural(c),
        Parameter::Scalar(i),
        Parameter::Scalar(j),
    ] = body.parameters[..]
    else {
        panic!("expected a view, a cell and two selectors");
    };
    let [clause] = &machine.contract.requires[..] else {
        panic!("the authored premise is the machine's one entry clause");
    };
    assert_eq!(clause.predicate, selector_clause(PredicateOperator::NotEqual));
    let [(vacated, old), (occupied, displaced)] = moves(body)[..] else {
        panic!("the vacancy and one displaced sibling");
    };
    let [(sibling, supplied, sibling_disposition), (repaired, restored, repair_disposition)] = stores(body)[..] else {
        panic!("the sibling store, then the repair");
    };
    // The displacement extracts at the same occurrence it stores through.
    assert_eq!(occupied, sibling);
    assert_eq!(element(vacated, view).0.selector, i);
    assert_eq!(element(sibling, view).0.selector, j);
    assert_eq!(element(repaired, view).0.selector, i);
    assert!(moved_from(body, supplied, OperationResult::Structural(c)), "the sibling receives the spare cell");
    assert!(moved_from(body, restored, old));
    assert_eq!(*repair_disposition, StructuralStoreDisposition::FillVacancy);
    assert_eq!(*sibling_disposition, StructuralStoreDisposition::FillVacancy);
    let OperationResult::Structural(displaced) = displaced else {
        panic!("a displaced element is an owned place");
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/premised-distinct-selector-displacement",
        assert!(
            body.blocks
                .iter()
                .any(|block| matches!(&block.terminator, BlockTerminator::Jump(edge) if edge.cleanup == [CleanupAction::DiscardRoot(displaced)]))
        ),
    );
    psi_handoff_consumer::product_witness::premised_sibling_displacement(machine, 0, 2, 3).unwrap();
}

#[test]
fn a_selector_premise_chooses_by_the_exact_selectors_it_names() {
    // The clause order does not matter.
    let src = format!("{CELL} {PREMISED} requires j == i {{let old=move p[i]; p[j]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    assert_eq!(pkg.machines[0].contract.requires[0].predicate[..2], [PredicateStep::Formal(3), PredicateStep::Formal(2)]);
    let [(_, _, disposition)] = stores(&pkg.machines[0].body)[..] else {
        panic!("one store");
    };
    assert_eq!(*disposition, StructuralStoreDisposition::FillVacancy);

    // A literal selector is related by a clause naming that literal.
    let src = format!("{CELL} machine edit(p:@mut [Cell],c:Cell,i:u64) requires i != 0 {{let old=move p[i]; p[0]=move c; p[i]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [Parameter::Structural(view), _, Parameter::Scalar(i)] = body.parameters[..] else {
        panic!("expected a view, a cell and a selector");
    };
    let [(vacated, _), (occupied, _)] = moves(body)[..] else {
        panic!("the vacancy and the displaced literal element");
    };
    assert_eq!(element(vacated, view).0.selector, i);
    let [(sibling, _, _), (repaired, _, _)] = stores(body)[..] else {
        panic!("two stores");
    };
    assert_eq!(occupied, sibling);
    assert_eq!(element(repaired, view).0.selector, i);

    // With two open vacancies the store must relate to each of them.
    let src = format!(
        "{CELL} machine edit(p:@mut [Cell],i:u64,j:u64,k:u64) requires i == j requires j != k {{let a=move p[i]; let b=move p[k]; p[j]=move a; p[k]=move b;}}"
    );
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [
        Parameter::Structural(view),
        Parameter::Scalar(i),
        Parameter::Scalar(j),
        Parameter::Scalar(k),
    ] = body.parameters[..]
    else {
        panic!("expected a view and three selectors");
    };
    let taken: Vec<_> = moves(body)
        .iter()
        .map(|(place, _)| element(place, view).0.selector)
        .collect();
    assert_eq!(taken, [i, k], "both repairs fill a vacancy and displace nothing");
    let written: Vec<_> = stores(body)
        .iter()
        .map(|(place, _, d)| (element(place, view).0.selector, (*d).clone()))
        .collect();
    assert_eq!(written, [
        (j, StructuralStoreDisposition::FillVacancy),
        (k, StructuralStoreDisposition::FillVacancy)
    ]);

    // Under `i != j` the store through j displaces even when it supplies the
    // removed value, and the vacancy at i stays open to the return. Lowering
    // hands the checker this exact product; the missing repair is its refusal.
    let src = format!("{CELL} {PREMISED} requires i != j {{let old=move p[i]; p[j]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [Parameter::Structural(view), _, Parameter::Scalar(i), Parameter::Scalar(j)] = body.parameters[..] else {
        panic!("expected a view, a cell and two selectors");
    };
    let taken: Vec<_> = moves(body)
        .iter()
        .map(|(place, _)| element(place, view).0.selector)
        .collect();
    assert_eq!(taken, [i, j]);
    let written: Vec<_> = stores(body)
        .iter()
        .map(|(place, _, _)| element(place, view).0.selector)
        .collect();
    assert_eq!(written, [j], "no store repairs the element vacated through i");
}

fn assert_unrelated_premises_leave_the_store_unresolved() {
    // Each program can run with the two selectors equal or distinct, and no
    // clause decides which, so neither encoding of the store is the program's.
    for (signature, body, destination) in [
        // The distinct-selector body without its premise.
        (PREMISED.to_string(), "let old=move p[i]; p[j]=move c; p[i]=move old;", "p[j]=move c"),
        // The premise names a third selector.
        (
            "machine edit(p:@mut [Cell],c:Cell,i:u64,j:u64,k:u64) requires i == k".into(),
            "let old=move p[i]; p[j]=move old;",
            "p[j]=move old",
        ),
        // The store relates to the first vacancy but not to the second.
        (
            "machine edit(p:@mut [Cell],i:u64,j:u64,k:u64) requires i == j".into(),
            "let a=move p[i]; let b=move p[k]; p[j]=move a; p[k]=move b;",
            "p[j]=move a",
        ),
    ] {
        let src = format!("{CELL} {signature} {{{body}}}");
        sources::refuse(
            &[("view.omg", &src)],
            "view.omg",
            sources::span_within(&src, destination, "p[j]"),
            UNPROVED_SELECTOR,
        );
    }
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

#[test]
fn two_entry_equalities_still_stop_before_the_checker_can_relate_the_selectors() {
    let source = "data Cell{v:u8;} machine edit(p:@mut [Cell],c:Cell,i:u64,j:u64,k:u64) requires i == k requires k == j {let old=move p[i]; p[j]=move old;}";
    sources::refuse(
        &[("view.omg", source)],
        "view.omg",
        sources::span_within(source, "p[j]=move old", "p[j]"),
        UNPROVED_SELECTOR,
    );
}

/// The bounded selector twins F1/F2 and their unbounded originals A1/A2
/// (`selector-design-challenge`), byte for byte.
const LEASE: &str = "data Lease dropped by release {mark:u8;} machine release(y:Lease) {let Lease {mark as _}=y;} ";
const LEASE_EDIT: &str = "machine edit(p:@mut [Lease],c:Lease,i:u64,j:u64,k:u64)";
const LEASE_SWAP: &str = "{let old=move p[i]; p[j]=move c; p[i]=move old;}";
/// Two same-typed views swap element `i`; each has its own bound clause.
const TWO_VIEW_SWAP: &str =
    "machine edit(p:@mut [Lease],q:@mut [Lease],i:u64) requires i < p.len requires i < q.len {let a=move p[i]; let b=move q[i]; p[i]=move b; q[i]=move a;}";

/// `Formal(selector) < Formal(view).len`: the live element count of that exact formal.
fn element_bound(
    selector: usize,
    view: usize,
) -> Vec<PredicateStep> {
    vec![
        PredicateStep::Formal(selector),
        PredicateStep::Formal(view),
        PredicateStep::ElementViewLength,
        PredicateStep::Apply(PredicateOperator::LessThan),
    ]
}

fn edit(pkg: &omega_lowering::LoweredPackage) -> &omega_lowering::LoweredMachine {
    pkg.machines
        .iter()
        .find(|machine| machine.declaration.path == "edit")
        .expect("the edit machine")
}

#[test]
fn a_length_clause_on_a_nominal_element_view_states_the_bound_without_changing_the_body() {
    // `i < p.len` on `p: @mut [Lease]` measures the same live element count
    // each element bound of `p` is owed against. The clause is an erased term,
    // so the bounded program's body is the unbounded program's body: the same
    // extractions, stores, length observations and dispositions. Only the
    // contract gains the two bounds, and the selector premise still picks the store.
    for (relation, operator) in [
        ("i == j", PredicateOperator::Equal),
        ("i != j", PredicateOperator::NotEqual),
    ] {
        let bounded = format!("{LEASE}{LEASE_EDIT} requires i < p.len requires j < p.len requires {relation} {LEASE_SWAP}");
        let unbounded = format!("{LEASE}{LEASE_EDIT} requires {relation} {LEASE_SWAP}");
        let bounded = sources::lower(&[("view.omg", &bounded)]).unwrap();
        let unbounded = sources::lower(&[("view.omg", &unbounded)]).unwrap();
        let (with, without) = (edit(&bounded), edit(&unbounded));
        let clauses: Vec<_> = with
            .contract
            .requires
            .iter()
            .map(|clause| clause.predicate.clone())
            .collect();
        assert_eq!(
            clauses,
            [
                element_bound(2, 0),
                element_bound(3, 0),
                vec![
                    PredicateStep::Formal(2),
                    PredicateStep::Formal(3),
                    PredicateStep::Apply(operator)
                ]
            ]
        );
        assert!(matches!(bounded.types.get(with.parameter_types[0]), Some(TypeShape::MutableElementView { .. })));
        assert_eq!(bounded.types.get(with.parameter_types[0]), unbounded.types.get(without.parameter_types[0]));
        let (a, b) = (&with.body, &without.body);
        assert_eq!(
            (&a.parameters, &a.places, &a.values, &a.operations, &a.blocks),
            (&b.parameters, &b.places, &b.values, &b.operations, &b.blocks),
            "{relation}: the length clause adds no body work and changes no ownership"
        );
        // Each occurrence's bound is stated by the clause on its own selector formal.
        let read = psi_handoff_consumer::view_element_bounds(&bounded, with).unwrap();
        let [
            Parameter::Structural(_),
            _,
            Parameter::Scalar(i),
            Parameter::Scalar(j),
            _,
        ] = a.parameters[..]
        else {
            panic!("expected a view, a lease and three selectors");
        };
        assert!(!read.is_empty());
        for bound in &read {
            assert_eq!(bound.view, 0);
            let expected: &[usize] = if bound.selector == i {
                &[0]
            } else if bound.selector == j {
                &[1]
            } else {
                panic!("{relation}: an element bound kept a selector that is not a formal")
            };
            assert_eq!(bound.stated_by, expected, "{relation}");
        }
        // Without the clauses the same bounds are still owed and nothing states them.
        let owed = psi_handoff_consumer::view_element_bounds(&unbounded, without).unwrap();
        assert_eq!(owed.len(), read.len());
        assert!(
            owed.iter().all(|bound| bound.stated_by.is_empty()),
            "{relation}: an unbounded original still owes every bound"
        );
    }
}

#[test]
fn a_length_clause_names_its_own_view_among_equal_typed_views() {
    // Two views of the same type each owe their own bounds. A clause on `q`
    // states nothing about an element of `p`, and the reverse.
    let swap = format!("{LEASE}{TWO_VIEW_SWAP}");
    let pkg = sources::lower(&[("view.omg", &swap)]).unwrap();
    let machine = edit(&pkg);
    assert_eq!(machine.parameter_types[0], machine.parameter_types[1], "the views share one Psi type");
    let clauses: Vec<_> = machine
        .contract
        .requires
        .iter()
        .map(|clause| clause.predicate.clone())
        .collect();
    assert_eq!(clauses, [element_bound(2, 0), element_bound(2, 1)]);
    let read = psi_handoff_consumer::view_element_bounds(&pkg, machine).unwrap();
    assert_eq!(read.len(), 4);
    let claims: std::collections::BTreeSet<_> = read.iter().map(|b| (b.view, b.stated_by.as_slice())).collect();
    spec_corpus::observe_handoff!(psi "psi-output/nominal-view-bound-subject", assert_eq!(
        claims, std::collections::BTreeSet::from([(0, &[0][..]), (1, &[1][..])])
    ));

    // A clause on the other view leaves the bound owed.
    let other = format!("{LEASE}machine edit(p:@mut [Lease],q:@mut [Lease],i:u64) requires i < q.len {{let old=move p[i]; p[i]=move old;}}");
    let pkg = sources::lower(&[("view.omg", &other)]).unwrap();
    let machine = edit(&pkg);
    assert_eq!(machine.contract.requires[0].predicate, element_bound(2, 1));
    let read = psi_handoff_consumer::view_element_bounds(&pkg, machine).unwrap();
    assert!(!read.is_empty());
    assert!(
        read.iter()
            .all(|bound| bound.view == 0 && bound.stated_by.is_empty())
    );
}

#[test]
fn the_psi_bounds_reader_refuses_mutated_views_and_snapshots() {
    // Negative mutants of the two-view swap, made on published Psi alone.
    let swap = format!("{LEASE}{TWO_VIEW_SWAP}");
    let pkg = sources::lower(&[("view.omg", &swap)]).unwrap();
    let machine = edit(&pkg);
    let [
        Parameter::Structural(p),
        Parameter::Structural(q),
        Parameter::Scalar(i),
    ] = machine.body.parameters[..]
    else {
        panic!("expected two views and a selector");
    };
    let first_p = |body: &MachineBody| {
        body.operations
            .iter()
            .position(|op| matches!(&op.kind, OperationKind::BorrowedMove { source } if source.root == p))
            .expect("an extraction through p")
    };
    // Each mutant edits its own freshly lowered copy of the published package.
    let slot = pkg
        .machines
        .iter()
        .position(|candidate| candidate.declaration.path == "edit")
        .expect("the edit machine");
    let owe_against = |length: ValueId| {
        let mut mutant = sources::lower(&[("view.omg", &swap)]).unwrap();
        let body = &mut mutant.machines[slot].body;
        let at = first_p(body);
        let OperationKind::BorrowedMove { source } = &mut body.operations[at].kind else {
            unreachable!("first_p finds an extraction")
        };
        let [_, PlaceProjection::ViewElement { length: owed, .. }] = source.path.as_mut_slice() else {
            panic!("expected a view element")
        };
        *owed = length;
        let mutant_machine = &mutant.machines[slot];
        psi_handoff_consumer::view_element_bounds(&mutant, mutant_machine).map(|_| ())
    };

    // 1. The clause on p now measures the equal-typed q: p's bounds are unstated.
    let mut moved = sources::lower(&[("view.omg", &swap)]).unwrap();
    moved.machines[slot].contract.requires[0].predicate = element_bound(2, 1);
    let read = psi_handoff_consumer::view_element_bounds(&moved, &moved.machines[slot]).unwrap();
    assert!(
        read.iter()
            .filter(|bound| bound.view == 0)
            .all(|bound| bound.stated_by.is_empty())
    );
    assert!(
        read.iter()
            .filter(|bound| bound.view == 1)
            .all(|bound| bound.stated_by == [0, 1])
    );

    // 2. A p element owed against q's count: another view's snapshot.
    assert_eq!(
        owe_against(lengths(&machine.body, q)[0]),
        Err("an element bound's length observes another view")
    );

    // 3. A p element owed against a later count of p: a snapshot taken after the use.
    let later = machine.body.operations[first_p(&machine.body) + 1..]
        .iter()
        .find_map(|op| match (&op.kind, op.result) {
            (OperationKind::ElementViewLength { source }, OperationResult::Scalar(length)) if *source == p => Some(length),
            _ => None,
        })
        .expect("p is measured again for its repair");
    assert_eq!(owe_against(later), Err("an element bound's length does not dominate its occurrence"));

    // 4. A p element owed against the selector itself, not a count.
    assert_eq!(owe_against(i), Err("an element bound's length is not an operation result"));

    // Control: the unmutated lengths still read, so each refusal is the mutation's.
    assert_eq!(owe_against(lengths(&machine.body, p)[0]), Ok(()));
}

#[test]
fn a_nominal_view_count_comes_from_the_requested_formal_in_each_context() {
    let source = "data R { x:u8; } machine count(p:@mut [R],q:@mut [R],i:u64)->u64 requires i < p.len ensures result == q.len {q.len}";
    let mut package = sources::lower(&[("case.omg", source)]).unwrap();
    let m = package
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "count")
        .unwrap();
    assert_eq!(m.parameter_types[0], m.parameter_types[1]);
    assert_eq!(
        m.contract.requires[0].predicate,
        [
            PredicateStep::Formal(2),
            PredicateStep::Formal(0),
            PredicateStep::ElementViewLength,
            PredicateStep::Apply(PredicateOperator::LessThan),
        ]
    );
    assert_eq!(
        m.contract.ensures[0].predicate,
        [
            PredicateStep::Result,
            PredicateStep::Formal(1),
            PredicateStep::ElementViewLength,
            PredicateStep::Apply(PredicateOperator::Equal),
        ]
    );
    spec_corpus::observe_handoff!(psi "psi-output/returned-nominal-view-count", assert_eq!(psi_handoff_consumer::returned_element_count_formal(m), Some(1)));
    assert_eq!(m.body.operations.len(), 1);
    let Parameter::Structural(first) = m.body.parameters[0] else {
        panic!("expected the first descriptor")
    };
    m.body.operations[0].kind = OperationKind::ElementViewLength { source: first };
    assert_eq!(psi_handoff_consumer::returned_element_count_formal(m), Some(0));
    m.body.operations[0].kind = OperationKind::ByteSequenceLength { source: first };
    assert_eq!(psi_handoff_consumer::returned_element_count_formal(m), None);
}

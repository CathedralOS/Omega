//! `view.value = 7` through `view: @write Cell`
//! (`wiki/spec/terminal-psi/structural_access.md#write-only-authority`).
//!
//! A write-only loan of an existing valid `Cell` may locate `value` and store
//! into it without reading anything. Psi keeps the formal's write-only access,
//! so the checker can tell this store from the same store through `@mut`: the
//! `@mut` control lowers to the identical operation, and only the loan's
//! access differs.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{OperationKind, PlaceProjection, ProjectedPlace, ReferenceAccess, TypeShape};
use psi_handoff_consumer::{WriteOnlyStore, write_only_stores};

const CASE: &str = include_str!("fixtures/package_cases/write-only-view-store/case/case.omg");
const CONTROL: &str = include_str!("fixtures/package_cases/write-only-view-store/control/case.omg");

fn body(package: &omega_lowering::LoweredPackage) -> &omega_lowering::MachineBody {
    &package.machines.iter().find(|m| m.declaration.path == "f").unwrap().body
}

/// The formal's reference access and the body's one store.
fn store(package: &omega_lowering::LoweredPackage) -> (ReferenceAccess, OperationKind) {
    let body = body(package);
    let omega_lowering::Parameter::Structural(view) = body.parameters[0] else {
        panic!("view is a structural formal")
    };
    let Some(TypeShape::NominalReference { access, .. }) = package.types.get(body.places[view.index()].ty) else {
        panic!("a nominal reference formal")
    };
    let stores: Vec<_> = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind, OperationKind::StructuralScalarFieldStore { .. }))
        .collect();
    let [store] = stores.as_slice() else { panic!("one store") };
    (*access, store.kind.clone())
}

#[test]
fn a_store_through_a_write_only_loan_keeps_the_loans_access() {
    let pkg = source::lower(&[("case.omg", CASE)]).unwrap();
    let control = source::lower(&[("case.omg", CONTROL)]).unwrap();
    let (access, kind) = store(&pkg);
    let OperationKind::StructuralScalarFieldStore { destination, field, value } = kind.clone() else { unreachable!() };
    let omega_lowering::Parameter::Structural(view) = body(&pkg).parameters[0] else { unreachable!() };
    spec_corpus::observe_handoff!(psi "psi-output/write-only-view-store", assert_eq!(
        (
            access,
            (destination.clone(), field.path.as_str()),
            store(&control),
            write_only_stores(&pkg, "f"),
            write_only_stores(&control, "f"),
        ),
        (
            ReferenceAccess::WriteOnly,
            (ProjectedPlace { root: view, path: vec![PlaceProjection::Referent] }, "Cell::value"),
            (ReferenceAccess::Mutable, kind),
            Ok(vec![WriteOnlyStore { formal: 0, path: vec![PlaceProjection::Referent], field: Some(field.clone()), value }]),
            Ok(vec![]),
        )
    ));
}

#[test]
fn a_whole_scalar_and_a_forwarded_write_only_loan_store_without_reading() {
    // `$view = 7u8` stores the whole scalar referent through the loan.
    let scalar = source::lower(&[("scalar.omg", "machine f(view:@write u8) {$view=7u8;}")]).unwrap();
    let stores = write_only_stores(&scalar, "f").unwrap();
    let [WriteOnlyStore { formal: 0, path, field: None, .. }] = stores.as_slice() else {
        panic!("one whole-scalar store: {stores:?}")
    };
    assert_eq!(path, &[PlaceProjection::Referent]);
    // Passing the loan to another write-only formal forwards it unread.
    let text = "data Cell {value:u8;} machine g(c:@write Cell) {c.value=1u8;} machine f(view:@write Cell) {g(view)}";
    let forwarded = source::lower(&[("forward.omg", text)]).unwrap();
    assert_eq!(write_only_stores(&forwarded, "f"), Ok(vec![]));
    assert_eq!(write_only_stores(&forwarded, "g").map(|s| s.len()), Ok(1));
}

#[test]
fn the_reader_rejects_a_read_through_the_write_only_loan() {
    let mut pkg = source::lower(&[("case.omg", CASE)]).unwrap();
    let f = pkg.machines.iter_mut().find(|m| m.declaration.path == "f").unwrap();
    let op = f
        .body
        .operations
        .iter_mut()
        .find(|op| matches!(op.kind, OperationKind::StructuralScalarFieldStore { .. }))
        .unwrap();
    let OperationKind::StructuralScalarFieldStore { destination, field, .. } = op.kind.clone() else { unreachable!() };
    let mut source = destination;
    source.path.push(PlaceProjection::Field(field));
    op.kind = OperationKind::ProjectedScalarRead { source };
    assert_eq!(write_only_stores(&pkg, "f"), Err("an operation reads or takes through a write-only loan"));
}

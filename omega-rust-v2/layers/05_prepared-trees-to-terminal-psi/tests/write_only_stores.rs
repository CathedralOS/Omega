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

use omega_lowering::{OperationKind, PlaceProjection, ProjectedPlace, ReferenceAccess, StructuralStoreDisposition, TypeShape};
use psi_handoff_consumer::{WriteOnlyStore, write_only_stores};

const CASE: &str = include_str!("fixtures/package_cases/write-only-view-store/case/case.omg");
const CONTROL: &str = include_str!("fixtures/package_cases/write-only-view-store/control/case.omg");

fn body(package: &omega_lowering::LoweredPackage) -> &omega_lowering::MachineBody {
    &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "f")
        .unwrap()
        .body
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
    let OperationKind::StructuralScalarFieldStore { destination, field, value } = kind.clone() else {
        unreachable!()
    };
    let omega_lowering::Parameter::Structural(view) = body(&pkg).parameters[0] else {
        unreachable!()
    };
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
            Ok(vec![WriteOnlyStore {
                formal: 0,
                path: vec![PlaceProjection::Referent],
                crossing: 0,
                field: Some(field.clone()),
                value: omega_lowering::Parameter::Scalar(value),
            }]),
            Ok(vec![]),
        )
    ));
}

#[test]
fn a_whole_scalar_and_a_forwarded_write_only_loan_store_without_reading() {
    // `$view = 7u8` stores the whole scalar referent through the loan.
    let scalar = source::lower(&[("scalar.omg", "machine f(view:@write u8) {$view=7u8;}")]).unwrap();
    let stores = write_only_stores(&scalar, "f").unwrap();
    let [
        WriteOnlyStore {
            formal: 0, path, field: None, ..
        },
    ] = stores.as_slice()
    else {
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
    let f = pkg
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "f")
        .unwrap();
    let op = f
        .body
        .operations
        .iter_mut()
        .find(|op| matches!(op.kind, OperationKind::StructuralScalarFieldStore { .. }))
        .unwrap();
    let OperationKind::StructuralScalarFieldStore { destination, field, .. } = op.kind.clone() else {
        unreachable!()
    };
    let mut source = destination;
    source.path.push(PlaceProjection::Field(field));
    op.kind = OperationKind::ProjectedScalarRead { source };
    assert_eq!(write_only_stores(&pkg, "f"), Err("an operation reads or takes through a write-only loan"));
}

// The Psi-only reader through stored write-only loans and structural
// replacement. These tests check the reader, not which operation lowering
// chooses; the corrupted products below exist only to show what it refuses.

fn lowered(text: &str) -> omega_lowering::LoweredPackage {
    source::lower(&[("case.omg", text)]).unwrap_or_else(|error| panic!("{text}: {}", error.message))
}

fn body_mut<'a>(
    package: &'a mut omega_lowering::LoweredPackage,
    name: &str,
) -> &'a mut omega_lowering::MachineBody {
    &mut package
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == name)
        .unwrap()
        .body
}

/// A record field or case-payload field, found by its spelling for the expectation only.
fn declared(
    package: &omega_lowering::LoweredPackage,
    path: &str,
) -> omega_lowering::DeclarationId {
    package
        .data
        .iter()
        .flat_map(|data| {
            data.fields
                .iter()
                .chain(data.cases.iter().flat_map(|case| case.fields.iter()))
        })
        .find(|field| field.declaration.path == path)
        .unwrap_or_else(|| panic!("no field {path}"))
        .declaration
        .clone()
}

fn declared_case(
    package: &omega_lowering::LoweredPackage,
    path: &str,
) -> omega_lowering::DeclarationId {
    package
        .data
        .iter()
        .flat_map(|data| data.cases.iter())
        .find(|case| case.declaration.path == path)
        .unwrap_or_else(|| panic!("no case {path}"))
        .declaration
        .clone()
}

fn data_type(
    package: &omega_lowering::LoweredPackage,
    path: &str,
) -> omega_lowering::TypeId {
    package
        .data
        .iter()
        .find(|data| data.declaration.path == path)
        .unwrap()
        .carrier
}

fn formal_type(
    package: &omega_lowering::LoweredPackage,
    machine: &str,
) -> omega_lowering::TypeId {
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == machine)
        .unwrap()
        .body;
    let omega_lowering::Parameter::Structural(place) = body.parameters[0] else {
        panic!("a structural formal")
    };
    body.places[place.index()].ty
}

fn only_op(
    body: &mut omega_lowering::MachineBody,
    matches: fn(&OperationKind) -> bool,
) -> &mut omega_lowering::PsiOperation {
    let mut found = body.operations.iter_mut().filter(|op| matches(&op.kind));
    let op = found.next().expect("one matching operation");
    assert!(found.next().is_none(), "exactly one matching operation");
    op
}

const STORED: &str = "data Leaf{v:u8;} data Node{next:@write Leaf;}";

/// `x.next.v = 1u8` with `next: @write Leaf` loads the stored descriptor
/// through `x`, then writes through the stored loan. The reader names that
/// deepest crossing whether `x` owns the node or borrows it mutably.
#[test]
fn the_reader_names_the_deepest_crossing_of_a_stored_write_only_loan() {
    use PlaceProjection::{Field, Referent};
    for (parent, crossing) in [("Node", 1), ("@mut Node", 2)] {
        let package = lowered(&format!("{STORED} machine f(x:{parent}){{x.next.v = 1u8;}}"));
        let next = Field(declared(&package, "Node::next"));
        let path = if crossing == 1 {
            vec![next, Referent]
        } else {
            vec![Referent, next, Referent]
        };
        let stores = write_only_stores(&package, "f").unwrap();
        let [
            WriteOnlyStore {
                formal: 0,
                path: found,
                crossing: deepest,
                field: Some(field),
                value: omega_lowering::Parameter::Scalar(_),
            },
        ] = stores.as_slice()
        else {
            panic!("{parent}: one scalar store: {stores:?}")
        };
        assert_eq!((found, *deepest, field), (&path, crossing, &declared(&package, "Leaf::v")), "{parent}");
    }
}

/// Read the case and field identities from constructed Psi paths. A case step
/// records the selected payload. The checker must establish that the case was
/// already known. Reading the discriminator is a separate operation.
#[test]
fn the_reader_keeps_case_identity_on_either_side_of_the_loan() {
    use PlaceProjection::{Case, Field, Referent};
    let text = format!(
        "{STORED} data Ch{{case A(w:@write Leaf); case B(w:@write Leaf);}} data S{{case P(v:u8); case Q(v:u8);}} machine k(c:Ch){{}} machine s(t:@write S){{}} machine f(x:Node){{x.next.v = 1u8;}}"
    );
    let is_store: fn(&OperationKind) -> bool = |kind| matches!(kind, OperationKind::StructuralScalarFieldStore { .. });
    let rewritten = |formal: &str, path: &dyn Fn(&omega_lowering::LoweredPackage) -> Vec<PlaceProjection>, field: &str| {
        let mut package = lowered(&text);
        let (ty, path, field) = (formal_type(&package, formal), path(&package), declared(&package, field));
        let body = body_mut(&mut package, "f");
        let omega_lowering::Parameter::Structural(x) = body.parameters[0] else {
            unreachable!()
        };
        body.places[x.index()].ty = ty;
        let OperationKind::StructuralScalarFieldStore {
            destination, field: written, ..
        } = &mut only_op(body, is_store).kind
        else {
            unreachable!()
        };
        (destination.path, *written) = (path, field);
        write_only_stores(&package, "f").map(|stores| {
            stores
                .into_iter()
                .map(|s| (s.path, s.crossing))
                .collect::<Vec<_>>()
        })
    };
    let payload = |case: &'static str, field: &'static str| {
        move |package: &omega_lowering::LoweredPackage| {
            vec![
                Case(declared_case(package, case)),
                Field(declared(package, field)),
                Referent,
            ]
        }
    };
    let package = lowered(&text);
    assert_eq!(
        rewritten("k", &payload("Ch::A", "Ch::A::w"), "Leaf::v"),
        Ok(vec![(payload("Ch::A", "Ch::A::w")(&package), 2)])
    );
    assert_eq!(
        rewritten("k", &payload("Ch::A", "Ch::B::w"), "Leaf::v"),
        Err("a place's path does not follow its declared types")
    );
    assert_eq!(
        rewritten("s", &|package| vec![Referent, Case(declared_case(package, "S::P"))], "S::P::v"),
        Ok(vec![(vec![Referent, Case(declared_case(&package, "S::P"))], 0)])
    );
}

/// `x: @write Node` cannot reach the referent of a descriptor stored in the
/// node: crossing `next` loads that descriptor through the write-only loan.
/// This holds whether `next` is `@mut` or `@write`; a mutable deepest loan does
/// not make the path above it readable. L3 refuses the source, so the product
/// is corrupted from the readable `@mut Node` parent, which the reader admits.
#[test]
fn a_write_only_parent_cannot_supply_a_stored_descriptor() {
    for stored in ["@mut", "@write"] {
        let text = format!("data Leaf{{v:u8;}} data Node{{next:{stored} Leaf;}} machine h(a:@write Node){{}} machine f(x:@mut Node){{x.next.v = 1u8;}}");
        let mut package = lowered(&text);
        let control = write_only_stores(&package, "f").map(|stores| stores.iter().map(|s| s.crossing).collect::<Vec<_>>());
        assert_eq!(control, Ok(if stored == "@mut" { vec![] } else { vec![2] }), "{stored}");
        let write_only = formal_type(&package, "h");
        let body = body_mut(&mut package, "f");
        let omega_lowering::Parameter::Structural(x) = body.parameters[0] else {
            unreachable!()
        };
        body.places[x.index()].ty = write_only;
        assert_eq!(
            write_only_stores(&package, "f"),
            Err("a descriptor stored behind a write-only loan is read to cross it"),
            "{stored}"
        );
    }
}

/// Reads, takes and readable reborrows through a stored write-only loan
/// refuse exactly as through a write-only formal, as do a scalar of the wrong
/// carrier and a field the written record does not declare.
#[test]
fn the_reader_refuses_reads_and_mistyped_stores_through_a_stored_loan() {
    let text = format!("{STORED} machine f(x:Node){{x.next.v = 1u8;}}");
    let is_store: fn(&OperationKind) -> bool = |kind| matches!(kind, OperationKind::StructuralScalarFieldStore { .. });
    let read = |kind: fn(ProjectedPlace) -> OperationKind| {
        let mut package = lowered(&text);
        let op = only_op(body_mut(&mut package, "f"), is_store);
        let OperationKind::StructuralScalarFieldStore { destination, field, .. } = op.kind.clone() else {
            unreachable!()
        };
        let mut source = destination;
        source.path.push(PlaceProjection::Field(field));
        op.kind = kind(source);
        write_only_stores(&package, "f")
    };
    let taken = Err("an operation reads or takes through a write-only loan");
    assert_eq!(read(|source| OperationKind::ProjectedScalarRead { source }), taken);
    assert_eq!(read(|source| OperationKind::CopyStructural { source }), taken);
    assert_eq!(read(|source| OperationKind::MoveStructural { source }), taken);
    assert_eq!(read(|source| OperationKind::BorrowedMove { source }), taken);
    assert_eq!(
        read(|source| OperationKind::BorrowPlace {
            source,
            access: ReferenceAccess::Mutable
        }),
        Err("a readable reborrow crosses a write-only loan")
    );
    assert_eq!(
        read(|source| OperationKind::BorrowPlace {
            source,
            access: ReferenceAccess::WriteOnly
        }),
        Ok(vec![])
    );

    let mut package = lowered(&text);
    let node = formal_type(&package, "f");
    let body = body_mut(&mut package, "f");
    let OperationKind::StructuralScalarFieldStore { value, .. } = only_op(body, is_store).kind else {
        unreachable!()
    };
    body.values[value.index()].ty = node;
    assert_eq!(
        write_only_stores(&package, "f"),
        Err("a write-only store's value disagrees with its destination")
    );

    let mut package = lowered(&text);
    let next = declared(&package, "Node::next");
    let OperationKind::StructuralScalarFieldStore { field, .. } = &mut only_op(body_mut(&mut package, "f"), is_store).kind else {
        unreachable!()
    };
    *field = next;
    assert_eq!(write_only_stores(&package, "f"), Err("a write-only store names no field of its destination"));
}

/// Closed copy storage is derived from declarations: declared `Copy`
/// multiplicity, no authored cleanup, and fields (including every case
/// payload) that are themselves closed copy storage. A shared descriptor is
/// copyable but carries loan custody, so a record holding one never qualifies.
#[test]
fn closed_copy_storage_follows_multiplicity_cleanup_fields_and_custody() {
    let package =
        lowered("data C [copy] {v:u8;} data A {v:u8;} data S [copy] {case P(c:C); case Q(v:u8);} data W [copy] {a:[C;2]; s:S;} data D [copy] {r:@u8;}");
    let closed = |name: &str| psi_handoff_consumer::closed_copy_storage(&package, data_type(&package, name));
    assert_eq!(
        ["C", "A", "S", "W", "D"].map(closed),
        [true, false, true, true, false],
        "copy record, affine record, copy sum, nested copy, copy descriptor holder"
    );
}

/// A declaration that says `Copy` does not make its value discardable: a field
/// that holds custody, or an authored cleanup, means displacing it would lose
/// something. The corrupted declarations show the reader derives this rather
/// than trusting the declared multiplicity alone.
#[test]
fn closed_copy_storage_rejects_declared_copy_over_custody() {
    let text = format!("{STORED} data C [copy] {{v:u8;}} machine k(c:C){{}}");
    let fresh = || {
        let package = lowered(&text);
        let c = data_type(&package, "C");
        let index = package.data.iter().position(|data| data.carrier == c).unwrap();
        assert!(psi_handoff_consumer::closed_copy_storage(&package, c));
        (package, c, index)
    };
    let (mut package, c, index) = fresh();
    package.data[index].fields[0].carrier = data_type(&package, "Node");
    assert!(!psi_handoff_consumer::closed_copy_storage(&package, c), "a field holding a stored loan");
    let (mut package, c, index) = fresh();
    package.data[index].cleanup_target = Some(
        package
            .machines
            .iter()
            .find(|m| m.declaration.path == "k")
            .unwrap()
            .id,
    );
    assert!(!psi_handoff_consumer::closed_copy_storage(&package, c), "an authored cleanup");
    let (mut package, c, index) = fresh();
    package.data[index].fields[0].carrier = c;
    assert!(!psi_handoff_consumer::closed_copy_storage(&package, c), "a cycle through a by-value field");
    let (mut package, c, index) = fresh();
    package.data[index].multiplicity = omega_lowering::Multiplicity::Affine;
    assert!(!psi_handoff_consumer::closed_copy_storage(&package, c), "an affine declaration");
}

/// Capability publication waits for selected nominal cleanup, follows each
/// owned field and possible sum case, and stops at a descriptor's referent.
#[test]
fn write_only_authority_follows_claims_and_selected_cleanup() {
    use omega_lowering::WriteOnlyCleanup::{DescriptorCleanup, NoCodeAffine};
    let package = lowered("data Claim [linear]{n:u8;} data Hook dropped by release {n:u8;}
        machine release(x:Hook){let Hook{n as _}=x;}
        data Nested{case Plain(n:u8); case Claimed(c:Claim);}
        data Hooked{values:[Hook;2];} data Loan{r:@mut Hook;} data Cells{values:[u8;4];}");
    let capability = |name: &str| package.data.iter().find(|schema| schema.declaration.path == name).unwrap().write_only_cleanup;
    assert_eq!(["Claim", "Hook", "Nested", "Hooked", "Loan", "Cells"].map(capability),
               [None, None, None, None, Some(DescriptorCleanup), Some(NoCodeAffine)]);
}

/// A second write to the same resident slot retires the descriptor installed by
/// the first. Each incoming descriptor has its own original backing parameter.
#[test]
fn descriptor_replacements_keep_both_backings_and_the_same_resident_slot() {
    let pkg = lowered("data C [copy]{r:@u8;} data Box{c:C;}
        machine f(x:@write Box,a:@u8,b:@u8){x.c=C{r:a};x.c=C{r:b};}");
    let f = body(&pkg);
    let stores = write_only_stores(&pkg, "f").unwrap();
    assert_eq!(stores.len(), 2);
    for (store, formal) in stores.iter().zip(&f.parameters[1..]) {
        assert_eq!((store.formal, store.crossing, store.field.as_ref().unwrap().path.as_str()), (0, 0, "Box::c"));
        let (omega_lowering::Parameter::Structural(value), omega_lowering::Parameter::Structural(reference)) = (store.value, formal) else {
            panic!("record replacement and reference formal")
        };
        let omega_lowering::PlaceOrigin::Operation(origin) = f.places[value.index()].origin else { panic!("constructed record") };
        let OperationKind::EstablishRecord { fields } = &f.operations[origin.index()].kind else { panic!("constructor") };
        let [field] = fields.as_slice() else { panic!("one descriptor") };
        assert_eq!(field.field.path, "C::r");
        assert_eq!(field.operand, omega_lowering::ValueOperand::CopyWhole(*reference));
    }
}

/// Replacing a copy record through a write-only loan writes the whole field
/// without reading or disposing the old one
/// (structural_access.md#write-only-authority). The reader names the incoming
/// SSA place and the deepest crossing for a write-only formal and for a
/// write-only loan stored in an owned or mutably borrowed formal.
#[test]
fn the_reader_names_a_structural_replacement_through_a_write_only_loan() {
    use PlaceProjection::{Field, Referent};
    const COPY: &str = "data C [copy] {v:u8;} data Leaf{c:C; v:u8;} data Node{next:@write Leaf;}";
    for (formal, store, crossing) in [
        ("@write Leaf", "x.c", 0),
        ("Node", "x.next.c", 1),
        ("@mut Node", "x.next.c", 2),
    ] {
        let package = lowered(&format!("{COPY} machine f(x:{formal}){{ {store} = C{{v:1u8}}; }}"));
        let next = Field(declared(&package, "Node::next"));
        let path = match crossing {
            0 => vec![Referent],
            1 => vec![next, Referent],
            _ => vec![Referent, next, Referent],
        };
        let established = body(&package)
            .operations
            .iter()
            .find_map(|op| match (&op.kind, op.result) {
                (OperationKind::EstablishRecord { .. }, omega_lowering::OperationResult::Structural(place)) => Some(place),
                _ => None,
            })
            .expect("the incoming record");
        assert_eq!(
            write_only_stores(&package, "f"),
            Ok(vec![WriteOnlyStore {
                formal: 0,
                path,
                crossing,
                field: Some(declared(&package, "Leaf::c")),
                value: omega_lowering::Parameter::Structural(established),
            }]),
            "{formal}"
        );
    }
}

/// Corruptions of the structural replacement product: a disposition that
/// takes or disposes the old value, a read or take in place of the store, an
/// incoming value of another copy type, a destination field of another type,
/// and a destination type that is not freely discardable.
#[test]
fn the_reader_refuses_a_corrupted_write_only_replacement() {
    const TEXT: &str = "data C [copy] {v:u8;} data D [copy] {v:u8;} data Leaf{c:C; v:u8;} machine k(d:D){} machine f(x:@write Leaf){ x.c = C{v:1u8}; }";
    let is_store: fn(&OperationKind) -> bool = |kind| matches!(kind, OperationKind::StructuralStore { .. });
    let corrupted = |change: &dyn Fn(&mut omega_lowering::LoweredPackage)| {
        let mut package = lowered(TEXT);
        change(&mut package);
        write_only_stores(&package, "f")
    };
    let destination = |package: &mut omega_lowering::LoweredPackage| match &only_op(body_mut(package, "f"), is_store).kind {
        OperationKind::StructuralStore { destination, value, .. } => (destination.clone(), *value),
        _ => unreachable!(),
    };
    for disposition in [
        StructuralStoreDisposition::FillVacancy,
        StructuralStoreDisposition::Dispose(vec![]),
    ] {
        let changed = corrupted(&|package| {
            let OperationKind::StructuralStore { disposition: found, .. } = &mut only_op(body_mut(package, "f"), is_store).kind else {
                unreachable!()
            };
            *found = disposition.clone();
        });
        assert_eq!(changed, Err("a write-only loan can neither take nor dispose the displaced value"));
    }
    let reads: [fn(ProjectedPlace) -> OperationKind; 3] = [
        |source| OperationKind::CopyStructural { source },
        |source| OperationKind::MoveStructural { source },
        |source| OperationKind::BorrowedMove { source },
    ];
    for read in reads {
        let changed = corrupted(&|package| {
            let (place, _) = destination(package);
            only_op(body_mut(package, "f"), is_store).kind = read(place);
        });
        assert_eq!(changed, Err("an operation reads or takes through a write-only loan"));
    }
    let disagrees = Err("a write-only store's value disagrees with its destination");
    let other_payload = corrupted(&|package| {
        let d = data_type(package, "D");
        let (_, value) = destination(package);
        body_mut(package, "f").places[value.index()].ty = d;
    });
    assert_eq!(other_payload, disagrees);
    let other_field = corrupted(&|package| {
        let v = declared(package, "Leaf::v");
        let OperationKind::StructuralStore { destination, .. } = &mut only_op(body_mut(package, "f"), is_store).kind else {
            unreachable!()
        };
        *destination.path.last_mut().unwrap() = PlaceProjection::Field(v);
    });
    assert_eq!(other_field, disagrees);
    let affine = corrupted(&|package| {
        let c = data_type(package, "C");
        package
            .data
            .iter_mut()
            .find(|data| data.carrier == c)
            .unwrap()
            .multiplicity = omega_lowering::Multiplicity::Affine;
    });
    assert_eq!(affine, Err("a write-only replacement displaces content that is not freely discardable"));
}

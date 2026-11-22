#[path = "support/source.rs"]
mod sources;
use omega_lowering::{
    BlockTerminator, CallArgument, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceProjection, ReferenceAccess, ReturnEdge, ScalarType,
    TypeShape,
};

fn referent(
    pkg: &LoweredPackage,
    ty: omega_lowering::TypeId,
) -> Option<(ScalarType, ReferenceAccess)> {
    match pkg.types.get(ty) {
        Some(TypeShape::ScalarReference { referent, access }) => Some((*referent, *access)),
        _ => None,
    }
}

fn check_prepared_recast(
    prepared: &omega_preparation::PreparedTrees,
    src: &str,
    cast: &str,
    from: &str,
    to: &str,
) -> omega_typed::Site {
    use omega_typed::ObligationKind;
    let mut obligations = prepared
        .typed()
        .obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::Recast { .. }));
    let obligation = obligations.next().expect("one typed recast");
    assert!(obligations.next().is_none());
    let ObligationKind::Recast { source, destination } = &obligation.kind else {
        unreachable!()
    };
    let op = prepared
        .typed()
        .operations
        .iter()
        .find(|op| op.node == obligation.node)
        .unwrap();
    assert_eq!((&op.site, op.token.as_str()), (&obligation.site, "as"));
    assert_eq!(op.operands.as_slice(), std::slice::from_ref(source));
    assert_eq!(&op.result, destination);
    assert_eq!(source.to_string(), format!("@{from}"));
    assert_eq!(destination.to_string(), format!("@{to}"));
    assert_eq!(op.site.file, "recast.omg");
    assert_eq!(op.site.span.slice(src.as_bytes()), Some(cast.as_bytes()));
    op.site.clone()
}

/// A shared same-width recast keeps the original borrowed field as its backing,
/// carries the exact semantic width, and states which side reads the top bit as
/// a sign. Exchanging the carriers must exchange the recorded signedness.
#[test]
fn shared_equal_width_recast_reinterprets_the_same_borrowed_bits() {
    for (from, to, width, source_signed) in [
        ("u8", "i8", 8, false),
        ("i8", "u8", 8, true),
        ("u16", "i16", 16, false),
        ("u32", "i32", 32, false),
        ("i32", "u32", 32, true),
        ("u64", "i64", 64, false),
        ("u13", "i13", 13, false),
    ] {
        check_shared_view(
            from,
            to,
            ScalarType::Integer { signed: source_signed, width },
            ScalarType::Integer { signed: !source_signed, width },
        );
    }
}

fn check_shared_view(
    from: &str,
    to: &str,
    source_referent: ScalarType,
    destination_referent: ScalarType,
) -> (ScalarType, ScalarType) {
    let src = format!("data Cell {{value:{from};}} machine view(cell:@Cell)->@{to} {{ @cell.value as @{to} }}");
    let files = [("recast.omg", src.as_str())];
    let package = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    let site = check_prepared_recast(&prepared, &src, &format!("@cell.value as @{to}"), from, to);
    let pkg = sources::lower_prepared(&prepared, &files).unwrap();
    check_shared_product(&pkg, &site, source_referent, destination_referent)
}

fn check_shared_product(
    pkg: &LoweredPackage,
    site: &omega_typed::Site,
    source_referent: ScalarType,
    destination_referent: ScalarType,
) -> (ScalarType, ScalarType) {
    let body = &pkg.machines[0].body;
    let Parameter::Structural(cell) = body.parameters[0] else {
        panic!("the borrowed record is a descriptor")
    };
    let [borrow, recast] = body.operations.as_slice() else {
        panic!("{source_referent:?}->{destination_referent:?}: one borrow and one recast, no value conversion")
    };
    let OperationKind::BorrowPlace {
        source,
        access: ReferenceAccess::Shared,
    } = &borrow.kind
    else {
        panic!("the operand borrows the original field")
    };
    assert_eq!(&body.diagnostics.operations[recast.id.index()], site);
    assert_eq!(source.root, cell);
    let [PlaceProjection::Referent, PlaceProjection::Field(field)] = source.path.as_slice() else {
        panic!("the borrow projects a field through the record reference")
    };
    assert_eq!(field.path, "Cell::value");
    let Some(TypeShape::NominalReference { referent: owner, access: ReferenceAccess::Shared }) = pkg.types.get(body.places[cell.index()].ty) else {
        panic!("the root is a shared record reference")
    };
    let record = pkg.data.iter().find(|data| data.id == *owner).expect("borrowed record");
    let backing = record.fields.iter().find(|f| f.declaration == *field).expect("field belongs to the borrowed record");
    assert_eq!(pkg.types.get(backing.carrier), Some(&TypeShape::PrimitiveScalar(source_referent)), "stored scalar behind the borrow");
    let OperationResult::Structural(borrowed) = borrow.result else {
        panic!("a borrow yields a descriptor")
    };
    let OperationKind::ScalarBitRecast { source, source_referent: from, destination_referent: to } = recast.kind else {
        panic!("expected a scalar recast of the borrowed place")
    };
    assert_eq!((source, from, to), (borrowed, source_referent, destination_referent));
    let OperationResult::Structural(view) = recast.result else {
        panic!("a recast yields a descriptor")
    };
    assert_eq!(
        referent(pkg, body.places[borrowed.index()].ty),
        Some((source_referent, ReferenceAccess::Shared))
    );
    assert_eq!(
        referent(pkg, body.places[view.index()].ty),
        Some((destination_referent, ReferenceAccess::Shared))
    );
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result },
        ..
    } = body.blocks[0].terminator
    else {
        panic!("the recast view is returned")
    };
    assert_eq!(source, view);
    assert_eq!(body.places[result.index()].origin, omega_lowering::PlaceOrigin::Result, "return transfers the view into the result home");
    (from, to)
}

/// Changing just the selected destination plan can change storage extent
/// without changing the typed compatibility question. This runtime cast is
/// not a folded layout query. Neither L4 nor portable Psi certifies equality.
#[test]
fn selected_extent_is_not_the_semantic_width_of_the_referent() {
    use omega_preparation::{LayoutExtent, LayoutGeometry, SelectedLayout, TargetIdentity, TargetSemantics, prepare_source_package_for_target};
    use omega_typed::ValueType;
    let typed = |scalar| match scalar {
        ScalarType::Boolean => ValueType::Bool,
        ScalarType::Integer { signed, width } => ValueType::Integer { signed, width: Some(width) },
        ScalarType::IeeeFloat(omega_lowering::IeeeFloatFormat::Binary32) => ValueType::Float32,
        ScalarType::IeeeFloat(omega_lowering::IeeeFloatFormat::Binary64) => ValueType::Float64,
    };
    let target = TargetIdentity {
        profile: "recast-test".into(),
        semantics_version: 1,
        realization: vec![1].into(),
    };
    let row = |scalar, bytes| SelectedLayout {
        target: target.clone(),
        subject: typed(scalar),
        schema: vec![1].into(),
        plan: vec![bytes].into(),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(u64::from(bytes)),
            alignment: Some(1),
        },
    };
    let int = |signed, width| ScalarType::Integer { signed, width };
    for (from, to, source_bytes) in [
        (int(false, 16), int(true, 16), 2),
        (int(false, 16), int(true, 32), 4),
        (ScalarType::Boolean, int(false, 8), 1),
    ] {
        let (field, result) = (typed(from).to_string(), typed(to).to_string());
        let cast = format!("@cell.value as @{result}");
        let src = format!("data Cell {{value:{field};}} machine view(cell:@Cell)->@{result} {{ {cast} }}");
        let package = sources::resolve(&[("recast.omg", &src)]);
        for destination_bytes in [source_bytes, source_bytes * 2] {
            let rows = [row(from, source_bytes), row(to, destination_bytes)];
            let selected = TargetSemantics::new(target.clone(), rows.clone(), []).unwrap();
            for layout in rows {
                assert_eq!(selected.layout_for(&layout.subject), Some(&layout));
            }
            let prepared = prepare_source_package_for_target(&package, &[], &selected).unwrap();
            let site = check_prepared_recast(&prepared, &src, &cast, &field, &result);
            assert!(prepared.target_observations().is_empty());
            let pkg = omega_lowering::lower_package(&prepared).unwrap();
            assert!(pkg.target_closure.is_none());
            check_shared_product(&pkg, &site, from, to);
        }
    }
}

/// A bound recast view forwards its own descriptor, not the unrecast borrow.
#[test]
fn call_receives_the_recast_descriptor_directly_or_after_binding() {
    for body in [
        "let bits:@i32 = @cell.value as @i32; sink(bits);",
        "sink(@cell.value as @i32);",
    ] {
        let src = format!("data Cell {{value:u32;}} machine sink(bits:@i32) {{}} machine relay(cell:@Cell) {{ {body} }}");
        let pkg = sources::lower(&[("recast.omg", &src)]).unwrap();
        let sink = pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == "sink")
            .unwrap();
        let relay = pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == "relay")
            .unwrap();
        let body = &relay.body;
        let [borrow, recast, call] = body.operations.as_slice() else {
            panic!("borrow, recast and call")
        };
        let OperationResult::Structural(borrowed) = borrow.result else {
            panic!("descriptor")
        };
        let OperationResult::Structural(view) = recast.result else {
            panic!("descriptor")
        };
        assert!(
            matches!(recast.kind, OperationKind::ScalarBitRecast { source, source_referent: ScalarType::Integer { signed: false, width: 32 }, destination_referent: ScalarType::Integer { signed: true, width: 32 } } if source == borrowed)
        );
        let OperationKind::Call {
            callee: omega_lowering::CallTarget::Machine(callee),
            arguments,
            ..
        } = &call.kind
        else {
            panic!("direct call")
        };
        assert_eq!(*callee, sink.id);
        assert!(
            matches!(arguments.as_slice(), [CallArgument::Borrow { place, access: ReferenceAccess::Shared }] if place.root == view && place.path == [PlaceProjection::Referent])
        );
    }
}

/// A value-level cast between the same widths stays a value-preserving checked
/// conversion; it never becomes a bit reinterpretation.
#[test]
fn equal_width_value_cast_remains_an_exact_conversion() {
    let pkg = sources::lower(&[("convert.omg", "machine convert(x:u8)->i8 { x as i8 }")]).unwrap();
    let body = &pkg.machines[0].body;
    assert!(
        body.operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::ExactNumericCast { .. }))
    );
    assert!(
        !body
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::ScalarBitRecast { .. }))
    );
}

#[test]
fn differing_scalar_referents_keep_their_shared_recast_obligation() {
    let int = |signed, width| ScalarType::Integer { signed, width };
    let pairs = [
        ("u16", "i32", int(false, 16), int(true, 32)),
        ("u32", "i16", int(false, 32), int(true, 16)),
        ("u64", "u32", int(false, 64), int(false, 32)),
        ("bool", "u8", ScalarType::Boolean, int(false, 8)),
        ("u8", "bool", int(false, 8), ScalarType::Boolean),
    ].map(|(from, to, source, destination)| check_shared_view(from, to, source, destination));
    spec_corpus::observe_handoff!(psi "psi-output/differing-recast-carriers", assert_eq!(pairs, [
        (int(false, 16), int(true, 32)), (int(false, 32), int(true, 16)), (int(false, 64), int(false, 32)),
        (ScalarType::Boolean, int(false, 8)), (int(false, 8), ScalarType::Boolean),
    ]));
}

#[test]
fn returned_scalar_recast_preserves_the_exclusive_lender() {
    let unsigned = ScalarType::Integer { signed: false, width: 8 };
    let signed = ScalarType::Integer { signed: true, width: 8 };
    for (src, cast, qualifier, field, access) in [
        ("data Cell {value:u8;} machine view(cell:@mutable Cell)->@mutable i8 { @mutable cell.value as @mutable i8 }",
            "@mutable cell.value as @mutable i8", "mutable ", "Cell::value", ReferenceAccess::Mutable),
        ("data Cell {value:u8;} machine view(cell:@Cell)->@i8 { @cell.value as @i8 }",
            "@cell.value as @i8", "", "Cell::value", ReferenceAccess::Shared),
        ("data Cell {value:u8; other:u8;} machine view(cell:@mutable Cell)->@mutable i8 { @mutable cell.other as @mutable i8 }",
            "@mutable cell.other as @mutable i8", "mutable ", "Cell::other", ReferenceAccess::Mutable),
    ] {
        let files = [("recast.omg", src)];
        let package = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&package).unwrap();
        let site = check_prepared_recast(&prepared, src, cast, &format!("{qualifier}u8"), &format!("{qualifier}i8"));
        let pkg = sources::lower_prepared(&prepared, &files).unwrap();
        let body = &pkg.machines[0].body;
        let Parameter::Structural(cell) = body.parameters[0] else { panic!("record descriptor") };
        let [borrow, recast] = body.operations.as_slice() else { panic!("borrow then recast") };
        let OperationKind::BorrowPlace { source, access: loan_access } = &borrow.kind else { panic!("field loan") };
        let declaration = &pkg.data[0].fields.iter().find(|f| f.declaration.path == field).unwrap().declaration;
        assert_eq!((source.root, *loan_access), (cell, access));
        spec_corpus::observe_handoff!(psi "psi-output/recast-lender-path", assert_eq!(
            source.path, [PlaceProjection::Referent, PlaceProjection::Field(declaration.clone())]
        ));
        let (OperationResult::Structural(lender), OperationResult::Structural(view)) = (borrow.result, recast.result) else { panic!("two descriptors") };
        assert_ne!(lender, view);
        assert_eq!(referent(&pkg, body.places[lender.index()].ty), Some((unsigned, access)));
        assert_eq!(referent(&pkg, body.places[view.index()].ty), Some((signed, access)));
        assert_eq!(body.diagnostics.operations[recast.id.index()], site);
        let BlockTerminator::Return { result: ReturnEdge::Structural { source, result }, .. } = body.blocks[0].terminator else { panic!("returned descriptor") };
        assert_eq!(source, view);
        assert_eq!(body.places[result.index()].ty, body.places[view.index()].ty);
        match access {
            ReferenceAccess::Mutable => spec_corpus::observe_handoff!(psi "psi-output/mutable-recast-handback", assert_eq!(
                recast.kind, OperationKind::MutableScalarBitRecast { lender, source_referent: unsigned, destination_referent: signed }
            )),
            ReferenceAccess::Shared => assert_eq!(recast.kind, OperationKind::ScalarBitRecast {
                source: lender, source_referent: unsigned, destination_referent: signed,
            }),
            ReferenceAccess::WriteOnly => unreachable!("this program recasts only shared and mutable views"),
        }
    }
}

fn float(format: omega_lowering::IeeeFloatFormat) -> ScalarType {
    ScalarType::IeeeFloat(format)
}

/// A float view borrows the same stored bits as any other shared scalar
/// recast. Psi records the IEEE format on the destination referent only;
/// there is no value conversion between the borrow and the view.
#[test]
fn shared_float_recast_views_the_borrowed_integer_bits() {
    use omega_lowering::IeeeFloatFormat::{Binary32, Binary64};
    let int = |signed, width| ScalarType::Integer { signed, width };
    let pairs = [
        ("u32", "f32", int(false, 32), float(Binary32)),
        ("u64", "f64", int(false, 64), float(Binary64)),
        ("f32", "u32", float(Binary32), int(false, 32)),
    ]
    .map(|(from, to, source, destination)| check_shared_view(from, to, source, destination));
    assert_eq!(
        pairs,
        [
            (int(false, 32), float(Binary32)),
            (int(false, 64), float(Binary64)),
            (float(Binary32), int(false, 32))
        ]
    );
}

/// The published example: a local `u32` is borrowed, viewed as `@f32`, and
/// read back. Psi homes the local once, borrows that home, recasts the
/// borrow and reads the float through the view. No operation converts.
#[test]
fn borrowed_local_scalar_is_homed_and_read_through_its_float_view() {
    use omega_lowering::{IeeeFloatFormat, PlaceOrigin, ScalarLiteral};
    let src = "machine build() { let bits: u32 = 0x3F80_0000u32; let view: @f32 = @bits as @f32; let value: f32 = $view; }";
    let files = [("recast.omg", src)];
    let package = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    let site = check_prepared_recast(&prepared, src, "@bits as @f32", "u32", "f32");
    let pkg = sources::lower_prepared(&prepared, &files).unwrap();
    let body = &pkg.machines[0].body;
    let [constant, home, borrow, recast, read] = body.operations.as_slice() else {
        panic!("expected constant, home, borrow, recast and read: {:#?}", body.operations)
    };
    let u32_ty = ScalarType::Integer { signed: false, width: 32 };
    let f32_ty = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    assert_eq!(constant.kind, OperationKind::Constant(ScalarLiteral::Integer(0x3F80_0000u32.into())));
    let OperationResult::Scalar(initializer) = constant.result else {
        panic!("the initializer is a scalar")
    };
    assert_eq!(home.kind, OperationKind::EstablishPrimitiveLocal { value: initializer, receiving_domains: None });
    let OperationResult::Structural(home_place) = home.result else {
        panic!("the home is a place")
    };
    assert_eq!(pkg.types.get(body.places[home_place.index()].ty), Some(&TypeShape::PrimitiveScalar(u32_ty)));
    assert_eq!(body.places[home_place.index()].origin, PlaceOrigin::Operation(home.id));
    let OperationKind::BorrowPlace {
        source,
        access: ReferenceAccess::Shared,
    } = &borrow.kind
    else {
        panic!("the view borrows the home")
    };
    assert_eq!((source.root, source.path.as_slice()), (home_place, &[][..]));
    let OperationResult::Structural(borrowed) = borrow.result else {
        panic!("descriptor")
    };
    assert_eq!(referent(&pkg, body.places[borrowed.index()].ty), Some((u32_ty, ReferenceAccess::Shared)));
    assert_eq!(
        recast.kind,
        OperationKind::ScalarBitRecast {
            source: borrowed,
            source_referent: u32_ty,
            destination_referent: f32_ty
        }
    );
    assert_eq!(&body.diagnostics.operations[recast.id.index()], &site);
    let OperationResult::Structural(view) = recast.result else {
        panic!("descriptor")
    };
    assert_eq!(referent(&pkg, body.places[view.index()].ty), Some((f32_ty, ReferenceAccess::Shared)));
    let OperationKind::ProjectedScalarRead { source } = &read.kind else {
        panic!("the value is read through the view, found {:?}", read.kind)
    };
    assert_eq!((source.root, source.path.as_slice()), (view, &[PlaceProjection::Referent][..]));
    let OperationResult::Scalar(value) = read.result else {
        panic!("an independent float value")
    };
    assert_eq!(pkg.types.get(body.values[value.index()].ty), Some(&TypeShape::PrimitiveScalar(f32_ty)));
    let BlockTerminator::Return {
        result: ReturnEdge::Unit,
        cleanup,
        ..
    } = &body.blocks[0].terminator
    else {
        panic!("unit return")
    };
    assert!(cleanup.is_empty(), "a primitive home owns nothing to dispose");
}

/// Every borrow of one scalar formal shares its single home, and a later
/// plain use observes the home rather than the incoming SSA value. Without
/// a borrow the formal stays an SSA value with no home at all.
#[test]
fn borrowed_scalar_formal_has_one_home_for_every_view() {
    let borrowed = "machine keep(bits:@u32) {} machine pair(bits:u32)->u32 { keep(@bits); keep(@bits); bits }";
    let pkg = sources::lower(&[("homes.omg", borrowed)]).unwrap();
    let pair = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "pair")
        .unwrap();
    let body = &pair.body;
    let [Parameter::Scalar(formal)] = body.parameters[..] else {
        panic!("a scalar formal")
    };
    let [home, first, second, read] = body.operations.as_slice() else {
        panic!("expected home, two calls and a read: {:#?}", body.operations)
    };
    assert_eq!(home.kind, OperationKind::EstablishPrimitiveLocal { value: formal, receiving_domains: None });
    let OperationResult::Structural(home_place) = home.result else {
        panic!("the home is a place")
    };
    for call in [first, second] {
        let OperationKind::Call { arguments, .. } = &call.kind else {
            panic!("direct call")
        };
        assert!(
            matches!(arguments.as_slice(), [CallArgument::Borrow { place, access: ReferenceAccess::Shared }] if place.root == home_place && place.path.is_empty()),
            "{arguments:?}"
        );
    }
    assert_eq!(
        read.kind,
        OperationKind::PrimitiveScalarRead {
            source: home_place,
            path: vec![]
        }
    );
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = body.blocks[0].terminator
    else {
        panic!("scalar return")
    };
    assert_eq!(OperationResult::Scalar(returned), read.result, "the return observes the home");

    let unborrowed = "machine pair(bits:u32)->u32 { bits }";
    let pkg = sources::lower(&[("homes.omg", unborrowed)]).unwrap();
    assert!(pkg.machines[0].body.operations.is_empty(), "an unborrowed formal needs no home");
}

fn last_body(pkg: &LoweredPackage) -> &omega_lowering::MachineBody {
    &pkg.machines.last().unwrap().body
}

/// A borrowed `let mutable` scalar gets one home, like an immutable one, and the
/// home is writable. Each assignment stores into that home, so the view, the
/// store and later reads all name the same storage. Whether a store may run
/// while a view of the home is still read is the checker's verdict, so that
/// program lowers with the store between the borrow and the read.
#[test]
fn a_borrowed_mutable_scalar_local_keeps_one_writable_home() {
    use omega_lowering::ScalarLiteral;
    use psi_handoff_consumer::{HomeEvent, primitive_homes};
    // The original program: the mutable local's home is borrowed and recast.
    let original = "machine build() { let mutable bits: u32 = 1u32; let view: @f32 = @bits as @f32; }";
    let pkg = sources::lower(&[("recast.omg", original)]).unwrap();
    let body = last_body(&pkg);
    let [constant, home, borrow, recast] = body.operations.as_slice() else {
        panic!("expected constant, home, borrow and recast: {:#?}", body.operations)
    };
    let (OperationResult::Scalar(one), OperationResult::Structural(home_place), OperationResult::Structural(descriptor)) =
        (constant.result, home.result, borrow.result)
    else {
        panic!("a scalar initializer, a home and a descriptor")
    };
    assert_eq!(home.kind, OperationKind::EstablishPrimitiveLocal { value: one, receiving_domains: None });
    assert_eq!(body.mutable_homes, [home_place]);
    assert!(
        matches!(&borrow.kind, OperationKind::BorrowPlace { source, access: ReferenceAccess::Shared } if source.root == home_place && source.path.is_empty())
    );
    assert!(matches!(recast.kind, OperationKind::ScalarBitRecast { source, .. } if source == descriptor));
    let homes = primitive_homes(body).unwrap();
    assert_eq!(homes.len(), 1);
    spec_corpus::observe_handoff!(
        psi "psi-output/mutable-scalar-home-view",
        assert_eq!(
            (homes[0].mutable, homes[0].events.as_slice()),
            (true, &[HomeEvent::Established(one), HomeEvent::Borrowed(ReferenceAccess::Shared)][..]),
            "the writable home of 1u32 is lent once by a shared borrow"
        ),
    );

    // An assignment after the view stores into the home; the return reads it.
    let assigned = "machine build()->u32 { let mutable bits: u32 = 1u32; let view: @f32 = @bits as @f32; bits = 2u32; bits }";
    let mut pkg = sources::lower(&[("recast.omg", assigned)]).unwrap();
    let body = last_body(&pkg);
    let [home] = primitive_homes(body).unwrap().try_into().unwrap();
    let [
        HomeEvent::Established(one),
        HomeEvent::Borrowed(ReferenceAccess::Shared),
        HomeEvent::Stored(two),
        HomeEvent::Read(read),
    ] = home.events[..]
    else {
        panic!("expected establish, borrow, store and read: {:?}", home.events)
    };
    let constant = |value: omega_lowering::ValueId| {
        body.operations
            .iter()
            .find(|op| op.result == OperationResult::Scalar(value))
            .map(|op| op.kind.clone())
    };
    assert_eq!(constant(one), Some(OperationKind::Constant(ScalarLiteral::Integer(1u32.into()))));
    assert_eq!(constant(two), Some(OperationKind::Constant(ScalarLiteral::Integer(2u32.into()))));
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = body.blocks[0].terminator
    else {
        panic!("scalar return")
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/mutable-scalar-home-store",
        assert_eq!((home.mutable, returned), (true, read), "the return reads the home after the store, not the stored SSA value"),
    );
    // A Psi-only reader refuses the same store without write authority or outside the home.
    let body = &mut pkg.machines[0].body;
    let homes = std::mem::take(&mut body.mutable_homes);
    assert_eq!(primitive_homes(body), Err("store into a home without write authority"));
    body.mutable_homes = homes;
    let store = body
        .operations
        .iter_mut()
        .find(|op| matches!(op.kind, OperationKind::ProjectedScalarStore { .. }))
        .unwrap();
    let OperationKind::ProjectedScalarStore { destination, .. } = &mut store.kind else {
        unreachable!()
    };
    destination.root = omega_lowering::PlaceId(destination.root.0.checked_add(1).unwrap());
    assert_eq!(primitive_homes(body), Err("whole-primitive store outside a primitive home"));

    // The view is still read after the store. Lowering keeps that order for the checker.
    let live = "machine build()->f32 { let mutable bits: u32 = 1u32; let view: @f32 = @bits as @f32; bits = 2u32; $view }";
    let pkg = sources::lower(&[("recast.omg", live)]).unwrap();
    let body = last_body(&pkg);
    let position = |found: &dyn Fn(&OperationKind) -> bool| body.operations.iter().position(|op| found(&op.kind)).unwrap();
    let store = position(&|kind| matches!(kind, OperationKind::ProjectedScalarStore { destination, .. } if destination.path.is_empty()));
    let recast = position(&|kind| matches!(kind, OperationKind::ScalarBitRecast { .. }));
    let OperationResult::Structural(view) = body.operations[recast].result else {
        panic!("view")
    };
    let read =
        position(&|kind| matches!(kind, OperationKind::ProjectedScalarRead { source } if source.root == view && source.path == [PlaceProjection::Referent]));
    assert!(recast < store && store < read, "{:#?}", body.operations);

    // A mutable loan of the home writes through `Referent`; the later read observes the home.
    let lent = "machine build()->u32 { let mutable bits: u32 = 1u32; let r: @mutable u32 = @mutable bits; $r = 3u32; bits }";
    let pkg = sources::lower(&[("recast.omg", lent)]).unwrap();
    let body = last_body(&pkg);
    let [home] = primitive_homes(body).unwrap().try_into().unwrap();
    assert!(
        matches!(
            home.events[..],
            [
                HomeEvent::Established(_),
                HomeEvent::Borrowed(ReferenceAccess::Mutable),
                HomeEvent::Read(_)
            ]
        ),
        "{:?}",
        home.events
    );
    assert!(
        body.operations
            .iter()
            .any(|op| matches!(&op.kind, OperationKind::ProjectedScalarStore { destination, .. } if destination.path == [PlaceProjection::Referent]))
    );

    // An unborrowed mutable local stays a rebound SSA value.
    let unborrowed = "machine build()->u32 { let mutable bits: u32 = 1u32; bits = 2u32; bits }";
    let pkg = sources::lower(&[("recast.omg", unborrowed)]).unwrap();
    let body = last_body(&pkg);
    assert_eq!((primitive_homes(body).unwrap(), body.mutable_homes.as_slice()), (vec![], &[][..]));
}

/// A home live across a suspending call stays in the parked activation as
/// caller storage: no owner row, no loan row, and no activation value copy.
/// Its row says whether the callee, the continuation or both need it,
/// directly or through a retained loan of the home.
#[test]
fn a_borrowed_scalar_home_stays_in_the_parked_activation() {
    use omega_lowering::{MachineBody, PlaceId, RetainedHome, SuspensionSite};
    fn caller(pkg: &LoweredPackage) -> &MachineBody {
        &pkg.machines
            .iter()
            .find(|m| m.declaration.path == "f")
            .unwrap()
            .body
    }
    use psi_handoff_consumer::{ParkedHome, parked_homes};
    let parked = |src: &str| -> (LoweredPackage, SuspensionSite) {
        let pkg = sources::lower(&[("recast.omg", src)]).unwrap();
        let [site] = caller(&pkg).suspension_sites.clone().try_into().unwrap();
        (pkg, site)
    };
    // The original program: `v` is dead, but the continuation reads `bits` from its home.
    let original = "machine step(n:u8)->u8 suspends {n} machine f(bits:u8)->u8 suspends { let v:@u8 = @bits; let a:u8 = suspend step(1u8); bits }";
    let (mut pkg, mut site) = parked(original);
    let body = caller(&pkg);
    let home = body
        .operations
        .iter()
        .find_map(|op| match (&op.kind, op.result) {
            (OperationKind::EstablishPrimitiveLocal { .. }, OperationResult::Structural(p)) => Some(p),
            _ => None,
        })
        .unwrap();
    assert!(site.retained_loans.is_empty() && site.retained_owners.is_empty());
    let expected = ParkedHome {
        place: home,
        formal: Some(0),
        needed_by_callee: false,
        needed_after_completion: true,
        loans: vec![],
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/parked-scalar-home",
        assert_eq!(parked_homes(body, &site), Ok(vec![expected.clone()]), "the continuation reads formal 0's home, which stays in the activation"),
    );
    // A Psi-only reader rejects a missing, understated or redirected row.
    let row = site.retained_homes.pop().unwrap();
    assert_eq!(parked_homes(body, &site), Err("a needed home has no row, or the rows are out of order"));
    site.retained_homes.push(RetainedHome {
        needed_after_completion: false,
        needed_by_callee: true,
        ..row.clone()
    });
    assert_eq!(parked_homes(body, &site), Err("home row needs disagree with the call and its continuation"));
    site.retained_homes[0] = RetainedHome {
        place: PlaceId(home.0.checked_add(1).unwrap()),
        ..row.clone()
    };
    assert_eq!(parked_homes(body, &site), Err("a needed home has no row, or the rows are out of order"));
    site.retained_homes[0] = row;
    let body = pkg
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "f")
        .unwrap();
    body.body.mutable_homes.push(home);
    assert_eq!(
        parked_homes(&body.body, &site),
        Ok(vec![expected]),
        "write authority does not change the parked row"
    );

    // Without the borrow there is no home: the formal parks as a scalar value.
    let (pkg, site) = parked("machine step(n:u8)->u8 suspends {n} machine f(bits:u8)->u8 suspends { let a:u8 = suspend step(1u8); bits }");
    let body = caller(&pkg);
    let Parameter::Scalar(formal) = body.parameters[0] else {
        panic!("scalar formal")
    };
    assert!(site.retained_homes.is_empty() && site.activation.iter().any(|value| value.value == formal));

    let callees = "machine step(n:u8)->u8 suspends {n} machine g(x:@u8)->u8 suspends {$x} ";
    for (src, by_callee, after, loans) in [
        // The continuation reads the named loan; the loan keeps the home.
        (
            "machine f(bits:u8)->u8 suspends { let v:@u8 = @bits; let a:u8 = suspend step(1u8); $v }",
            false,
            true,
            1,
        ),
        // A reborrow of that loan leads back to the same home.
        (
            "machine f(bits:u8)->u8 suspends { let v:@u8 = @bits; let w:@u8 = @$v; let a:u8 = suspend step(1u8); $w }",
            false,
            true,
            1,
        ),
        // The callee borrows the home directly and the continuation reads it.
        ("machine f(bits:u8)->u8 suspends { let a:u8 = suspend g(@bits); bits }", true, true, 0),
        // A parked terminal call needs the home only while it is incomplete.
        ("machine f(bits:u8)->u8 suspends { transition { _ -> suspend g(@bits), } }", true, false, 0),
        // A mutable local's home, written before the call, is read after it.
        (
            "machine f()->u8 suspends { let mutable b:u8 = 1u8; let v:@u8 = @b; b = 2u8; let a:u8 = suspend step(1u8); b }",
            false,
            true,
            0,
        ),
    ] {
        let (pkg, site) = parked(&format!("{callees}{src}"));
        let body = caller(&pkg);
        let [home] = parked_homes(body, &site).unwrap().try_into().unwrap();
        assert_eq!(
            (home.needed_by_callee, home.needed_after_completion, home.loans.len()),
            (by_callee, after, loans),
            "{src}"
        );
        assert_eq!(
            home.loans,
            site.retained_loans
                .iter()
                .map(|loan| loan.descriptor)
                .collect::<Vec<_>>(),
            "{src}"
        );
    }
}

#[test]
fn parked_scalar_home_tracks_the_selected_same_type_parameter() {
    use psi_handoff_consumer::{parked_homes, primitive_homes};

    for (formal, selected) in [(0, "left"), (1, "right")] {
        let source = format!(
            "machine step() suspends {{}}
             machine f(left:u8, right:u8)->u8 suspends {{
                 let first:@u8 = @left;
                 let second:@u8 = @right;
                 suspend step();
                 {selected}
             }}"
        );
        let package = sources::lower(&[("homes.omg", &source)]).unwrap();
        let body = &package.machines.iter().find(|machine| machine.declaration.path == "f").unwrap().body;
        let homes = primitive_homes(body).unwrap();
        assert_eq!(homes.len(), 2);
        let home_of = |slot| homes.iter().find(|home| home.formal == Some(slot)).unwrap().place;
        let expected = home_of(formal);
        let sibling = home_of(1 - formal);
        assert_ne!(expected, sibling);
        assert_eq!(body.places[expected.index()].ty, body.places[sibling.index()].ty);

        let [site] = body.suspension_sites.as_slice() else { panic!("one suspension") };
        let [parked] = parked_homes(body, site).unwrap().try_into().unwrap();
        spec_corpus::observe_handoff!(psi "psi-output/parked-scalar-home-identity", assert_eq!((parked.formal, parked.place), (Some(formal), expected)));
        assert!(!parked.needed_by_callee && parked.needed_after_completion);
        assert!(parked.loans.is_empty());

        // Both places exist and have the row's type. Only the continuation distinguishes them.
        let mut redirected = site.clone();
        let [row] = redirected.retained_homes.as_mut_slice() else { panic!("one retained home") };
        row.place = sibling;
        assert_eq!(parked_homes(body, &redirected), Err("a needed home has no row, or the rows are out of order"));
    }
}

#[test]
fn borrowing_a_qualified_scalar_preserves_the_receiving_predicate_at_its_home() {
    let case = include_str!("fixtures/package_cases/borrow-of-a-qualified-scalar-formal/case/case.omg");
    let control = include_str!("fixtures/package_cases/borrow-of-a-qualified-scalar-formal/control/case.omg");
    let package = sources::lower(&[("case.omg", case)]).unwrap();
    let control = sources::lower(&[("case.omg", control)]).unwrap();
    let observed = |p: &LoweredPackage| {
        let m = &p.machines[0];
        let (formal, set) = psi_handoff_consumer::borrowed_scalar_formal(p, m).unwrap();
        let domains = set.into_iter().flat_map(|set| p.types.domain_atoms(set).unwrap()).map(|(decl, args)| {
            assert!(args.is_empty());
            decl.clone()
        }).collect::<Vec<_>>();
        (formal, domains)
    };
    let [small] = &package.domains[..] else { panic!("expected a single domain") };
    assert_eq!(small.declaration.path, "u8::Small");
    let requirement = &package.machines[0].contract.requires;
    let [clause] = &requirement[..] else { panic!("expected the entry predicate of Small") };
    assert_eq!(clause.predicate, [
        omega_psi::PredicateStep::Formal(0), omega_psi::PredicateStep::Integer(10.into()),
        omega_psi::PredicateStep::Apply(omega_psi::PredicateOperator::LessThan)
    ]);
    spec_corpus::observe_handoff!(psi "psi-output/qualified-scalar-home", assert_eq!(
        (observed(&package), observed(&control)), ((0, vec![small.declaration.clone()]), (1, vec![]))
    ));
    let mut corrupt = package;
    let home = corrupt.machines[0].body.operations.iter_mut().find_map(|op| match &mut op.kind {
        OperationKind::EstablishPrimitiveLocal { receiving_domains, .. } => Some(receiving_domains),
        _ => None,
    }).unwrap();
    *home = None;
    assert!(psi_handoff_consumer::borrowed_scalar_formal(&corrupt, &corrupt.machines[0]).is_none());
}

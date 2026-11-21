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
            .any(|op| matches!(op.kind, OperationKind::ExactIntegerCast { .. }))
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
        ("data Cell {value:u8;} machine view(cell:@mut Cell)->@mut i8 { @mut cell.value as @mut i8 }",
            "@mut cell.value as @mut i8", "mut ", "Cell::value", ReferenceAccess::Mutable),
        ("data Cell {value:u8;} machine view(cell:@Cell)->@i8 { @cell.value as @i8 }",
            "@cell.value as @i8", "", "Cell::value", ReferenceAccess::Shared),
        ("data Cell {value:u8; other:u8;} machine view(cell:@mut Cell)->@mut i8 { @mut cell.other as @mut i8 }",
            "@mut cell.other as @mut i8", "mut ", "Cell::other", ReferenceAccess::Mutable),
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
    assert_eq!(home.kind, OperationKind::EstablishPrimitiveLocal { value: initializer });
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
    assert_eq!(home.kind, OperationKind::EstablishPrimitiveLocal { value: formal });
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

/// A mutable scalar local is rebound by each assignment, so it has no single
/// home to lend; its borrow still refuses at the borrowed name. A value-level
/// `as f32` is not a bit cast, and float arithmetic has no Psi operation yet.
/// A home that is live across a suspending call also refuses there.
#[test]
fn float_views_do_not_admit_mutable_homes_owned_bit_casts_or_float_arithmetic() {
    let mutable = "machine build() { let mut bits: u32 = 1u32; let view: @f32 = @bits as @f32; }";
    sources::refuse(
        &[("recast.omg", mutable)],
        "recast.omg",
        sources::span_within(mutable, "@bits as", "bits"),
        "reference root has no structural place",
    );
    let owned = "machine build(bits:u32)->f32 { bits as f32 }";
    sources::refuse(
        &[("recast.omg", owned)],
        "recast.omg",
        sources::unique_span(owned, "bits as f32"),
        "cast requires supported scalar types and domain declarations",
    );
    let arithmetic = "machine add(a:@f32, b:@f32)->f32 { $a + $b }";
    sources::refuse(
        &[("recast.omg", arithmetic)],
        "recast.omg",
        sources::unique_span(arithmetic, "$a + $b"),
        "typed integer operator has incompatible carriers",
    );
    // Psi cannot yet park a primitive home, so a live one refuses at the suspension.
    let parked = "machine step(n:u8)->u8 suspends {n} machine f(bits:u8)->u8 suspends { let v:@u8 = @bits; let a:u8 = suspend step(1u8); bits }";
    sources::refuse(
        &[("recast.omg", parked)],
        "recast.omg",
        sources::unique_span(parked, "step(1u8)"),
        "suspension needs the exact backing and carry evidence of the live structural place",
    );
}

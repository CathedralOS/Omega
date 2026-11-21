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
    assert!(matches!(source.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(f)] if f.path == "Cell::value"));
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
        result: ReturnEdge::Structural { source, .. },
        ..
    } = body.blocks[0].terminator
    else {
        panic!("the recast view is returned")
    };
    assert_eq!(source, view);
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
fn incompatible_recasts_are_refused_before_psi() {
    let src = "data Cell {value:u8;} machine view(cell:@mut Cell)->@mut i8 { @mut cell.value as @mut i8 }";
    let cast = "@mut cell.value as @mut i8";
    let message = "mutable recast needs source handback evidence that Psi cannot carry";
    let error = sources::refuse(&[("recast.omg", src)], "recast.omg", sources::unique_span(src, cast), message);
    assert_eq!(error.message, message);
}

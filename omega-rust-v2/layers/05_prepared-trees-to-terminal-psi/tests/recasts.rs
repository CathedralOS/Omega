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

/// A shared same-width recast keeps the original borrowed field as its backing,
/// carries the exact semantic width, and states which side reads the top bit as
/// a sign. Exchanging the carriers must exchange the recorded signedness.
#[test]
fn shared_equal_width_recast_reinterprets_the_same_borrowed_bits() {
    for (from, to, width, source_signed) in [
        ("u8", "i8", 8, false),
        ("i8", "u8", 8, true),
        ("i32", "u32", 32, true),
        ("u64", "i64", 64, false),
        ("u13", "i13", 13, false),
    ] {
        let src = format!("data Cell {{value:{from};}} machine view(cell:@Cell)->@{to} {{ @cell.value as @{to} }}");
        let pkg = sources::lower(&[("recast.omg", &src)]).unwrap();
        let body = &pkg.machines[0].body;
        let Parameter::Structural(cell) = body.parameters[0] else {
            panic!("the borrowed record is a descriptor")
        };
        let [borrow, recast] = body.operations.as_slice() else {
            panic!("{from}->{to}: one borrow and one recast, no value conversion")
        };
        let OperationKind::BorrowPlace {
            source,
            access: ReferenceAccess::Shared,
        } = &borrow.kind
        else {
            panic!("the operand borrows the original field")
        };
        assert_eq!(source.root, cell);
        assert!(matches!(source.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(f)] if f.path == "Cell::value"));
        let OperationResult::Structural(borrowed) = borrow.result else {
            panic!("a borrow yields a descriptor")
        };
        assert_eq!(
            recast.kind,
            OperationKind::ScalarBitRecast {
                source: borrowed,
                width,
                source_signed,
                destination_signed: !source_signed,
            }
        );
        let OperationResult::Structural(view) = recast.result else {
            panic!("a recast yields a descriptor")
        };
        assert_eq!(
            referent(&pkg, body.places[borrowed.index()].ty),
            Some((ScalarType::Integer { signed: source_signed, width }, ReferenceAccess::Shared))
        );
        assert_eq!(
            referent(&pkg, body.places[view.index()].ty),
            Some((ScalarType::Integer { signed: !source_signed, width }, ReferenceAccess::Shared))
        );
        let BlockTerminator::Return {
            result: ReturnEdge::Structural { source, .. },
            ..
        } = body.blocks[0].terminator
        else {
            panic!("the recast view is returned")
        };
        assert_eq!(source, view);
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
            matches!(recast.kind, OperationKind::ScalarBitRecast { source, width: 32, source_signed: false, destination_signed: true } if source == borrowed)
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
fn incompatible_recasts_are_refused_before_psi() {
    for (field, access, target, message) in [
        ("u16", "@", "i32", "recast requires referents of exactly equal semantic width"),
        ("u32", "@", "i16", "recast requires referents of exactly equal semantic width"),
        ("u64", "@", "u32", "recast requires referents of exactly equal semantic width"),
        ("bool", "@", "u8", "recast requires unqualified fixed-width integral referents"),
        ("u8", "@mut ", "i8", "mutable recast needs source handback evidence that Psi cannot carry"),
    ] {
        let src = format!("data Cell {{value:{field};}} machine view(cell:{access}Cell)->{access}{target} {{ {access}cell.value as {access}{target} }}");
        let error = sources::lower(&[("recast.omg", &src)]).unwrap_err();
        assert_eq!(error.message, message, "{field} {access}{target}");
    }
}

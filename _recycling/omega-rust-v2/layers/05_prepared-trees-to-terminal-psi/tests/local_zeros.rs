#[path = "support/source.rs"]
mod sources;

use omega_lowering::{BlockTerminator, OperationKind, OperationResult, ReturnEdge, ScalarLiteral, TypeShape, ValueOperand};

#[test]
fn first_case_and_its_active_payload_supply_the_local_zero() {
    let source = "data Choice<'r> { case Count(n:u16); case Held(at:@'r u8); }
                  machine make<'r>()->Choice<'r> {let answer:Choice<'r>; answer}";
    let package = sources::lower(&[("zero.omg", source)]).unwrap();
    let machine = &package.machines[0];
    let body = &machine.body;
    let [zero, choice, transfer] = body.operations.as_slice() else {
        panic!("only the active payload is initialized: {:?}", body.operations)
    };
    assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    let OperationResult::Scalar(n) = zero.result else { panic!("zero scalar") };
    let OperationKind::EstablishCase { case, fields } = &choice.kind else {
        panic!("establish first case")
    };
    assert_eq!(case.path, "Choice::Count");
    let [field] = fields.as_slice() else { panic!("count payload") };
    assert_eq!(field.field.path, "Choice::Count::n");
    assert_eq!(field.operand, ValueOperand::Scalar(n));
    for op in [zero, choice] {
        let row = &body.diagnostics.operations[op.id.index()];
        assert_eq!(row.span.slice(source.as_bytes()), Some(&b"let answer:Choice<'r>;"[..]));
    }
    let OperationResult::Structural(answer) = choice.result else {
        panic!("case result")
    };
    assert!(matches!(&transfer.kind, OperationKind::MoveStructural {source} if source.root == answer && source.path.is_empty()));
    assert!(
        matches!(body.blocks[0].terminator, BlockTerminator::Return {result: ReturnEdge::Structural {source, ..}, ..}
        if transfer.result == OperationResult::Structural(source))
    );
    let active_reference = source.replace("case Count(n:u16); case Held(at:@'r u8);", "case Held(at:@'r u8); case Count(n:u16);");
    let error = sources::lower(&[("zero.omg", &active_reference)]).expect_err("active reference needs a source");
    assert!(
        error.message.contains("Choice::Held::at")
            && error
                .message
                .contains("zero cannot supply a borrowed reference")
    );
}

#[test]
fn a_zeroed_local_uses_the_same_mutable_home_for_borrow_reassignment_and_read() {
    let source = "machine set(p:@mutable u8){$p=1u8;}
                  machine count()->u8 {let mutable x:u8; set(@mutable x); x=9u8; x}";
    let package = sources::lower(&[("zero.omg", source)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "count")
        .unwrap()
        .body;
    let [zero, home, call, nine, store, read] = body.operations.as_slice() else {
        panic!("one home and one call: {:?}", body.operations)
    };
    assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    let OperationResult::Scalar(initializer) = zero.result else {
        panic!("scalar")
    };
    assert!(matches!(home.kind, OperationKind::EstablishPrimitiveLocal { value, .. } if value == initializer));
    let OperationResult::Structural(place) = home.result else {
        panic!("primitive home")
    };
    assert_eq!(body.mutable_homes, [place]);
    assert!(
        matches!(&call.kind, OperationKind::Call { arguments, .. } if matches!(arguments.as_slice(), [ValueOperand::Borrow { place: p, .. }] if p.root == place && p.path.is_empty()))
    );
    assert_eq!(nine.kind, OperationKind::Constant(ScalarLiteral::Integer(9.into())));
    assert!(matches!(&store.kind, OperationKind::ProjectedScalarStore { destination, value }
        if destination.root == place && destination.path.is_empty() && nine.result == OperationResult::Scalar(*value)));
    assert!(matches!(&read.kind, OperationKind::PrimitiveScalarRead { source, path } if *source == place && path.is_empty()));
}

#[test]
fn local_fixed_array_zero_has_constant_size_and_nested_record_transfers_its_zero() {
    let source = "machine array()->[u8;65536] {let a:[u8;65536]; a}
                  data Inner { v:u8; } data Outer { inner:Inner; }
                  machine outer()->Outer {let o:Outer; o}";
    let package = sources::lower(&[("zero.omg", source)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "array")
        .unwrap()
        .body;
    let [array] = body.operations.as_slice() else {
        panic!("a whole array needs one zero")
    };
    let OperationResult::Structural(place) = array.result else {
        panic!("array place")
    };
    assert_eq!(array.kind, OperationKind::EstablishZeroedStructural);
    assert!(matches!(
        package.types.get(body.places[place.index()].ty),
        Some(TypeShape::FixedArray { length: 65536, .. })
    ));
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "outer")
        .unwrap()
        .body;
    let [inner, outer, transfer] = body.operations.as_slice() else {
        panic!("nested establishment: {:?}", body.operations)
    };
    let OperationResult::Structural(place) = inner.result else {
        panic!("inner place")
    };
    assert_eq!(inner.kind, OperationKind::EstablishZeroedStructural);
    assert!(
        matches!(&outer.kind, OperationKind::EstablishRecord { fields } if matches!(fields.as_slice(), [field]
        if field.field.path == "Outer::inner" && field.operand == ValueOperand::MoveWhole(place)))
    );
    assert!(matches!(&transfer.kind, OperationKind::MoveStructural {source}
        if outer.result == OperationResult::Structural(source.root) && source.path.is_empty()));
}

#[test]
fn an_unused_zero_still_registers_its_exact_carrier_and_a_record_keeps_its_cleanup() {
    use omega_lowering::{CleanupAction, ScalarType};
    let text = "data Inner dropped by end {n:u8;} machine end(x:Inner){let Inner{n as _}=x;}
                machine f(){let unused:u13; let x:Inner;}";
    let package = sources::lower(&[("zero.omg", text)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "f")
        .unwrap()
        .body;
    let [scalar, record] = body.operations.as_slice() else {
        panic!("two local establishments")
    };
    let OperationResult::Scalar(value) = scalar.result else {
        panic!("zero value")
    };
    assert_eq!(scalar.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    assert_eq!(
        package.types.get(body.values[value.index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 13 }))
    );
    let OperationResult::Structural(home) = record.result else {
        panic!("record home")
    };
    assert_eq!(record.kind, OperationKind::EstablishZeroedStructural);
    let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
        panic!("return")
    };
    let end = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "end")
        .unwrap()
        .id;
    assert!(matches!(cleanup.as_slice(), [CleanupAction::Invoke {place,target,..}] if place.root==home && *target==end && place.path.is_empty()));
}

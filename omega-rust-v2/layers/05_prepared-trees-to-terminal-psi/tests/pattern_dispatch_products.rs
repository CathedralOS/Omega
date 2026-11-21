#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    ArithmeticPolicy, BlockTerminator, IntegerPrimitive, OperationKind, OperationResult, Parameter, PlaceProjection, ReturnEdge, ScalarLiteral, ScalarType,
    TypeShape,
};

#[test]
fn integer_tests_share_one_subject_and_reach_the_join_in_source_order() {
    let text = "machine sample(x:u16)->u16{x} machine choose(x:u16)->u8 {1u8 +% match sample(x) {300u16 -> 4u8, 2u16 -> 5u8, _ -> 6u8,}}";
    let package = sources::lower(&[("order.omg", text)]).unwrap();
    let [choose, sample] = package.machines.as_slice() else {
        panic!("expected caller and callee")
    };
    let body = &choose.body;
    let entry = &body.blocks[0];
    let calls = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind,OperationKind::Call{callee:omega_lowering::CallTarget::Machine(m),..} if m==sample.id))
        .collect::<Vec<_>>();
    let [call] = calls[..] else { panic!("the subject must execute once") };
    assert!(entry.operations.contains(&call.id.index()));
    let OperationResult::Scalar(subject) = call.result else {
        panic!("expected a scalar")
    };
    let mut test = entry;
    let mut join = None;
    for (value, result) in [(300, 4), (2, 5)] {
        let [.., literal, compare] = &body.operations[test.operations.clone()] else {
            panic!("expected literal and comparison")
        };
        assert_eq!(literal.kind, OperationKind::Constant(ScalarLiteral::Integer(value.into())));
        let OperationResult::Scalar(rhs) = literal.result else {
            panic!("expected scalar constant")
        };
        assert_eq!(body.values[rhs.index()].ty, body.values[subject.index()].ty);
        assert_eq!(
            package.types.get(body.values[rhs.index()].ty),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 16 }))
        );
        assert!(
            matches!(&compare.kind,OperationKind::Integer{operation,operands} if operation.primitive==IntegerPrimitive::Equal && operation.policy==ArithmeticPolicy::Exact && operation.obligations.is_empty() && operands==&[subject,rhs])
        );
        let BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &test.terminator
        else {
            panic!("expected a conditional test")
        };
        assert_eq!(compare.result, OperationResult::Scalar(*condition));
        let matched = &body.blocks[when_true.target.get() - 1];
        let [arm] = &body.operations[matched.operations.clone()] else {
            panic!("expected selected arm constant")
        };
        assert_eq!(arm.kind, OperationKind::Constant(ScalarLiteral::Integer(result.into())));
        let BlockTerminator::Jump(edge) = &matched.terminator else {
            panic!("expected a join edge")
        };
        assert!(matches!(arm.result,OperationResult::Scalar(v) if edge.arguments==[v]));
        assert_eq!(*join.get_or_insert(edge.target), edge.target);
        test = &body.blocks[when_false.target.get() - 1];
    }
    let [fallback] = &body.operations[test.operations.clone()] else {
        panic!("expected fallback")
    };
    assert_eq!(fallback.kind, OperationKind::Constant(ScalarLiteral::Integer(6.into())));
    let BlockTerminator::Jump(edge) = &test.terminator else {
        panic!("expected fallback join edge")
    };
    assert_eq!(Some(edge.target), join);
    assert!(matches!(fallback.result,OperationResult::Scalar(v) if edge.arguments==[v]));
}

const BIND: &str = "data Choice {case A(code:u8, value:u8); case B;} machine first(x:@Choice)->u8 {match x {Choice::A {code:1u8, value} -> value, _ -> 0u8,}}";

#[test]
fn matched_payload_binding_copies_the_second_field_at_its_use_only() {
    let files = [("binding.omg", BIND)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let reads = prepared
        .typed()
        .obligations
        .iter()
        .filter(|o| {
            o.site.file == "binding.omg"
                && matches!(
                    o.kind,
                    omega_typed::ObligationKind::CopyRead(omega_typed::ValueType::Integer { signed: false, width: Some(8) })
                )
        })
        .collect::<Vec<_>>();
    let [test, use_site] = reads[..] else {
        panic!("expected a copy at the equality and a separate copy at the use")
    };
    let pkg = sources::lower_prepared(&prepared, &files).unwrap();
    let body = &pkg.machines[0].body;
    let [Parameter::Structural(x)] = body.parameters[..] else {
        panic!("expected borrowed subject")
    };
    let BlockTerminator::Conditional { when_true, when_false, .. } = &body.blocks[0].terminator else {
        panic!("expected discriminator split")
    };
    let case = when_true.selected_case.as_ref().unwrap();
    let fallback = when_false.target;
    let check = &body.blocks[when_true.target.get() - 1];
    let BlockTerminator::Conditional { when_true, when_false, .. } = &check.terminator else {
        panic!("expected value split")
    };
    assert_eq!(when_false.target, fallback);
    let arm = &body.blocks[when_true.target.get() - 1];
    let [bound] = &body.operations[arm.operations.clone()] else {
        panic!("expected arm-local field read")
    };
    let check_read = &body.operations[check.operations.start];
    for (read, name, obligation) in [
        (check_read, "Choice::A::code", test),
        (bound, "Choice::A::value", use_site),
    ] {
        let OperationKind::ProjectedScalarRead { source } = &read.kind else {
            panic!("expected projected read")
        };
        assert_eq!(source.root, x);
        assert!(
            matches!(&source.path[..],[PlaceProjection::Referent,PlaceProjection::Case(id),PlaceProjection::Field(field)] if id==&case.case && field.path==name)
        );
        assert_eq!(body.diagnostics.operations[read.id.index()], obligation.site);
    }
    assert!(matches!(arm.terminator,BlockTerminator::Return{result:ReturnEdge::Scalar(v),..} if bound.result==OperationResult::Scalar(v)));
    let default = &body.blocks[fallback.get() - 1];
    let [zero] = &body.operations[default.operations.clone()] else {
        panic!("fallback must not read the binding")
    };
    assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
}

#[test]
fn compound_payload_binding_use_still_needs_scoped_expression_lowering() {
    let text = BIND.replace("-> value,", "-> value +% 1u8,");
    sources::refuse(
        &[("binding.omg", &text)],
        "binding.omg",
        sources::span_within(&text, "value} ->", "value"),
        "payload requires one literal test and direct value bindings: FieldPattern",
    );
}

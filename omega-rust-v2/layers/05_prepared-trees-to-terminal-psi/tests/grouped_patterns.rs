#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    ArithmeticPolicy, BlockTerminator, IntegerPrimitive, OperationKind, OperationResult, Parameter, ReturnEdge, ScalarLiteral, ScalarType, TypeShape,
};

#[test]
fn grouping_preserves_the_typed_equality_and_both_decisions() {
    for (pattern, fallback, spelling, value) in [
        ("0u8", "_", "0u8", 0),
        ("(0u8)", "_", "0u8", 0),
        ("((0u8))", "((_))", "0u8", 0),
        ("((17u8))", "(_)", "17u8", 17),
    ] {
        let text = format!("machine choose(x:u8)->bool {{ match x {{ {pattern} -> true, {fallback} -> false, }} }}");
        let pkg = sources::lower(&[("choose.omg", &text)]).unwrap();
        let body = &pkg.machines[0].body;
        let [Parameter::Scalar(subject)] = body.parameters[..] else {
            panic!("expected subject")
        };
        let entry = &body.blocks[0];
        let [literal, compare] = &body.operations[entry.operations.clone()] else {
            panic!("expected literal and comparison")
        };
        let OperationResult::Scalar(rhs) = literal.result else {
            panic!("expected literal value")
        };
        assert_eq!(literal.kind, OperationKind::Constant(ScalarLiteral::Integer(value.into())));
        assert_eq!(body.values[rhs.index()].ty, body.values[subject.index()].ty);
        assert_eq!(
            pkg.types.get(body.values[rhs.index()].ty),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 }))
        );
        for op in [literal, compare] {
            let site = &body.diagnostics.operations[op.id.index()];
            assert_eq!(site.file, "choose.omg");
            assert_eq!(site.span.slice(text.as_bytes()), Some(spelling.as_bytes()));
        }
        let OperationKind::Integer { operation, operands } = &compare.kind else {
            panic!("expected integer test")
        };
        assert_eq!((operation.primitive, operation.policy), (IntegerPrimitive::Equal, ArithmeticPolicy::Exact));
        assert!(operation.obligations.is_empty());
        assert_eq!(operands, &[subject, rhs]);
        let BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &entry.terminator
        else {
            panic!("expected decision")
        };
        assert_eq!(compare.result, OperationResult::Scalar(*condition));
        for (edge, expected) in [(when_true, true), (when_false, false)] {
            let arm = &body.blocks[edge.target.get() - 1];
            let [selected] = &body.operations[arm.operations.clone()] else {
                panic!("expected one arm literal")
            };
            assert_eq!(selected.kind, OperationKind::Constant(ScalarLiteral::Boolean(expected)));
            assert!(matches!(arm.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(v), .. } if selected.result == OperationResult::Scalar(v)));
        }
    }
}

#[test]
fn grouping_does_not_hide_a_mismatched_literal_carrier() {
    for pattern in ["1u16", "(1u16)", "((1u16))"] {
        let text = format!("machine choose(x:u8)->bool {{ match x {{ {pattern} -> true, _ -> false, }} }}");
        let files = [("mismatch.omg", text.as_str())];
        let resolved = sources::resolve(&files);
        let error = omega_preparation::prepare_source_package(&resolved).unwrap_err();
        assert_eq!(error.file, "mismatch.omg");
        assert_eq!(error.span.slice(text.as_bytes()), Some(&b"1u16"[..]));
        assert_eq!(error.message, "expected u8, found u16");
    }
}

#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    ArithmeticPolicy, BlockId, BlockTerminator, BranchBlock, CallTarget, IntegerPrimitive, LoweredPackage, MachineBody, OperationKind, OperationResult,
    Parameter, PlaceProjection, PsiOperation, ReturnEdge, ScalarLiteral, ScalarType, SuccessorEdge, TypeShape, ValueId,
};

fn scalar(op: &PsiOperation) -> ValueId {
    let OperationResult::Scalar(value) = op.result else {
        panic!("expected scalar")
    };
    value
}

fn constant(
    op: &PsiOperation,
    value: u16,
) -> ValueId {
    assert_eq!(op.kind, OperationKind::Constant(ScalarLiteral::Integer(value.into())));
    scalar(op)
}

fn split(
    block: &BranchBlock,
    condition: ValueId,
) -> (&SuccessorEdge, &SuccessorEdge) {
    let BlockTerminator::Conditional {
        condition: tested,
        when_true,
        when_false,
    } = &block.terminator
    else {
        panic!("expected split")
    };
    assert_eq!(*tested, condition);
    (when_true, when_false)
}

fn equality<'a>(
    package: &LoweredPackage,
    body: &MachineBody,
    block: &'a BranchBlock,
    subject: ValueId,
    width: u64,
    pattern: u16,
) -> (&'a SuccessorEdge, &'a SuccessorEdge) {
    let [.., rhs, eq] = &body.operations[block.operations.clone()] else {
        panic!("expected literal and test")
    };
    let literal = constant(rhs, pattern);
    assert_eq!(body.values[subject.index()].ty, body.values[literal.index()].ty);
    assert_eq!(
        package.types.get(body.values[subject.index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width }))
    );
    let OperationKind::Integer { operation, operands } = &eq.kind else {
        panic!("expected equality")
    };
    assert_eq!((operation.primitive, operation.policy), (IntegerPrimitive::Equal, ArithmeticPolicy::Exact));
    assert!(operation.obligations.is_empty());
    assert_eq!(operands, &[subject, literal]);
    split(block, scalar(eq))
}

fn arm_join(
    arm: &BranchBlock,
    value: ValueId,
) -> BlockId {
    let BlockTerminator::Jump(edge) = &arm.terminator else {
        panic!("expected join")
    };
    assert_eq!(edge.arguments, [value]);
    edge.target
}

fn constant_arm(
    body: &MachineBody,
    edge: &SuccessorEdge,
    expected: u16,
) -> BlockId {
    let arm = &body.blocks[edge.target.get() - 1];
    let [op] = &body.operations[arm.operations.clone()] else {
        panic!("expected one arm constant")
    };
    arm_join(arm, constant(op, expected))
}

fn returned_parameter(join: &BranchBlock) {
    let [result] = join.scalar_parameters[..] else {
        panic!("expected joined result")
    };
    assert!(matches!(join.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(value), .. } if value == result));
}

#[test]
fn nested_integer_subject_dispatches_on_the_inner_join_parameter() {
    let text = "machine sample(x:u8)->u8{x}
        machine choose(x:u8)->u8 {
            let answer:u8 = match (match sample(x) {7u8 -> 300u16, _ -> 301u16,}) {300u16 -> 8u8, _ -> 9u8,};
            answer
        }";
    let pkg = sources::lower(&[("nested.omg", text)]).unwrap();
    let [choose, sample] = pkg.machines.as_slice() else {
        panic!("expected two machines")
    };
    let body = &choose.body;
    let calls: Vec<_> = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind, OperationKind::Call { callee: CallTarget::Machine(m), .. } if m == sample.id))
        .collect();
    let [call] = calls[..] else { panic!("expected one subject call") };
    let entry = &body.blocks[0];
    assert!(entry.operations.contains(&call.id.index()));
    let (yes, no) = equality(&pkg, body, entry, scalar(call), 8, 7);
    let join = constant_arm(body, yes, 300);
    assert_eq!(constant_arm(body, no, 301), join);
    let outer = &body.blocks[join.get() - 1];
    let [subject] = outer.scalar_parameters[..] else {
        panic!("expected inner result")
    };
    let (yes, no) = equality(&pkg, body, outer, subject, 16, 300);
    let result = constant_arm(body, yes, 8);
    assert_eq!(constant_arm(body, no, 9), result);
    returned_parameter(&body.blocks[result.get() - 1]);
}

#[test]
fn interior_payload_literal_selects_the_bound_result_or_shared_fallback() {
    let text = "data Choice { case A(code:u8, value:u16); case B; }
        machine choose(x:@Choice)->u16 {
            let answer:u16 = match x {Choice::A {code:37u8, value} -> value, _ -> 55u16,};
            answer
        }";
    let pkg = sources::lower(&[("payload.omg", text)]).unwrap();
    let body = &pkg.machines[0].body;
    let [Parameter::Structural(x)] = body.parameters[..] else {
        panic!("expected descriptor")
    };
    let entry = &body.blocks[0];
    let [tag] = &body.operations[entry.operations.clone()] else {
        panic!("expected only discriminator test")
    };
    let OperationKind::StructuralCaseMembership { source, case } = &tag.kind else {
        panic!("expected membership")
    };
    assert_eq!(source.root, x);
    assert_eq!(source.path, [PlaceProjection::Referent]);
    assert_eq!(case.path, "Choice::A");
    let (selected, wrong_case) = split(entry, scalar(tag));
    let selection = selected.selected_case.as_ref().unwrap();
    assert_eq!((&selection.case, &selection.source), (case, source));
    assert!(wrong_case.selected_case.is_none());
    let projected = |op: &PsiOperation, name: &str| {
        let OperationKind::ProjectedScalarRead { source } = &op.kind else {
            panic!("expected field read")
        };
        assert_eq!(source.root, x);
        assert!(matches!(&source.path[..], [PlaceProjection::Referent, PlaceProjection::Case(c), PlaceProjection::Field(f)] if c == case && f.path == name));
        scalar(op)
    };
    let check = &body.blocks[selected.target.get() - 1];
    let [code, rhs, _] = &body.operations[check.operations.clone()] else {
        panic!("expected field read, constant, comparison")
    };
    let span = &body.diagnostics.operations[rhs.id.index()].span;
    assert_eq!(&text[span.get_start_offset()..span.get_end_offset()], "37u8");
    let (matched, wrong_code) = equality(&pkg, body, check, projected(code, "Choice::A::code"), 8, 37);
    assert_eq!(wrong_case.target, wrong_code.target);
    let join = constant_arm(body, wrong_code, 55);
    let arm = &body.blocks[matched.target.get() - 1];
    let [bound] = &body.operations[arm.operations.clone()] else {
        panic!("expected arm-local binding read")
    };
    assert_eq!(arm_join(arm, projected(bound, "Choice::A::value")), join);
    returned_parameter(&body.blocks[join.get() - 1]);
}

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
fn same_named_payload_uses_in_sibling_arms_keep_their_own_cases() {
    let text = "data Choice {case A(code:u8, value:u8); case B(code:u8, value:u8);}
        machine first(x:@Choice)->bool {let result:bool = match x {
            Choice::A {code:1u8, value} -> value == 2u8,
            Choice::B {code:3u8, value} -> value == 4u8, _ -> false,}; result}";
    let pkg = sources::lower(&[("siblings.omg", text)]).unwrap();
    let body = &pkg.machines[0].body;
    let mut tag = &body.blocks[0];
    let mut join = None;
    for (case_name, field_name, rhs) in [
        ("Choice::A", "Choice::A::value", 2),
        ("Choice::B", "Choice::B::value", 4),
    ] {
        let BlockTerminator::Conditional { when_true, when_false, .. } = &tag.terminator else {
            panic!("expected tag split")
        };
        let case = &when_true.selected_case.as_ref().unwrap().case;
        assert_eq!(case.path, case_name);
        tag = &body.blocks[when_false.target.get() - 1];
        let check = &body.blocks[when_true.target.get() - 1];
        let BlockTerminator::Conditional { when_true, when_false, .. } = &check.terminator else {
            panic!("expected code split")
        };
        assert_eq!(when_false.target, tag.id);
        let arm = &body.blocks[when_true.target.get() - 1];
        let [read, literal, compare] = &body.operations[arm.operations.clone()] else {
            panic!("expected bound value comparison")
        };
        let OperationKind::ProjectedScalarRead { source } = &read.kind else {
            panic!("expected scoped read")
        };
        assert!(
            matches!(&source.path[..], [PlaceProjection::Referent, PlaceProjection::Case(c), PlaceProjection::Field(f)] if c == case && f.path == field_name)
        );
        assert_eq!(literal.kind, OperationKind::Constant(ScalarLiteral::Integer(rhs.into())));
        let (OperationResult::Scalar(left), OperationResult::Scalar(right)) = (read.result, literal.result) else {
            panic!("expected scalars")
        };
        assert!(
            matches!(&compare.kind, OperationKind::Integer { operation, operands } if operation.primitive == IntegerPrimitive::Equal && operands == &[left, right])
        );
        let BlockTerminator::Jump(edge) = &arm.terminator else {
            panic!("expected interior result edge")
        };
        assert!(matches!(compare.result, OperationResult::Scalar(v) if edge.arguments == [v]));
        assert_eq!(*join.get_or_insert(edge.target), edge.target);
    }
}

#[test]
fn compound_payload_binding_reads_feed_the_selected_arm_expression() {
    let text = BIND.replace("-> value,", "-> value +% 1u8,");
    let (add_operands, [bound_value, one]) = check_compound(&text, false);
    spec_corpus::observe_handoff!(psi "psi-output/compound-payload-binding", assert_eq!(add_operands, [bound_value, one]));
}

#[test]
fn compound_payload_binding_retains_rhs_call_and_repeated_use() {
    let text = "data Choice {case A(code:u8, value:u8); case B;} machine first(x:@Choice,other:u8)->u8 {match x {Choice::A {code:1u8, value} -> value +% step(value), _ -> other,}} machine step(n:u8)->u8{n}";
    let (add_operands, [bound_value, call_result]) = check_compound(text, true);
    assert_eq!(add_operands, [bound_value, call_result]);
}

fn check_compound(
    text: &str,
    with_call: bool,
) -> (Vec<omega_lowering::ValueId>, [omega_lowering::ValueId; 2]) {
    use omega_lowering::{CallArgument, CallTarget};
    let files = [("binding.omg", text)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let tree = typed.trees().next().unwrap();
    let tree = tree.resolved();
    let mut contexts = vec![("code:1u8", "code:1u8"), ("value +%", "value")];
    if with_call {
        contexts.push(("step(value)", "value"));
    }
    let copies: Vec<_> = contexts
        .into_iter()
        .map(|(context, token)| {
            let span = sources::span_within(text, context, token);
            typed
                .obligations
                .iter()
                .find(|o| {
                    o.site.file == "binding.omg"
                        && o.site.span == omega_lexer::SourceSpan::new(span.start, span.end)
                        && matches!(
                            o.kind,
                            omega_typed::ObligationKind::CopyRead(omega_typed::ValueType::Integer { signed: false, width: Some(8) })
                        )
                })
                .expect("expected an integer copy at this occurrence")
        })
        .collect();
    for occurrence in &copies[1..] {
        let Some(omega_resolved::Symbol::Binding { binder_node, .. }) = tree.symbol(occurrence.node.node) else {
            panic!("expected a bound operand")
        };
        let binder = tree.file.node_id(*binder_node);
        assert_eq!(tree.file.nodes[*binder_node].kind, omega_syntax::NodeKind::FieldPattern);
        assert_eq!(typed.place_path(occurrence.node), typed.place_path(binder));
    }

    let pkg = sources::lower_prepared(&prepared, &files).unwrap();
    let body = &pkg.machines[0].body;
    let Parameter::Structural(x) = body.parameters[0] else {
        panic!("expected borrowed subject")
    };
    let entry = &body.blocks[0];
    let [tag] = &body.operations[entry.operations.clone()] else {
        panic!("only test the discriminator before selection")
    };
    let OperationKind::StructuralCaseMembership { source, case } = &tag.kind else {
        panic!("expected tag test")
    };
    assert_eq!(
        (source.root, source.path.as_slice(), case.path.as_str()),
        (x, [PlaceProjection::Referent].as_slice(), "Choice::A")
    );
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &entry.terminator
    else {
        panic!("expected discriminator split")
    };
    assert_eq!(tag.result, OperationResult::Scalar(*condition));
    let selected = when_true.selected_case.as_ref().unwrap();
    assert_eq!((&selected.source, &selected.case), (source, case));
    assert!(when_false.selected_case.is_none());
    let fallback = when_false.target;
    let check = &body.blocks[when_true.target.get() - 1];
    let [code, one, equal] = &body.operations[check.operations.clone()] else {
        panic!("only check code after tag success")
    };
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &check.terminator
    else {
        panic!("expected literal split")
    };
    assert_eq!(equal.result, OperationResult::Scalar(*condition));
    assert_eq!(when_false.target, fallback);
    let scalar = |op: &omega_lowering::PsiOperation| match op.result {
        OperationResult::Scalar(v) => v,
        _ => panic!("expected scalar"),
    };
    assert_eq!(one.kind, OperationKind::Constant(ScalarLiteral::Integer(1.into())));
    assert!(
        matches!(&equal.kind, OperationKind::Integer { operation, operands } if operation.primitive == IntegerPrimitive::Equal
        && operation.policy == ArithmeticPolicy::Exact && operands == &[scalar(code), scalar(one)])
    );

    let arm = &body.blocks[when_true.target.get() - 1];
    let [left, rhs_work @ .., sum] = &body.operations[arm.operations.clone()] else {
        panic!("missing arm evaluation")
    };
    let mut reads = vec![(code, "Choice::A::code"), (left, "Choice::A::value")];
    let rhs = if with_call {
        let [arg, call] = rhs_work else { panic!("expected argument read and call") };
        reads.push((arg, "Choice::A::value"));
        assert!(
            matches!(&call.kind, OperationKind::Call { callee: CallTarget::Machine(id), arguments, .. } if *id == pkg.machines[1].id && arguments == &[CallArgument::Scalar(scalar(arg))])
        );
        scalar(call)
    } else {
        let [one] = rhs_work else { panic!("expected addend") };
        assert_eq!(one.kind, OperationKind::Constant(ScalarLiteral::Integer(1.into())));
        scalar(one)
    };
    assert_eq!(reads.len(), copies.len());
    for ((op, name), copy) in reads.into_iter().zip(copies) {
        let OperationKind::ProjectedScalarRead { source } = &op.kind else {
            panic!("expected borrowed read")
        };
        assert_eq!(source.root, x);
        assert!(matches!(&source.path[..], [PlaceProjection::Referent, PlaceProjection::Case(c), PlaceProjection::Field(f)] if c == case && f.path == name));
        assert_eq!(body.diagnostics.operations[op.id.index()], copy.site);
    }
    let OperationKind::Integer { operation, operands } = &sum.kind else {
        panic!("expected arithmetic result")
    };
    assert!(operation.obligations.is_empty());
    assert!(matches!(arm.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(v), .. } if v == scalar(sum)));
    let default = &body.blocks[fallback.get() - 1];
    let default_value = if with_call {
        assert!(default.operations.is_empty());
        let Parameter::Scalar(other) = body.parameters[1] else {
            panic!("expected decoy")
        };
        other
    } else {
        let [zero] = &body.operations[default.operations.clone()] else {
            panic!("expected fallback constant")
        };
        assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
        scalar(zero)
    };
    assert!(matches!(default.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(v), .. } if v == default_value));
    assert_eq!((operation.primitive, operation.policy), (IntegerPrimitive::Add, ArithmeticPolicy::Wrapping));
    (operands.clone(), [scalar(left), rhs])
}

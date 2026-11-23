#[path = "support/source.rs"]
mod sources;

use omega_lowering::{BlockTerminator, CallArgument, LoweredPackage, LoweringError, OperationResult, Parameter, PlaceOrigin, ReturnEdge};

fn lower(source: &str) -> Result<LoweredPackage, LoweringError> {
    sources::lower(&[("branches.omg", source)])
}

#[test]
fn false_first_transition_executes_its_operations_only_in_the_false_block() {
    let source = String::from(
        "machine choose(left: u8, right: u8) -> u8 {
            transition left < right {false -> left +% 1, true -> right +| 1,}
         }",
    );
    let package = lower(&source).unwrap();
    drop(source);
    let body = &package.machines[0].body;
    let [entry, false_arm, true_arm] = body.blocks.as_slice() else {
        panic!("expected the condition and both arm blocks");
    };
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &entry.terminator
    else {
        panic!("expected conditional control flow");
    };
    assert_eq!(entry.operations, 0..1);
    assert_eq!(OperationResult::Scalar(*condition), body.operations[0].result);
    assert_eq!(when_false.target, false_arm.id);
    assert_eq!(when_true.target, true_arm.id);
    assert!(when_false.arguments.is_empty());
    assert!(when_true.arguments.is_empty());
    assert_eq!(false_arm.operations, 1..3);
    assert_eq!(true_arm.operations, 3..5);
    let BlockTerminator::Return {
        id: false_return,
        result: ReturnEdge::Scalar(false_result),
        cleanup: ref false_discards,
    } = false_arm.terminator
    else {
        panic!("false arm must return its own scalar result");
    };
    let BlockTerminator::Return {
        id: true_return,
        result: ReturnEdge::Scalar(true_result),
        cleanup: ref true_discards,
    } = true_arm.terminator
    else {
        panic!("true arm must return its own scalar result");
    };
    assert!(false_discards.is_empty());
    assert!(true_discards.is_empty());
    assert_eq!(OperationResult::Scalar(false_result), body.operations[2].result);
    assert_eq!(OperationResult::Scalar(true_result), body.operations[4].result);
    assert_eq!(
        [
            when_false.id.get(),
            when_true.id.get(),
            false_return.get(),
            true_return.get()
        ],
        [1, 2, 3, 4]
    );
    assert_eq!(body.diagnostics.edges.len(), 4);
}

#[test]
fn both_structural_returns_share_the_machine_result_place() {
    let package = lower(
        "machine choose(flag: bool, left: [u8; 2], right: [u8; 2]) -> [u8; 2] {
            transition flag {true -> left, _ -> right,}
         }",
    )
    .unwrap();
    let body = &package.machines[0].body;
    let [_, Parameter::Structural(left), Parameter::Structural(right)] = *body.parameters.as_slice() else {
        panic!("expected two structural parameters");
    };
    let [_, true_arm, false_arm] = body.blocks.as_slice() else {
        panic!("expected both return branches");
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural {
            source: true_source,
            result: true_result,
        },
        ..
    } = true_arm.terminator
    else {
        panic!("expected a structural true return");
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural {
            source: false_source,
            result: false_result,
        },
        ..
    } = false_arm.terminator
    else {
        panic!("expected a structural false return");
    };
    assert_eq!((true_source, false_source), (left, right));
    assert_eq!(true_result, false_result);
    assert_eq!(body.places[true_result.index()].origin, PlaceOrigin::Result);
    assert_eq!(body.places.len(), 3);
}

#[test]
fn transition_forwards_completion_on_the_selected_terminal_call_edge() {
    let package = lower(
        "machine leaf(value: u8) -> u8 { value }
         machine choose(flag: bool, value: u8) -> u8 {
            transition flag {true -> (leaf(value +% 1)), false -> value,}
         }",
    )
    .unwrap();
    let [choose, leaf] = package.machines.as_slice() else {
        panic!("expected caller and callee")
    };
    let [_, terminal, _] = choose.body.blocks.as_slice() else {
        panic!("expected both terminal blocks")
    };
    let BlockTerminator::TerminalCall { callee: omega_lowering::CallTarget::Machine(callee), arguments, .. } = &terminal.terminator else {
        panic!("expected a terminal call edge");
    };
    assert_eq!(*callee, leaf.id);
    assert_eq!(terminal.operations, 0..2);
    let OperationResult::Scalar(value) = choose.body.operations[1].result else {
        panic!("expected scalar argument operation")
    };
    assert_eq!(*arguments, [CallArgument::Scalar(value)]);
}

#[test]
fn nested_tail_matches_keep_successor_identities_after_another_arm_is_queued() {
    let package = lower(
        "machine choose(first: bool, second: bool) -> u8 {
            match first { false -> (match second {true -> 1, _ -> 2,}), true -> 3,}
         }",
    )
    .unwrap();
    let [outer, inner, otherwise, positive, negative] = package.machines[0].body.blocks.as_slice() else {
        panic!("expected both conditions and three returning arms");
    };
    let BlockTerminator::Conditional { when_true, when_false, .. } = &outer.terminator else {
        panic!("expected outer condition")
    };
    assert_eq!(when_true.target, otherwise.id);
    assert_eq!(when_false.target, inner.id);
    let BlockTerminator::Conditional { when_true, when_false, .. } = &inner.terminator else {
        panic!("expected nested condition")
    };
    assert_eq!(when_true.target, positive.id);
    assert_eq!(when_false.target, negative.id);
    assert_eq!([outer, inner, otherwise, positive, negative].map(|block| block.id.get()), [1, 2, 3, 4, 5]);
}

#[test]
fn match_argument_joins_before_the_next_argument_and_call() {
    let package = lower(
        "machine pair(left: u8, right: u8) -> u8 { left +% right }
         machine choose(flag: bool, left: u8, right: u8) -> u8 {
             pair(left +% 1u8, (match flag {false -> left +% 2u8, true -> right +% 3u8,}) +% 4u8)
         }",
    )
    .unwrap();
    let [choose, pair] = package.machines.as_slice() else {
        panic!("expected the caller and its callee")
    };
    let body = &choose.body;
    let [entry, negative, positive, join] = body.blocks.as_slice() else {
        panic!("expected a branch with two arms and one join")
    };
    assert_eq!(entry.operations, 0..2);
    assert_eq!(negative.operations, 2..4);
    assert_eq!(positive.operations, 4..6);
    assert_eq!(join.operations, 6..9);
    let [parameter] = *join.scalar_parameters.as_slice() else {
        panic!("expected one scalar join parameter")
    };
    assert_eq!(body.values[parameter.index()].origin, omega_lowering::ValueOrigin::BlockParameter(join.id));
    for (arm, value_index) in [(negative, 3), (positive, 5)] {
        let BlockTerminator::Jump(edge) = &arm.terminator else {
            panic!("expected an arm edge to the join")
        };
        let OperationResult::Scalar(value) = body.operations[value_index].result else {
            panic!("expected the arm value")
        };
        assert_eq!(edge.target, join.id);
        assert_eq!(edge.arguments, [value]);
        assert_eq!(body.values[value.index()].ty, body.values[parameter.index()].ty);
    }
    let omega_lowering::OperationKind::Integer { operands, .. } = &body.operations[7].kind else {
        panic!("expected the operation consuming the match result")
    };
    assert_eq!(operands[0], parameter);
    let omega_lowering::OperationKind::Call { callee: omega_lowering::CallTarget::Machine(callee), arguments, .. } = &body.operations[8].kind else {
        panic!("expected the outer call")
    };
    assert_eq!(*callee, pair.id);
    let [OperationResult::Scalar(left), OperationResult::Scalar(right)] = [body.operations[1].result, body.operations[7].result] else {
        panic!("expected the outer operands")
    };
    assert_eq!(*arguments, [CallArgument::Scalar(left), CallArgument::Scalar(right)]);
}

#[test]
fn boolean_connectives_only_execute_the_selected_right_operand() {
    let package = lower(
        "machine choose(left: bool, value: u8) -> bool { left && positive(value) || fallback(value) }
         machine positive(value: u8) -> bool { value > 0u8 }
         machine fallback(value: u8) -> bool { value == 0u8 }",
    )
    .unwrap();
    let [choose, fallback, positive] = package.machines.as_slice() else {
        panic!("expected caller and two callees")
    };
    let body = &choose.body;
    let [entry, and_right, and_join, or_right, or_join] = body.blocks.as_slice() else {
        panic!("expected one conditional split per connective")
    };
    let [Parameter::Scalar(left), _] = *body.parameters.as_slice() else {
        panic!("expected a scalar left condition")
    };
    let [and_value] = *and_join.scalar_parameters.as_slice() else {
        panic!("expected the and result parameter")
    };
    let [or_value] = *or_join.scalar_parameters.as_slice() else {
        panic!("expected the or result parameter")
    };
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &entry.terminator
    else {
        panic!("expected the and split")
    };
    assert_eq!(*condition, left);
    assert_eq!((when_true.target, when_false.target), (and_right.id, and_join.id));
    assert_eq!(when_false.arguments, [left]);
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &and_join.terminator
    else {
        panic!("expected the or split")
    };
    assert_eq!(*condition, and_value);
    assert_eq!((when_true.target, when_false.target), (or_join.id, or_right.id));
    assert_eq!(when_true.arguments, [and_value]);
    for (right, join, expected) in [
        (and_right, and_join, positive.id),
        (or_right, or_join, fallback.id),
    ] {
        let [operation] = &body.operations[right.operations.clone()] else {
            panic!("expected the one right operand call")
        };
        let omega_lowering::OperationKind::Call { callee: omega_lowering::CallTarget::Machine(callee), .. } = &operation.kind else {
            panic!("expected a call")
        };
        assert_eq!(*callee, expected);
        let BlockTerminator::Jump(edge) = &right.terminator else {
            panic!("expected the right operand edge")
        };
        let OperationResult::Scalar(value) = operation.result else {
            panic!("expected a scalar call result")
        };
        assert_eq!(edge.target, join.id);
        assert_eq!(edge.arguments, [value]);
    }
    assert!(matches!(or_join.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(value), .. } if value == or_value));
}

#[test]
fn nested_match_joins_only_feed_their_dominating_outer_arm() {
    let package = lower(
        "machine choose(outer: bool, inner: bool, value: u8) -> u8 {
             (value +% 1u8) +% (match outer {
                 true -> value +% (match inner {true -> 2u8, false -> 3u8,}),
                 false -> value +% 4u8,
             })
         }",
    )
    .unwrap();
    let body = &package.machines[0].body;
    let [
        entry,
        outer_true,
        outer_false,
        outer_join,
        inner_true,
        inner_false,
        inner_join,
    ] = body.blocks.as_slice()
    else {
        panic!("expected two splits and two distinct joins");
    };
    let [outer_parameter] = *outer_join.scalar_parameters.as_slice() else {
        panic!("expected an outer parameter")
    };
    let [inner_parameter] = *inner_join.scalar_parameters.as_slice() else {
        panic!("expected an inner parameter")
    };
    for (split, when_true, when_false) in [
        (entry, outer_true, outer_false),
        (outer_true, inner_true, inner_false),
    ] {
        let BlockTerminator::Conditional {
            when_true: positive,
            when_false: negative,
            ..
        } = &split.terminator
        else {
            panic!("expected a Boolean split");
        };
        assert_eq!((positive.target, negative.target), (when_true.id, when_false.id));
    }
    for (arm, join) in [
        (inner_true, inner_join),
        (inner_false, inner_join),
        (inner_join, outer_join),
        (outer_false, outer_join),
    ] {
        let BlockTerminator::Jump(edge) = &arm.terminator else {
            panic!("expected an arm jump")
        };
        assert_eq!(edge.target, join.id);
        assert!(matches!(body.operations[arm.operations.end - 1].result, OperationResult::Scalar(value) if edge.arguments == [value]));
    }
    let omega_lowering::OperationKind::Integer { operands, .. } = &body.operations[inner_join.operations.start].kind else {
        panic!("expected the operation consuming the inner value");
    };
    assert_eq!(operands[1], inner_parameter);
    let omega_lowering::OperationKind::Integer { operands, .. } = &body.operations[outer_join.operations.start].kind else {
        panic!("expected the operation consuming the outer value");
    };
    assert_eq!(operands[1], outer_parameter);
    assert_eq!(body.operations[entry.operations.end - 1].result, OperationResult::Scalar(operands[0]));
}

#[test]
fn subjectless_guards_evaluate_in_order_and_only_reach_the_selected_continuation() {
    let source = "machine probe(value:bool)->bool{value}
        machine choose(a:bool,b:bool,x:u8,y:u8,z:u8)->u8{
            1u8 +% match { a -> x, probe(b) -> y, _ -> z, }
        }";
    let package = lower(source).unwrap();
    let [choose, probe] = package.machines.as_slice() else {
        panic!("expected caller and guard")
    };
    let body = &choose.body;
    let [
        Parameter::Scalar(a),
        Parameter::Scalar(b),
        Parameter::Scalar(x),
        Parameter::Scalar(y),
        Parameter::Scalar(z),
    ] = body.parameters[..]
    else {
        panic!("expected scalar parameters")
    };
    let entry = &body.blocks[0];
    let BlockTerminator::Conditional {
        condition,
        when_true: first,
        when_false: next,
    } = &entry.terminator
    else {
        panic!("expected first guard")
    };
    assert_eq!(*condition, a);
    let remaining = &body.blocks[next.target.get() - 1];
    let [call] = &body.operations[remaining.operations.clone()] else {
        panic!("expected guard call after first guard failed")
    };
    assert!(matches!(&call.kind,omega_lowering::OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==probe.id && arguments==&[CallArgument::Scalar(b)]));
    let BlockTerminator::Conditional {
        condition,
        when_true: second,
        when_false: last,
    } = &remaining.terminator
    else {
        panic!("expected second guard")
    };
    assert_eq!(call.result, OperationResult::Scalar(*condition));
    let arrivals = [(first.target, x), (second.target, y), (last.target, z)].map(|(target, expected)| {
        let BlockTerminator::Jump(edge) = &body.blocks[target.get() - 1].terminator else {
            panic!("expected join")
        };
        assert_eq!(edge.arguments, [expected]);
        edge.target
    });
    assert_eq!(arrivals, [arrivals[0]; 3]);
    let join = &body.blocks[arrivals[0].get() - 1];
    let [sum] = &body.operations[join.operations.clone()] else {
        panic!("expected continuation")
    };
    assert!(matches!(&sum.kind,omega_lowering::OperationKind::Integer{operands,..} if operands[1]==join.scalar_parameters[0]));
}

#[test]
fn subjectless_wildcard_forwards_the_terminal_call_without_a_condition() {
    let package = lower("machine leaf(x:u8)->u8{x} machine choose(x:u8)->u8{transition { _ -> leaf(x), }}").unwrap();
    let [choose, leaf] = package.machines.as_slice() else {
        panic!("expected two machines")
    };
    let [Parameter::Scalar(x)] = choose.body.parameters[..] else {
        panic!("expected a scalar parameter")
    };
    assert!(choose.body.operations.is_empty());
    assert!(choose.body.blocks.iter().any(
        |block| matches!(&block.terminator,BlockTerminator::TerminalCall{callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==leaf.id && arguments==&[CallArgument::Scalar(x)])
    ));
}

#[test]
fn a_constant_and_false_guard_keep_their_boolean_carrier_in_an_integer_machine() {
    let package = lower("const READY:bool=false; machine choose()->u8{match { false -> 1u8, READY -> 2u8, true -> 3u8, }}").unwrap();
    let body = &package.machines[0].body;
    let branches = body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Conditional { condition, .. } => Some((block, condition)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(branches.len(), 2);
    for (block, condition) in branches {
        let [guard] = &body.operations[block.operations.clone()] else {
            panic!("expected one guard value")
        };
        assert_eq!(guard.result, OperationResult::Scalar(*condition));
        assert_eq!(
            guard.kind,
            omega_lowering::OperationKind::Constant(omega_lowering::ScalarLiteral::Boolean(false))
        );
    }
}

#[test]
fn a_unit_guard_dispatch_rejoins_before_the_following_call() {
    let package = lower("machine act(){} machine after(){} machine choose(flag:bool){match {flag -> act(), _ -> act(),}; after();}").unwrap();
    let [act, after, choose] = package.machines.as_slice() else {
        panic!("expected three machines")
    };
    let body = &choose.body;
    let BlockTerminator::Conditional { when_true, when_false, .. } = &body.blocks[0].terminator else {
        panic!("expected guarded dispatch")
    };
    let joins = [when_true, when_false].map(|edge| {
        let arm = &body.blocks[edge.target.get() - 1];
        let [call] = &body.operations[arm.operations.clone()] else {
            panic!("expected a unit call")
        };
        assert!(matches!(call.kind,omega_lowering::OperationKind::Call {callee: omega_lowering::CallTarget::Machine(callee),..} if callee==act.id));
        let BlockTerminator::Jump(next) = &arm.terminator else {
            panic!("expected continuation")
        };
        assert!(next.arguments.is_empty() && next.structural_arguments.is_empty());
        next.target
    });
    spec_corpus::observe_handoff!(
        psi "psi-output/guard-join",
        assert_eq!(joins[0], joins[1]),
    );
    let join = &body.blocks[joins[0].get() - 1];
    assert!(join.scalar_parameters.is_empty() && join.structural_parameters.is_empty());
    let [call] = &body.operations[join.operations.clone()] else {
        panic!("expected following call")
    };
    assert!(matches!(call.kind,omega_lowering::OperationKind::Call {callee: omega_lowering::CallTarget::Machine(callee),..} if callee==after.id));
}

#[test]
fn integer_value_pattern_selects_by_exact_typed_equality() {
    let source = "machine choose(value: u8) -> u8 { match value {0u8 -> 1u8, _ -> 2u8,} }";
    use omega_lowering::{ArithmeticPolicy, IntegerPrimitive, OperationKind, ScalarLiteral, ScalarType, TypeShape};
    let package = lower(source).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Scalar(subject)] = body.parameters[..] else {
        panic!("expected one integer")
    };
    let entry = &body.blocks[0];
    let [zero, compare] = &body.operations[entry.operations.clone()] else {
        panic!("expected constant and equality")
    };
    let OperationResult::Scalar(pattern) = zero.result else {
        panic!("expected scalar pattern")
    };
    assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    assert_eq!(body.values[subject.index()].ty, body.values[pattern.index()].ty);
    assert_eq!(
        package.types.get(body.values[pattern.index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 }))
    );
    let OperationKind::Integer { operation, operands } = &compare.kind else {
        panic!("expected exact equality")
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/integer-pattern-equality",
        assert_eq!((operation.primitive, operation.policy, operands.as_slice()), (IntegerPrimitive::Equal, ArithmeticPolicy::Exact, [subject, pattern].as_slice())),
    );
    assert!(operation.obligations.is_empty());
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &entry.terminator
    else {
        panic!("expected conditional successors")
    };
    assert_eq!(compare.result, OperationResult::Scalar(*condition));
    assert_eq!(
        package.types.get(body.values[condition.index()].ty),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Boolean))
    );
    for (edge, expected) in [(when_true, 1), (when_false, 2)] {
        let arm = &body.blocks[edge.target.get() - 1];
        let [result] = &body.operations[arm.operations.clone()] else {
            panic!("expected arm constant")
        };
        assert_eq!(result.kind, OperationKind::Constant(ScalarLiteral::Integer(expected.into())));
        assert!(matches!(arm.terminator, BlockTerminator::Return {result:ReturnEdge::Scalar(value), ..} if OperationResult::Scalar(value)==result.result));
    }
}

#[test]
fn control_flow_invariant_detects_unowned_and_multiply_owned_operations() {
    let mut package = lower("machine choose(flag:bool)->u8{match flag {true -> 7u8, false -> 9u8,}}").unwrap();
    let body = &mut package.machines[0].body;
    let original = body.operations[1].id;
    body.operations[1].id = body.operations[0].id;
    assert_eq!(
        sources::control_flow::check_operation_ownership(body),
        Err("operation identity disagrees with its slot")
    );
    body.operations[1].id = original;
    let original = body.operations[1].result;
    body.operations[1].result = body.operations[0].result;
    assert_eq!(
        sources::control_flow::check_operation_ownership(body),
        Err("result does not name its defining operation")
    );
    body.operations[1].result = original;
    let first = body.blocks[1].operations.clone();
    let second = body.blocks[2].operations.clone();
    body.blocks[2].operations = first;
    assert_eq!(
        sources::control_flow::check_operation_ownership(body),
        Err("operation belongs to multiple reachable blocks")
    );
    body.blocks[2].operations = second.start..second.start;
    assert_eq!(
        sources::control_flow::check_operation_ownership(body),
        Err("operation belongs to no reachable block")
    );
    body.blocks[2].operations = second;
    let BlockTerminator::Conditional { when_true, when_false, .. } = &mut body.blocks[0].terminator else {
        panic!("expected conditional entry")
    };
    when_false.target = when_true.target;
    assert_eq!(sources::control_flow::check_operation_ownership(body), Err("block has no path from the entry"));
}

#[test]
fn a_unit_result_cannot_define_a_scalar_or_structural_result_slot() {
    let mut package = lower("data Cell{v:u8;} machine make()->Cell{Cell{v:7u8}}").unwrap();
    let body = &mut package.machines[0].body;
    let scalar = body.operations.iter().position(|op| matches!(op.result, OperationResult::Scalar(_))).expect("field initializer");
    let place = body.operations.iter().position(|op| matches!(op.result, OperationResult::Structural(_))).expect("constructed cell");
    for (index, diagnostic) in [
        (scalar, "scalar origin names an operation that does not return that value"),
        (place, "place origin names an operation that does not return that place"),
    ] {
        let result = std::mem::replace(&mut body.operations[index].result, OperationResult::Unit);
        assert_eq!(sources::control_flow::check_operation_ownership(body), Err(diagnostic));
        body.operations[index].result = result;
    }
    assert_eq!(sources::control_flow::check_operation_ownership(body), Ok(()));
}

/// `keep` returns a record that stores a view at its region, chosen by a
/// runtime Boolean. Typing puts one `ReturnBorrow` row on the tail `match`,
/// because either arm's record may hold the caller's loan.
const RETURNED_RECORD: &str = "data Schema<'t> [copy] { storage: @'t [u8]; }
data Held<'t> [copy] { schema: Schema<'t>; }
machine keep<'t>(first: bool, left: Schema<'t>, right: Schema<'t>) -> Held<'t> {
    match first { true -> Held::<'t>{ schema: left }, false -> Held::<'t>{ schema: right }, }
}
";

/// Each arm of a returned `match` keeps its own borrow and its own return
/// edge. The body branches on the formal and selects no arm. The `true` block
/// constructs its record from `left` and returns that place; the `false` block
/// does the same from `right`. Nothing joins the two records, so the place a
/// return edge names leads to the one view that arm stored.
#[test]
fn each_arm_of_a_returned_match_keeps_its_own_borrow_and_return_edge() {
    use omega_lowering::{OperationKind, ValueOperand};
    let files = [("branches.omg", RETURNED_RECORD)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let rows: Vec<_> = prepared
        .typed()
        .obligations
        .iter()
        .filter(|o| matches!(&o.kind, omega_typed::ObligationKind::ReturnBorrow { machine } if machine.path == "keep"))
        .collect();
    let [row] = rows.as_slice() else {
        panic!("expected one returned borrow: {rows:?}")
    };
    let whole_match = "match first { true -> Held::<'t>{ schema: left }, false -> Held::<'t>{ schema: right }, }";
    assert_eq!(row.site.span.slice(RETURNED_RECORD.as_bytes()), Some(whole_match.as_bytes()));
    let package = sources::lower_prepared(&prepared, &files).unwrap();
    let body = &package.machines[0].body;
    let [
        Parameter::Scalar(first),
        Parameter::Structural(left),
        Parameter::Structural(right),
    ] = *body.parameters.as_slice()
    else {
        panic!("expected the Boolean and two records: {:?}", body.parameters);
    };
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &body.blocks[0].terminator
    else {
        panic!("expected a conditional entry: {:?}", body.blocks[0].terminator);
    };
    assert_eq!(*condition, first);
    assert!(body.blocks[0].operations.is_empty());
    let returned = |target: omega_lowering::BlockId| {
        let block = &body.blocks[target.get() - 1];
        let BlockTerminator::Return {
            result: ReturnEdge::Structural { source, result },
            ..
        } = block.terminator
        else {
            panic!("expected a structural return: {:?}", block.terminator);
        };
        let PlaceOrigin::Operation(formed) = body.places[source.index()].origin else {
            panic!("the arm returns a place it did not form: {:?}", body.places[source.index()]);
        };
        assert!(block.operations.contains(&formed.index()), "the record is formed in its own arm");
        let OperationKind::EstablishRecord { fields } = &body.operations[formed.index()].kind else {
            panic!("expected a record construction: {:?}", body.operations[formed.index()].kind);
        };
        let [field] = fields.as_slice() else {
            panic!("expected the one stored record")
        };
        (field.operand.clone(), result)
    };
    let (true_view, true_result) = returned(when_true.target);
    let (false_view, false_result) = returned(when_false.target);
    assert_eq!((true_view, false_view), (ValueOperand::CopyWhole(left), ValueOperand::CopyWhole(right)));
    assert_eq!(true_result, false_result);
    assert_eq!(body.places[true_result.index()].origin, PlaceOrigin::Result);
    assert_eq!(sources::control_flow::check_operation_ownership(body), Ok(()));
}

#[test]
fn a_returned_match_arm_forwards_the_exact_borrow_carrying_formal() {
    let source = RETURNED_RECORD.replace(
        "keep<'t>(first: bool, left: Schema<'t>, right: Schema<'t>) -> Held<'t> {\n    match first { true -> Held::<'t>{ schema: left }, false -> Held::<'t>{ schema: right }, }",
        "keep<'t>(first: bool, left: Schema<'t>, held: Held<'t>) -> Held<'t> {\n    match first { true -> Held::<'t>{ schema: left }, false -> held, }",
    );
    assert_ne!(source, RETURNED_RECORD);
    let package = lower(&source).unwrap();
    let body = &package.machines[0].body;
    let [_, _, Parameter::Structural(held)] = *body.parameters.as_slice() else {
        panic!("expected the held record formal");
    };
    let BlockTerminator::Conditional { when_false, .. } = &body.blocks[0].terminator else {
        panic!("expected a conditional return");
    };
    let arm = &body.blocks[when_false.target.get() - 1];
    let BlockTerminator::Return { result: ReturnEdge::Structural { source, result }, .. } = arm.terminator else {
        panic!("expected a structural return");
    };
    assert_eq!(source, held);
    assert_eq!(body.places[result.index()].origin, PlaceOrigin::Result);
    assert!(arm.operations.is_empty());
    assert_eq!(sources::control_flow::check_operation_ownership(body), Ok(()));
}

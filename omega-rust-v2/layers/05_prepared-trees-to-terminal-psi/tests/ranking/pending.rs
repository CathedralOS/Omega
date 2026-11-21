use super::{formal, lower, machine, ordinary_rank_actual, ranking};
use omega_lowering::{
    ArithmeticPolicy, BlockId, BlockTerminator, CallArgument, CallRequirement, CallSite, CallTarget, IntegerObligation, IntegerPrimitive, LoweredMachine,
    OperationKind, PredicateOperator, PredicateStep, RankComparison, RankExpression, RankOperand, ScalarLiteral, ValueId, ValueOrigin,
};

fn definition(
    machine: &LoweredMachine,
    value: ValueId,
) -> &OperationKind {
    let ValueOrigin::Operation(op) = machine.body.values[value.index()].origin else {
        panic!("expected an operation result")
    };
    &machine.body.operations[op.index()].kind
}

/// Recover the branch predicate from the CFG, not from the ranking claim.
fn guard(
    machine: &LoweredMachine,
    target: BlockId,
    taken: bool,
) -> &OperationKind {
    let mut arrivals = machine
        .body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            } if (if taken { when_true } else { when_false }).target == target && when_true.target != when_false.target => Some(*condition),
            _ => None,
        });
    let condition = arrivals.next().expect("guard for the recursive call");
    assert!(arrivals.next().is_none());
    definition(machine, condition)
}

fn assert_literal(
    machine: &LoweredMachine,
    value: ValueId,
    expected: u64,
) {
    assert_eq!(definition(machine, value), &OperationKind::Constant(ScalarLiteral::Integer(expected.into())));
}

#[test]
fn parameter_step_retains_entry_premise_call_requirement_and_exact_subtraction() {
    // With step == 0 there is no descent; the same raw comparison still belongs
    // to the checker. Only the literal decrement has an explicit strict claim.
    for (required, rhs, comparison) in [
        (1, "step", RankComparison::Pending),
        (0, "step", RankComparison::Pending),
        (1, "1", RankComparison::Strict),
    ] {
        let source = format!(
            "machine count(n:u64,step:u64)->u64 requires step == {required} terminates by n {{
            match n == 0 {{ true -> 7, false -> count(n - {rhs},step), }}
        }}"
        );
        let package = lower(&source).unwrap();
        let count = machine(&package, "count");
        let (n, step) = (formal(count, 0), formal(count, 1));
        let [premise] = count.contract.requires.as_slice() else {
            panic!("expected entry premise")
        };
        assert_eq!(
            premise.predicate,
            [
                PredicateStep::Formal(1),
                PredicateStep::Integer(required.into()),
                PredicateStep::Apply(PredicateOperator::Equal)
            ]
        );
        let witness = ranking(count);
        assert_eq!(witness.component, [count.id]);
        assert_eq!(
            witness.source_rank(&count.body.parameters),
            Some(RankExpression::Operand(RankOperand::Input(n)))
        );
        let next = ordinary_rank_actual(count, count, comparison);
        let OperationKind::Integer { operation, operands } = definition(count, next) else {
            panic!("expected subtraction result")
        };
        assert_eq!((operation.primitive, operation.policy), (IntegerPrimitive::Subtract, ArithmeticPolicy::Exact));
        assert_eq!(operation.obligations, [IntegerObligation::ResultFitsCarrier]);
        let [left, right] = operands[..] else {
            panic!("expected both subtraction operands")
        };
        assert_eq!(left, n);
        if rhs == "step" {
            assert_eq!(right, step);
        } else {
            assert_literal(count, right, 1);
        }

        let edge = &witness.edges[0];
        let CallSite::Operation(op) = edge.call else { panic!("expected ordinary call") };
        let OperationKind::Call { arguments, requirements, .. } = &count.body.operations[op.index()].kind else {
            panic!("expected call")
        };
        assert_eq!(*arguments, [CallArgument::Scalar(next), CallArgument::Scalar(step)]);
        assert_eq!(
            *requirements,
            [CallRequirement {
                site: CallSite::Operation(op),
                clause: 0
            }]
        );
        let OperationKind::Integer { operation, operands } = guard(count, edge.block, false) else {
            panic!("expected equality guard")
        };
        assert_eq!(operation.primitive, IntegerPrimitive::Equal);
        let [subject, zero] = operands[..] else { panic!("expected guard operands") };
        assert_eq!(subject, n);
        assert_literal(count, zero, 0);
    }
}

#[test]
fn a_false_recursive_requirement_remains_attached_to_the_actual_call() {
    // The entry premise describes the *caller*. Re-entry with zero violates
    // that same clause; emitting a rank edge must not discharge the call.
    let package = lower(
        "machine count(n:u64,step:u64)->u64 requires step == 1 terminates by n {
        match n == 0 {true -> 7, false -> count(n - step,0),}
    }",
    )
    .unwrap();
    let count = machine(&package, "count");
    let next = ordinary_rank_actual(count, count, RankComparison::Pending);
    let CallSite::Operation(call) = ranking(count).edges[0].call else { panic!("expected ordinary call") };
    let OperationKind::Call { arguments, requirements, .. } = &count.body.operations[call.index()].kind else {
        panic!("expected call")
    };
    let [CallArgument::Scalar(rank), CallArgument::Scalar(step)] = arguments[..] else {
        panic!("expected scalar actuals")
    };
    assert_eq!(rank, next);
    assert_literal(count, step, 0);
    assert_eq!(
        *requirements,
        [CallRequirement {
            site: CallSite::Operation(call),
            clause: 0
        }]
    );
    assert_eq!(
        count.contract.requires[0].predicate,
        [
            PredicateStep::Formal(1),
            PredicateStep::Integer(1.into()),
            PredicateStep::Apply(PredicateOperator::Equal)
        ]
    );
}

#[test]
fn terminal_rank_and_positive_guard_keep_their_distinct_zero_definitions() {
    let package = lower("machine count(n:u64)->u64 terminates by n {transition n > 0 {true -> count(0), false -> 7,}}").unwrap();
    let count = machine(&package, "count");
    let witness = ranking(count);
    assert_eq!(witness.component, [count.id]);
    let n = formal(count, 0);
    assert_eq!(
        witness.source_rank(&count.body.parameters),
        Some(RankExpression::Operand(RankOperand::Input(n)))
    );
    let [edge] = witness.edges.as_slice() else {
        panic!("expected recursive edge")
    };
    let BlockTerminator::TerminalCall { id, callee, arguments, .. } = &count.body.blocks[edge.block.get() - 1].terminator else {
        panic!("expected terminal call")
    };
    assert_eq!(
        (edge.call, *callee, edge.target, edge.forwarding_return),
        (CallSite::Edge(*id), CallTarget::Machine(count.id), count.id, None)
    );
    let [CallArgument::Scalar(actual)] = arguments[..] else {
        panic!("expected rank argument")
    };
    assert_eq!(
        (edge.comparison, &edge.successor),
        (RankComparison::Pending, &Some(RankExpression::Operand(RankOperand::Input(actual))))
    );
    let OperationKind::Integer { operation, operands } = guard(count, edge.block, true) else {
        panic!("expected positive guard")
    };
    assert_eq!(operation.primitive, IntegerPrimitive::GreaterThan);
    let [subject, zero] = operands[..] else { panic!("expected guard operands") };
    assert_eq!(subject, n);
    assert_ne!(actual, zero);
    assert_literal(count, zero, 0);
    assert_literal(count, actual, 0);
}

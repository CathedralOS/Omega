#[path = "support/source.rs"]
mod sources;

#[path = "ranking/pending.rs"]
mod pending;

use omega_lowering::{
    BlockTerminator, CallArgument, IntegerPrimitive, LoweredMachine, LoweredPackage, LoweringError, MachineRanking, OperationKind, OperationResult, Parameter,
    RankComparison, RankExpression, RankOrder, RankRelation, ScalarType, ValueId,
};

fn lower(source: &str) -> Result<LoweredPackage, LoweringError> {
    sources::lower(&[("ranking.omg", source)])
}

fn machine<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|m| m.declaration.path == path)
        .unwrap()
}

fn ranking(machine: &LoweredMachine) -> &MachineRanking {
    machine
        .contract
        .ranking
        .as_ref()
        .expect("authored ranking witness")
}

fn formal(
    machine: &LoweredMachine,
    index: usize,
) -> ValueId {
    let Parameter::Scalar(value) = machine.body.parameters[index] else {
        panic!("expected a scalar formal")
    };
    value
}

/// The result of the only integer operation with this primitive over `left`.
fn operation_result(
    machine: &LoweredMachine,
    primitive: IntegerPrimitive,
    left: ValueId,
) -> ValueId {
    let mut found = machine
        .body
        .operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::Integer { operation, operands }, OperationResult::Scalar(value))
                if operation.primitive == primitive && operands.first() == Some(&left) =>
            {
                Some(value)
            }
            _ => None,
        });
    let value = found.next().expect("integer operation");
    assert!(found.next().is_none());
    value
}

const NATURAL_U64: RankRelation = RankRelation {
    carrier: ScalarType::Integer { signed: false, width: 64 },
    order: RankOrder::NaturalDescending,
};

#[test]
fn a_self_countdown_claims_strict_descent_only_on_its_exact_internal_edge() {
    // The bare clause binds the canonical view; the twin names it explicitly.
    for view in ["", " -> omega\\language\\core\\Nat\\Descending"] {
        let package = lower(&format!(
            "machine leaf(x:u64)->u64 {{x}}
             machine count(n:u64)->u64 terminates by n{view} {{
                transition n == 0 {{true -> leaf(n), false -> count(n - 1),}}
             }}"
        ))
        .unwrap();
        countdown_claims_strict_descent(&package);
    }
}

fn countdown_claims_strict_descent(package: &LoweredPackage) {
    let count = machine(package, "count");
    let leaf = machine(package, "leaf");
    assert!(leaf.contract.ranking.is_none());
    let witness = ranking(count);
    assert_eq!(witness.view.package, "@omega/toolchain");
    assert_eq!(witness.view.path, "language\\core\\Nat\\Descending");
    assert_eq!(witness.relation, NATURAL_U64);
    assert_eq!(witness.rank, RankExpression::Operand(0));
    assert_eq!(witness.bounds, None);
    assert_eq!(witness.component, [count.id]);
    let n = formal(count, 0);
    assert_eq!(witness.source_rank(&count.body.parameters), Some(RankExpression::Operand(n)));
    let decremented = operation_result(count, IntegerPrimitive::Subtract, n);

    // Two terminal transfers exist; only the one back into the component is claimed.
    let terminals: Vec<_> = count
        .body
        .blocks
        .iter()
        .filter(|block| matches!(block.terminator, BlockTerminator::TerminalCall { .. }))
        .collect();
    assert_eq!(terminals.len(), 2);
    let [edge] = witness.edges.as_slice() else {
        panic!("expected one internal edge claim")
    };
    let block = count
        .body
        .blocks
        .iter()
        .find(|block| block.id == edge.block)
        .unwrap();
    let BlockTerminator::TerminalCall { id, callee, arguments, .. } = &block.terminator else {
        panic!("the claim must name a terminal call block")
    };
    assert_eq!((*id, *callee), (edge.edge, omega_lowering::CallTarget::Machine(count.id)));
    assert_eq!(edge.forwarded_call, None);
    assert_eq!(edge.target, count.id);
    assert_eq!(*arguments, [CallArgument::Scalar(decremented)]);
    assert_ne!(decremented, n);
    // The view decides the rank shape; the edge substitutes the actual argument.
    spec_corpus::observe_handoff!(
        "canonical-ranking-view/consumer",
        assert_eq!(
            (&edge.successor, &edge.comparison),
            (&RankExpression::Operand(decremented), &RankComparison::Strict)
        )
    );
}

#[test]
fn a_mutual_handoff_preserves_while_the_return_edge_descends() {
    let package = lower(
        "machine first(n:u64)->u64 terminates by n {transition {_ -> second(n),}}
         machine second(n:u64)->u64 terminates by n {
            transition n == 0 {true -> 9, false -> first(n - 1),}
         }",
    )
    .unwrap();
    let first = machine(&package, "first");
    let second = machine(&package, "second");
    let mut members = vec![first.id, second.id];
    members.sort_unstable_by_key(|id| id.get());
    for member in [first, second] {
        assert_eq!(ranking(member).component, members);
        assert_eq!(ranking(member).relation, NATURAL_U64);
    }
    let [handoff] = ranking(first).edges.as_slice() else {
        panic!("expected the handoff claim")
    };
    assert_eq!(handoff.target, second.id);
    assert_eq!(handoff.successor, RankExpression::Operand(formal(first, 0)));
    assert_eq!(handoff.comparison, RankComparison::Preserving);
    let [back] = ranking(second).edges.as_slice() else {
        panic!("expected the return claim")
    };
    let n = formal(second, 0);
    assert_eq!(back.target, first.id);
    assert_eq!(back.successor, RankExpression::Operand(operation_result(second, IntegerPrimitive::Subtract, n)));
    assert_eq!(back.comparison, RankComparison::Strict);
}

#[test]
fn a_bounded_distance_witness_substitutes_both_actual_successor_arguments() {
    let package = lower(
        "machine climb(i:u64, limit:u64)->u64 terminates by (i, limit) -> omega\\language\\core\\Nat\\BoundedDistance in 0..=256 {
            transition i < limit {true -> climb(i + 1, limit), false -> i,}
         }",
    )
    .unwrap();
    let climb = machine(&package, "climb");
    let witness = ranking(climb);
    assert_eq!(witness.view.path, "language\\core\\Nat\\BoundedDistance");
    assert_eq!(witness.relation, NATURAL_U64);
    assert_eq!(witness.rank, RankExpression::SaturatingDistance { cursor: 0, limit: 1 });
    let bounds = witness.bounds.as_ref().unwrap();
    assert_eq!(
        (bounds.lower.clone(), bounds.upper.clone(), bounds.upper_inclusive),
        (0.into(), 256.into(), true)
    );
    let (i, limit) = (formal(climb, 0), formal(climb, 1));
    let [edge] = witness.edges.as_slice() else {
        panic!("expected one internal edge claim")
    };
    spec_corpus::observe_handoff!(
        "ranking-witnesses/consumer",
        assert_eq!(
            edge.successor,
            RankExpression::SaturatingDistance {
                cursor: operation_result(climb, IntegerPrimitive::Add, i),
                limit,
            }
        )
    );
    assert_eq!(edge.comparison, RankComparison::Strict);
}

#[test]
fn an_advancing_cursor_already_at_the_limit_does_not_decrease_distance() {
    let package = lower("machine climb(i:u64, limit:u64)->u64 terminates by (i, limit) -> omega\\language\\core\\Nat\\BoundedDistance {
        transition i == limit {true -> climb(i + 1, limit), false -> i,}
    }").unwrap();
    let climb = machine(&package, "climb");
    let [edge] = ranking(climb).edges.as_slice() else { panic!("expected one internal call") };
    assert_eq!(edge.successor, RankExpression::SaturatingDistance {
        cursor: operation_result(climb, IntegerPrimitive::Add, formal(climb, 0)), limit: formal(climb, 1),
    });
    assert_eq!(edge.comparison, RankComparison::Preserving);
}

#[test]
fn an_acyclic_witness_keeps_its_bounds_and_claims_no_edges() {
    for (range, inclusive, upper) in [("0..=9", true, 9), ("0..10", false, 10)] {
        let source = format!("machine visit(n:u64) terminates by n in {range} {{}}");
        let package = lower(&source).unwrap();
        let visit = machine(&package, "visit");
        let witness = ranking(visit);
        assert_eq!(witness.component, [visit.id]);
        assert!(witness.edges.is_empty());
        let bounds = witness.bounds.as_ref().unwrap();
        // Psi reads each endpoint from the literal destination L3 typed in the rank carrier.
        spec_corpus::observe_handoff!(
            "ranking-witnesses/consumer-2",
            assert_eq!(
                (bounds.lower.clone(), bounds.upper.clone(), bounds.upper_inclusive),
                (0.into(), upper.into(), inclusive)
            )
        );
        assert_eq!(bounds.site.span.slice(source.as_bytes()), Some(range.as_bytes()));
    }
}

#[test]
fn a_ranked_terminal_descriptor_call_refuses_at_the_invocation() {
    let source = "data Step = machine(x:u64)->u64 reaches none crashes none terminates;
        machine run(f:Step, n:u64)->u64 terminates {transition {_ -> f(n),}}";
    let package = lower(source).unwrap();
    let run = machine(&package, "run");
    assert!(run.contract.promises_termination && run.contract.ranking.is_none());
    let (edge, descriptor, contract, arguments) = run.body.blocks.iter().find_map(|block| match &block.terminator {
        BlockTerminator::TerminalCall { id, callee: omega_lowering::CallTarget::Runtime { descriptor, contract }, arguments, .. } => {
            Some((*id, *descriptor, *contract, arguments))
        }
        _ => None,
    }).expect("the terminal edge invokes the descriptor");
    assert_eq!(descriptor, formal(run, 0));
    assert_eq!(arguments.as_slice(), [CallArgument::Scalar(formal(run, 1))]);
    assert_eq!(package.types.get(run.parameter_types[0]), Some(&omega_lowering::TypeShape::Callable(contract)));
    assert!(package.callable_contracts[contract.index()].terminates);
    assert_eq!(run.body.diagnostics.edges[edge.get() - 1].span.slice(source.as_bytes()), Some(b"f(n)".as_slice()));

    // The contract and call are unchanged. Adding a private rank, not merely
    // promising termination, makes this otherwise acyclic caller refuse.
    let ranked = source.replace("terminates {transition", "terminates by n {transition");
    refuses_at(
        &ranked,
        sources::unique_span(&ranked, "f(n)"),
        "lowering cannot include an indirect terminal call in the caller's rank graph",
    );
}

#[test]
fn a_ranked_ordinary_descriptor_call_refuses_at_the_invocation() {
    let source = "data Step = machine(x:u64)->u64 reaches none crashes none terminates;
        machine run(f:Step,n:u64)->u64 terminates {let answer=f(n); answer}";
    let package = lower(source).unwrap();
    let run = machine(&package, "run");
    assert!(run.body.operations.iter().any(|operation| matches!(&operation.kind,
        omega_lowering::OperationKind::Call { callee: omega_lowering::CallTarget::Runtime { descriptor, contract }, .. }
            if *descriptor == formal(run, 0) && package.callable_contracts[contract.index()].terminates)));

    let ranked = source.replace("terminates {let", "terminates by n {let");
    refuses_at(
        &ranked,
        sources::unique_span(&ranked, "f(n)"),
        "lowering cannot include an indirect call in the caller's rank graph",
    );
}

#[test]
fn an_unranked_productive_cycle_stays_unranked() {
    let package = lower("machine spin(n:u64)->u64 {transition {_ -> spin(n),}}").unwrap();
    assert!(machine(&package, "spin").contract.ranking.is_none());
}

fn refuses_at(source: &str, span: std::ops::Range<usize>, reason: &str) {
    sources::refuse(&[("ranking.omg", source)], "ranking.omg", span, reason);
}

const ORDINARY: &str = "machine count(n:u64)->u64 terminates by n {match n == 0 {true -> 7, false -> count(n - 1),}}";

#[test]
fn ranked_ordinary_call_binds_the_call_operation_forwarding_return_and_actual_rank() {
    for (actual, comparison) in [("n - 1", RankComparison::Strict), ("n", RankComparison::Preserving)] {
        // The preserving twin cannot establish every-cycle descent. L5 still emits its claim.
        let source = ORDINARY.replace("n - 1", actual);
        let files = [("ranking.omg", source.as_str())];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let input = prepared.typed();
        let [witness] = input.ranking_witnesses.as_slice() else { panic!("expected count's witness") };
        let owner = &witness.machine;
        assert_eq!((owner.package.as_str(), owner.path.as_str()), ("app", "count"));
        assert_eq!(witness.view.path, "language\\core\\Nat\\Descending");
        let obligations: Vec<_> = input.obligations.iter().filter(|o| matches!(&o.kind, omega_typed::ObligationKind::Termination(m) if m == owner)).collect();
        let [obligation] = obligations.as_slice() else { panic!("expected one termination obligation") };
        assert_eq!(obligation.node, witness.node);
        assert_eq!(obligation.site.span.slice(source.as_bytes()), Some(b"terminates by n".as_slice()));
        let calls: Vec<_> = input.calls.iter().filter(|c| &c.machine == owner).collect();
        let [call] = calls.as_slice() else { panic!("expected one recursive call") };
        assert_eq!(call.caller.as_ref(), Some(owner));
        assert_eq!(call.use_site, omega_typed::CallUse::Execution);
        assert_eq!(call.site.span.slice(source.as_bytes()), Some(format!("count({actual})").as_bytes()));
        let [operand] = call.operands.as_slice() else { panic!("expected one actual") };
        assert_eq!(operand.site.span.slice(source.as_bytes()), Some(actual.as_bytes()));
        let sig = input.signatures.iter().find(|s| &s.machine == owner).unwrap();
        assert_eq!(sig.parameters, [(operand.parameter.span, witness.rank.clone())]);
        let subject = if actual == "n" { &operand.parameter } else { &operand.site };
        assert!(matches!(&operand.subject, omega_typed::ValueType::Subject { declaration, version: None } if declaration == subject), "{:?}", operand.subject);
        let (clause_site, call_site, actual_site) = (obligation.site.clone(), call.site.clone(), operand.site.clone());
        if actual == "n - 1" {
            assert!(input.obligations.iter().any(|o| o.site == actual_site && matches!(&o.kind, omega_typed::ObligationKind::OperationContract { token, .. } if token == "-")));
        }
        let package = sources::lower_prepared(&prepared, &files).unwrap();
        drop(prepared);
        drop(resolved);
        let count = machine(&package, "count");
        let w = ranking(count);
        assert!(count.contract.promises_termination);
        assert_eq!(w.site, clause_site);
        assert_eq!((w.view.package.as_str(), w.view.path.as_str()), ("@omega/toolchain", "language\\core\\Nat\\Descending"));
        assert_eq!(w.component, [count.id]);
        assert_eq!(w.relation, NATURAL_U64);
        let n = formal(count, 0);
        assert_eq!(w.source_rank(&count.body.parameters), Some(RankExpression::Operand(n)));
        let next = if actual == "n" { n } else {
            let value = operation_result(count, IntegerPrimitive::Subtract, n);
            let omega_lowering::ValueOrigin::Operation(def) = count.body.values[value.index()].origin else { panic!("expected subtraction") };
            assert_eq!(count.body.diagnostics.operations[def.index()], actual_site);
            let OperationKind::Integer { operation, operands } = &count.body.operations[def.index()].kind else { panic!("expected integer operation") };
            assert_eq!(operation.policy, omega_lowering::ArithmeticPolicy::Exact);
            assert_eq!(operation.obligations, [omega_lowering::IntegerObligation::ResultFitsCarrier]);
            let [left, right] = operands[..] else { panic!("expected two operands") };
            assert_eq!(left, n);
            let omega_lowering::ValueOrigin::Operation(one) = count.body.values[right.index()].origin else { panic!("expected a literal") };
            assert_eq!(count.body.operations[one.index()].kind, OperationKind::Constant(omega_lowering::ScalarLiteral::Integer(1.into())));
            value
        };
        let [edge] = w.edges.as_slice() else { panic!("expected only the in-component call") };
        spec_corpus::observe_handoff!(
            psi "psi-output/ordinary-rank-successor",
            assert_eq!((edge.target, &edge.successor, edge.comparison), (count.id, &RankExpression::Operand(next), comparison)),
        );
        let body = &count.body;
        let block = body.blocks.iter().find(|b| b.id == edge.block).unwrap();
        let op = body.operations[block.operations.clone()].last().unwrap();
        spec_corpus::observe_handoff!(psi "psi-output/ordinary-rank-call", assert_eq!(edge.forwarded_call, Some(op.id)));
        assert_eq!(body.diagnostics.operations[op.id.index()], call_site);
        assert!(matches!(&op.kind, OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), arguments, .. }
            if *target == edge.target && arguments.as_slice() == [CallArgument::Scalar(next)]));
        let BlockTerminator::Return { id, result: omega_lowering::ReturnEdge::Scalar(returned), cleanup } = &block.terminator else {
            panic!("expected a forwarding return")
        };
        assert_eq!(*id, edge.edge);
        assert_eq!(op.result, OperationResult::Scalar(*returned));
        assert_eq!(body.values[returned.index()].origin, omega_lowering::ValueOrigin::Operation(op.id));
        assert!(cleanup.is_empty());
        assert!(body.suspension_sites.is_empty());
    }
}

/// Check the claim against the actual forwarding call, not just the role tag.
fn ordinary_rank_actual(caller: &LoweredMachine, callee: &LoweredMachine, role: RankComparison) -> ValueId {
    let [edge] = ranking(caller).edges.as_slice() else { panic!("expected one internal call") };
    let RankExpression::Operand(slot) = ranking(callee).rank else { panic!("expected a scalar rank") };
    let block = &caller.body.blocks[edge.block.get() - 1];
    let call = caller.body.operations[block.operations.clone()].last().unwrap();
    let OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), arguments, .. } = &call.kind else { panic!("expected call") };
    let CallArgument::Scalar(actual) = arguments[slot] else { panic!("expected scalar actual") };
    assert_eq!(edge.forwarded_call, Some(call.id));
    assert_eq!((*target, edge.target, edge.comparison), (callee.id, callee.id, role));
    assert_eq!(edge.successor, RankExpression::Operand(actual));
    assert!(matches!(&block.terminator, BlockTerminator::Return { id, result: omega_lowering::ReturnEdge::Scalar(v), cleanup }
        if *id == edge.edge && call.result == OperationResult::Scalar(*v) && cleanup.is_empty()));
    actual
}

#[test]
fn ordinary_equal_sibling_preserves_and_the_countdown_back_edge_is_strict() {
    const BACK: &str = "machine second(dummy:u64,n:u64)->u64 terminates by n {match n == 0 {true -> 7, false -> first(n - 1,n - 1),}}";
    for (arguments, slot) in [("b,a", 0), ("a,b", 1)] {
        let first = format!("machine first(a:u64,b:u64)->u64 terminates by a {{match a == b {{true -> second({arguments}), false -> 7,}}}}");
        for source in [format!("{first}\n{BACK}"), format!("{BACK}\n{first}")] {
            let package = lower(&source).unwrap();
            let first = machine(&package, "first");
            let second = machine(&package, "second");
            assert_eq!(ranking(first).rank, RankExpression::Operand(0));
            assert_eq!(ranking(second).rank, RankExpression::Operand(1));
            assert_eq!(ordinary_rank_actual(first, second, RankComparison::Preserving), formal(first, slot));
            let guard = operation_result(first, IntegerPrimitive::Equal, formal(first, 0));
            let omega_lowering::ValueOrigin::Operation(eq) = first.body.values[guard.index()].origin else { panic!("expected equality") };
            assert!(matches!(&first.body.operations[eq.index()].kind, OperationKind::Integer { operands, .. }
                if operands.as_slice() == [formal(first, 0), formal(first, 1)]));
            assert!(first.body.blocks.iter().any(|block| matches!(&block.terminator,
                BlockTerminator::Conditional { condition, when_true, when_false } if *condition == guard
                    && when_true.target == ranking(first).edges[0].block && when_false.target != when_true.target)));
            let next = ordinary_rank_actual(second, first, RankComparison::Strict);
            let omega_lowering::ValueOrigin::Operation(def) = second.body.values[next.index()].origin else { panic!("expected subtraction") };
            assert!(matches!(&second.body.operations[def.index()].kind, OperationKind::Integer { operation, operands }
                if operation.primitive == IntegerPrimitive::Subtract && operation.policy == omega_lowering::ArithmeticPolicy::Exact
                    && operands[0] == formal(second, 1)));
        }
    }
}

#[test]
fn ordered_siblings_select_descent_only_on_the_ordered_arm() {
    for (guard, arms) in [
        ("a > b", "true -> count(b,a), false -> 7,"),
        ("b >= a", "false -> count(b,a), true -> 7,"),
    ] {
        let source = format!("machine count(a:u64,b:u64)->u64 terminates by a {{match {guard} {{{arms}}}}}");
        let package = lower(&source).unwrap();
        let count = machine(&package, "count");
        assert_eq!(ordinary_rank_actual(count, count, RankComparison::Strict), formal(count, 1));
    }

    // On these paths b may be greater than a. Neither role is justified
    // by the guard, and guessing Strict because this is a self-loop is wrong.
    for guard in ["a != b", "a < b", "a == c"] {
        let source = format!("machine count(a:u64,b:u64,c:u64)->u64 terminates by a {{match {guard} {{true -> count(b,a,c), false -> 7,}}}}");
        let package = lower(&source).unwrap();
        let count = machine(&package, "count");
        assert_eq!(ordinary_rank_actual(count, count, RankComparison::Pending), formal(count, 1));
    }
}

#[test]
fn a_strict_self_loop_does_not_supply_descent_on_a_parallel_preserving_cycle() {
    for arms in ["true -> count(b,a), false -> count(a - 1,b),", "false -> count(a - 1,b), true -> count(b,a),"] {
        let source = format!("machine count(a:u64,b:u64)->u64 terminates by a {{transition a == b {{{arms}}}}}");
        let package = lower(&source).unwrap();
        let count = machine(&package, "count");
        // count(1,1) repeats forever. L6 must see the preserving self-loop
        // after removing the strict one, independent of edge numbering.
        let edges = &ranking(count).edges;
        assert_eq!(edges.len(), 2);
        for (value, role) in [(formal(count, 1), RankComparison::Preserving),
            (operation_result(count, IntegerPrimitive::Subtract, formal(count, 0)), RankComparison::Strict)] {
            let edge = edges.iter().find(|e| e.successor == RankExpression::Operand(value)).unwrap();
            assert_eq!((edge.target, edge.comparison), (count.id, role));
            assert!(matches!(&count.body.blocks[edge.block.get() - 1].terminator, BlockTerminator::TerminalCall { id, arguments, .. }
                if *id == edge.edge && arguments[0] == CallArgument::Scalar(value)));
        }
    }
}

#[test]
fn ranked_ordinary_call_with_work_after_it_is_not_a_tail_transfer() {
    let source = ORDINARY.replace("count(n - 1),", "count(n - 1) + 1,");
    refuses_at(&source, sources::unique_span(&source, "count(n - 1)"), NON_TAIL);
}

const NON_TAIL: &str = "ranked recursion needs a terminal call or a direct cleanup-free scalar tail transfer";

#[test]
fn forwarding_the_call_result_with_cleanup_is_not_a_tail_transfer() {
    let source = "data Lease dropped by finish {mark:u8;}
        machine finish(x:Lease) {let Lease {mark as _}=x;}
        machine count(n:u64)->u64 terminates by n { let x=Lease {mark:0}; count(n - 1) }";
    refuses_at(source, sources::unique_span(source, "count(n - 1)"), NON_TAIL);
}

#[test]
fn unsupported_or_incomplete_ranked_components_refuse() {
    let ordinary = ORDINARY;

    // An ordinary call in Psi need not mean there is source work after it:
    // this match arm returns exactly the recursive call's result, without cleanup.
    let package = lower(&ordinary.replace(" terminates by n", "")).unwrap();
    let count = machine(&package, "count");
    assert!(count.body.blocks.iter().any(|block| {
        let BlockTerminator::Return { result: omega_lowering::ReturnEdge::Scalar(value), cleanup, .. } = &block.terminator else {
            return false;
        };
        cleanup.is_empty() && count.body.operations[block.operations.clone()].last().is_some_and(|op| {
            op.result == OperationResult::Scalar(*value)
                && matches!(&op.kind, OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), .. } if *target == count.id)
        })
    }));

    // Same ranking and actual, but a terminal transfer instead of call/return.
    let package = lower(&ordinary.replace("match", "transition")).unwrap();
    let count = machine(&package, "count");
    let [edge] = ranking(count).edges.as_slice() else {
        panic!("expected one in-component edge")
    };
    assert_eq!((edge.target, edge.comparison), (count.id, RankComparison::Strict));
    assert_eq!(edge.successor, RankExpression::Operand(operation_result(count, IntegerPrimitive::Subtract, formal(count, 0))));

    for (source, body, ranked_member, expected) in [
        (
            "machine first(n:u64)->u64 {transition {_ -> second(n),}}
             machine second(n:u64)->u64 terminates by n {transition n == 0 {true -> 9, false -> first(n - 1),}}",
            "{transition {_ -> second(n),}}",
            Some("second"),
            "lowering has no entry-rank mapping for this member of the recursive component",
        ),
        (
            "machine count(n:u64)->u64 terminates {transition n == 0 {true -> 7, false -> count(n - 1),}}",
            "{transition n == 0 {true -> 7, false -> count(n - 1),}}",
            None,
            "a cyclic terminating component requires a `terminates by` witness",
        ),
    ] {
        let files = [("ranking.omg", source)];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let ranked: Vec<_> = prepared.typed().ranking_witnesses.iter().map(|w| w.machine.path.as_str()).collect();
        assert_eq!(ranked, ranked_member.as_slice());
        sources::refuse_prepared(&prepared, &files, "ranking.omg", sources::unique_span(source, body), expected);
    }

    let constant = "const K:u64 = 3; machine f(n:u64) terminates by K {}";
    refuses_at(
        constant,
        sources::span_within(constant, "terminates by K", "K"),
        "lowering only represents rank operands that name machine formals",
    );
    let package = lower(&constant.replace("by K", "by n")).unwrap();
    let f = machine(&package, "f");
    let witness = ranking(f);
    assert_eq!(witness.relation, NATURAL_U64);
    assert_eq!(witness.rank, RankExpression::Operand(0));
    assert_eq!(witness.component, [f.id]);
    assert!(witness.edges.is_empty()); // Neither subject would owe a recursive comparison.
}

#[test]
fn selected_cleanup_participates_in_the_recursive_component() {
    let source = "data Lease dropped by finish {mark:u8;}
        machine again(n:u64) terminates by n { let lease=Lease {mark:0}; }
        machine finish(x:Lease) { let Lease {mark as _}=x; again(3); }";
    let files = [("ranking.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let input = prepared.typed();
    let [witness] = input.ranking_witnesses.as_slice() else { panic!("only again has an authored entry rank") };
    assert_eq!(witness.machine.path, "again");
    let [cleanup] = input.cleanups.as_slice() else { panic!("expected the selected cleanup") };
    assert_eq!((cleanup.owner.path.as_str(), cleanup.machine.path.as_str()), ("Lease", "finish"));
    assert!(input.calls.iter().any(|c| c.caller.as_ref() == Some(&cleanup.machine)
        && c.machine == witness.machine
        && c.site.span.slice(source.as_bytes()) == Some(b"again(3)".as_slice())));
    sources::refuse_prepared(
        &prepared,
        &files,
        "ranking.omg",
        sources::unique_span(source, "{ let Lease {mark as _}=x; again(3); }"),
        "lowering has no entry-rank mapping for this member of the recursive component",
    );

    // Removing the promise, not the cleanup, exposes both edges of the cycle.
    // Starting at again(3) reaches exactly again(3) after the handoff to finish.
    let package = lower(&source.replace(" terminates by n", "")).unwrap();
    let again = machine(&package, "again");
    let finish = machine(&package, "finish");
    assert!(again.body.blocks.iter().any(|block| matches!(&block.terminator,
        BlockTerminator::Return { cleanup, .. } if matches!(cleanup.as_slice(),
            [omega_lowering::CleanupAction::Invoke { target, ty, place }] if *target == finish.id
                && *ty == finish.parameter_types[0] && place.path.is_empty()))));
    let literal = finish.body.operations.iter().find(|op| op.kind == OperationKind::Constant(omega_lowering::ScalarLiteral::Integer(3.into())));
    let OperationResult::Scalar(three) = literal.expect("the rank at re-entry").result else {
        panic!("expected a scalar argument")
    };
    assert!(finish.body.operations.iter().any(|op| matches!(&op.kind,
        OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), arguments, .. }
            if *target == again.id && arguments.as_slice() == [CallArgument::Scalar(three)])));

    let ordinary = source.replace("again(3);", "");
    let package = lower(&ordinary).unwrap();
    let caller = machine(&package, "again");
    assert_eq!(ranking(caller).component, vec![caller.id]);
}

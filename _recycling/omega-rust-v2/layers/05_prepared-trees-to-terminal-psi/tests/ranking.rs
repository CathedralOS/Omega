#[path = "support/source.rs"]
mod sources;

#[path = "ranking/pending.rs"]
mod pending;

use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, IntegerPrimitive, LoweredMachine, LoweredPackage, LoweringError, MachineRanking, OperationKind, OperationResult, Parameter,
    RankComparison, RankExpression, RankOperand, RankOrder, RankRelation, ScalarType, ValueId,
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
    assert_eq!(witness.rank, RankExpression::Operand(RankOperand::Input(0)));
    assert_eq!(witness.bounds, None);
    assert_eq!(witness.component, [count.id]);
    let n = formal(count, 0);
    assert_eq!(witness.source_rank(&count.body.parameters), Some(RankExpression::Operand(RankOperand::Input(n))));
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
    assert_eq!((edge.call, *callee), (CallSite::Edge(*id), omega_lowering::CallTarget::Machine(count.id)));
    assert_eq!(edge.forwarding_return, None);
    assert_eq!(edge.target, count.id);
    assert_eq!(*arguments, [CallArgument::Scalar(decremented)]);
    assert_ne!(decremented, n);
    // The view decides the rank shape; the edge substitutes the actual argument.
    spec_corpus::observe_handoff!(
        "canonical-ranking-view/consumer",
        assert_eq!(
            (&edge.successor, &edge.comparison),
            (&Some(RankExpression::Operand(RankOperand::Input(decremented))), &RankComparison::Strict)
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
    assert_eq!(handoff.successor, Some(RankExpression::Operand(RankOperand::Input(formal(first, 0)))));
    assert_eq!(handoff.comparison, RankComparison::Preserving);
    let [back] = ranking(second).edges.as_slice() else {
        panic!("expected the return claim")
    };
    let n = formal(second, 0);
    assert_eq!(back.target, first.id);
    assert_eq!(back.successor, Some(RankExpression::Operand(RankOperand::Input(operation_result(second, IntegerPrimitive::Subtract, n)))));
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
    assert_eq!(witness.rank, RankExpression::SaturatingDistance { cursor: RankOperand::Input(0), limit: RankOperand::Input(1) });
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
            Some(RankExpression::SaturatingDistance {
                cursor: RankOperand::Input(operation_result(climb, IntegerPrimitive::Add, i)),
                limit: RankOperand::Input(limit),
            })
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
    assert_eq!(edge.successor, Some(RankExpression::SaturatingDistance {
        cursor: RankOperand::Input(operation_result(climb, IntegerPrimitive::Add, formal(climb, 0))), limit: RankOperand::Input(formal(climb, 1)),
    }));
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

/// The executable Psi of `run`: everything but source sites, which move when
/// a clause is added to the header.
fn executable(run: &LoweredMachine) -> impl PartialEq + std::fmt::Debug + '_ {
    let body = &run.body;
    (&body.blocks, &body.operations, &body.parameters, &body.places, &body.values, &body.suspension_sites, &body.call_acknowledgements)
}

/// A descriptor call is applied through its callable contract, not ranked.
/// Adding a private rank to the caller keeps the call exactly as the bare
/// promise lowered it and claims no edge for it: the descriptor's target is
/// outside every local component. The checker discharges the call's progress
/// from `callable_contracts[contract].terminates`; a contract without
/// `terminates` lowers the same way and gives that checker nothing to cite.
fn ranked_descriptor_call(
    unranked: &str,
    clause: &str,
    terminal: bool,
) -> (LoweredPackage, omega_lowering::CallableContractId) {
    let ranked = unranked.replace(clause, &clause.replace("terminates", "terminates by n"));
    assert_ne!(ranked, unranked);
    let promised = lower(unranked).unwrap();
    let package = lower(&ranked).unwrap();
    let (bare, run) = (machine(&promised, "run"), machine(&package, "run"));
    assert!(bare.contract.promises_termination && bare.contract.ranking.is_none());
    assert!(run.contract.promises_termination);
    assert_eq!(executable(run), executable(bare));
    let found: Vec<_> = if terminal {
        run.body.blocks.iter().filter_map(|block| match &block.terminator {
            BlockTerminator::TerminalCall { id, callee, arguments, .. } => {
                Some((callee, arguments, &run.body.diagnostics.edges[id.get() - 1]))
            }
            _ => None,
        }).collect()
    } else {
        run.body.operations.iter().filter_map(|op| match &op.kind {
            OperationKind::Call { callee, arguments, .. } => Some((callee, arguments, &run.body.diagnostics.operations[op.id.index()])),
            _ => None,
        }).collect()
    };
    let [(omega_lowering::CallTarget::Runtime { descriptor, contract }, arguments, site)] = found.as_slice() else {
        panic!("expected exactly one descriptor invocation: {found:?}")
    };
    assert_eq!(*descriptor, formal(run, 0));
    assert_eq!(arguments.as_slice(), [CallArgument::Scalar(formal(run, 1))]);
    assert_eq!(site.span.slice(ranked.as_bytes()), Some(b"f(n)".as_slice()));
    assert_eq!(package.types.get(run.parameter_types[0]), Some(&omega_lowering::TypeShape::Callable(*contract)));
    let witness = ranking(run);
    assert_eq!((witness.relation, &witness.rank), (NATURAL_U64, &RankExpression::Operand(RankOperand::Input(1))));
    let contract = *contract;

    // Negative twin: the same ranked caller and call through a descriptor that
    // promises no termination. Lowering still emits it; only the contract
    // the checker must cite differs, and that checker has to refuse.
    let silent = ranked.replace("crashes none terminates;", "crashes none;");
    assert_ne!(silent, ranked);
    let twin = lower(&silent).unwrap();
    let silent_run = machine(&twin, "run");
    assert!(ranking(silent_run).edges.is_empty());
    let omega_lowering::TypeShape::Callable(silent_contract) = twin.types.get(silent_run.parameter_types[0]).unwrap() else {
        panic!("expected a callable formal")
    };
    assert!(!twin.callable_contracts[silent_contract.index()].terminates);
    (package, contract)
}

#[test]
fn a_ranked_terminal_descriptor_call_stays_outside_the_component() {
    let (package, contract) = ranked_descriptor_call(
        "data Step = machine(x:u64)->u64 reaches none crashes none terminates;
        machine run(f:Step, n:u64)->u64 terminates {transition {_ -> f(n),}}",
        "terminates {transition",
        true,
    );
    let run = machine(&package, "run");
    let witness = ranking(run);
    spec_corpus::observe_handoff!(
        psi "psi-output/ranked-descriptor-terminal-call",
        assert_eq!(
            (witness.component.as_slice(), witness.edges.as_slice(), package.callable_contracts[contract.index()].terminates),
            ([run.id].as_slice(), [].as_slice(), true)
        ),
    );
}

#[test]
fn a_ranked_ordinary_descriptor_call_stays_outside_the_component() {
    let (package, contract) = ranked_descriptor_call(
        "data Step = machine(x:u64)->u64 reaches none crashes none terminates;
        machine run(f:Step,n:u64)->u64 terminates {let answer=f(n); answer}",
        "terminates {let",
        false,
    );
    let run = machine(&package, "run");
    let witness = ranking(run);
    spec_corpus::observe_handoff!(
        psi "psi-output/ranked-descriptor-ordinary-call",
        assert_eq!(
            (witness.component.as_slice(), witness.edges.as_slice(), package.callable_contracts[contract.index()].terminates),
            ([run.id].as_slice(), [].as_slice(), true)
        ),
    );
}

#[test]
fn a_descriptor_exit_from_a_ranked_cycle_claims_only_the_recursive_edge() {
    // The descriptor is forwarded on the recursive edge as an ordinary actual;
    // only that edge enters the component, and its rank substitutes the count.
    let source = "data Step = machine(x:u64)->u64 reaches none crashes none terminates;
        machine run(f:Step, n:u64)->u64 terminates by n {transition n == 0 {true -> f(n), false -> run(f, n - 1),}}";
    let package = lower(source).unwrap();
    let run = machine(&package, "run");
    let witness = ranking(run);
    assert_eq!(witness.component, [run.id]);
    let [edge] = witness.edges.as_slice() else { panic!("expected only the recursive edge") };
    let block = &run.body.blocks[edge.block.get() - 1];
    let BlockTerminator::TerminalCall { id, callee, arguments, .. } = &block.terminator else {
        panic!("expected the recursive terminal call")
    };
    let next = operation_result(run, IntegerPrimitive::Subtract, formal(run, 1));
    assert_eq!((edge.call, *callee), (CallSite::Edge(*id), omega_lowering::CallTarget::Machine(run.id)));
    assert_eq!(edge.forwarding_return, None);
    assert_eq!(*arguments, [CallArgument::Scalar(formal(run, 0)), CallArgument::Scalar(next)]);
    assert_eq!((edge.target, &edge.successor, edge.comparison), (run.id, &Some(RankExpression::Operand(RankOperand::Input(next))), RankComparison::Strict));
    let exits: Vec<_> = run.body.blocks.iter().filter(|block| matches!(&block.terminator,
        BlockTerminator::TerminalCall { callee: omega_lowering::CallTarget::Runtime { descriptor, .. }, arguments, .. }
            if *descriptor == formal(run, 0) && arguments.as_slice() == [CallArgument::Scalar(formal(run, 1))])).collect();
    assert_eq!(exits.len(), 1);
    assert_ne!(exits[0].id, edge.block);
}

#[test]
fn an_unranked_productive_cycle_stays_unranked() {
    let package = lower("machine spin(n:u64)->u64 {transition {_ -> spin(n),}}").unwrap();
    assert!(machine(&package, "spin").contract.ranking.is_none());
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
        assert_eq!(w.source_rank(&count.body.parameters), Some(RankExpression::Operand(RankOperand::Input(n))));
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
            assert_eq!((edge.target, &edge.successor, edge.comparison), (count.id, &Some(RankExpression::Operand(RankOperand::Input(next))), comparison)),
        );
        let body = &count.body;
        let block = body.blocks.iter().find(|b| b.id == edge.block).unwrap();
        let op = body.operations[block.operations.clone()].last().unwrap();
        spec_corpus::observe_handoff!(psi "psi-output/ordinary-rank-call", assert_eq!(edge.call, CallSite::Operation(op.id)));
        assert_eq!(body.diagnostics.operations[op.id.index()], call_site);
        assert!(matches!(&op.kind, OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), arguments, .. }
            if *target == edge.target && arguments.as_slice() == [CallArgument::Scalar(next)]));
        let BlockTerminator::Return { id, result: omega_lowering::ReturnEdge::Scalar(returned), cleanup } = &block.terminator else {
            panic!("expected a forwarding return")
        };
        assert_eq!(edge.forwarding_return, Some(*id));
        assert_eq!(op.result, OperationResult::Scalar(*returned));
        assert_eq!(body.values[returned.index()].origin, omega_lowering::ValueOrigin::Operation(op.id));
        assert!(cleanup.is_empty());
        assert!(body.suspension_sites.is_empty());
    }
}

/// Check the claim against the actual forwarding call, not just the role tag.
fn ordinary_rank_actual(caller: &LoweredMachine, callee: &LoweredMachine, role: RankComparison) -> ValueId {
    let [edge] = ranking(caller).edges.as_slice() else { panic!("expected one internal call") };
    let RankExpression::Operand(RankOperand::Input(slot)) = ranking(callee).rank else { panic!("expected a scalar rank") };
    let block = &caller.body.blocks[edge.block.get() - 1];
    let call = caller.body.operations[block.operations.clone()].last().unwrap();
    let OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), arguments, .. } = &call.kind else { panic!("expected call") };
    let CallArgument::Scalar(actual) = arguments[slot] else { panic!("expected scalar actual") };
    assert_eq!(edge.call, CallSite::Operation(call.id));
    assert_eq!((*target, edge.target, edge.comparison), (callee.id, callee.id, role));
    assert_eq!(edge.successor, Some(RankExpression::Operand(RankOperand::Input(actual))));
    assert!(matches!(&block.terminator, BlockTerminator::Return { id, result: omega_lowering::ReturnEdge::Scalar(v), cleanup }
        if edge.forwarding_return == Some(*id) && call.result == OperationResult::Scalar(*v) && cleanup.is_empty()));
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
            assert_eq!(ranking(first).rank, RankExpression::Operand(RankOperand::Input(0)));
            assert_eq!(ranking(second).rank, RankExpression::Operand(RankOperand::Input(1)));
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
            let edge = edges.iter().find(|e| e.successor == Some(RankExpression::Operand(RankOperand::Input(value)))).unwrap();
            assert_eq!((edge.target, edge.comparison), (count.id, role));
            assert!(matches!(&count.body.blocks[edge.block.get() - 1].terminator, BlockTerminator::TerminalCall { id, arguments, .. }
                if edge.call == CallSite::Edge(*id) && arguments[0] == CallArgument::Scalar(value)));
        }
    }
}

/// Check the prepared selections before inspecting L5's continuation and claim.
fn lower_countdown_continuation(source: &str, cleanup: bool) -> LoweredPackage {
    let files = [("ranking.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let [witness] = typed.ranking_witnesses.as_slice() else { panic!("expected count's rank") };
    assert_eq!((witness.machine.package.as_str(), witness.machine.path.as_str()), ("app", "count"));
    let clause = typed.obligations.iter().find(|o| o.node == witness.node && matches!(&o.kind,
        omega_typed::ObligationKind::Termination(m) if m == &witness.machine)).unwrap();
    let call = typed.calls.iter().find(|c| c.caller.as_ref() == Some(&witness.machine) && c.machine == witness.machine).unwrap();
    assert_eq!(call.use_site, omega_typed::CallUse::Execution);
    let [operand] = call.operands.as_slice() else { panic!("one countdown actual") };
    assert_eq!(operand.site.span.slice(source.as_bytes()), Some(b"n - 1".as_slice()));
    assert!(typed.obligations.iter().any(|o| o.site == operand.site && matches!(&o.kind,
        omega_typed::ObligationKind::OperationContract { token, .. } if token == "-")));
    if cleanup {
        let [selection] = typed.cleanups.as_slice() else { panic!("Lease's cleanup selection") };
        assert_eq!((selection.owner.path.as_str(), selection.machine.path.as_str()), ("Lease", "finish"));
        assert!(typed.obligations.iter().any(|o| matches!(&o.kind,
            omega_typed::ObligationKind::CleanupContract(m) if m == &selection.machine)));
    } else {
        assert!(typed.obligations.iter().any(|o| o.site.span.slice(source.as_bytes()) == Some(b"count(n - 1) + 1".as_slice()) && matches!(&o.kind,
            omega_typed::ObligationKind::OperationContract { token, .. } if token == "+")));
    }
    let package = sources::lower_prepared(&prepared, &files).unwrap();
    let count = machine(&package, "count");
    let (block, op, _) = countdown_call(count, source);
    assert_eq!(count.body.diagnostics.operations[op.id.index()], call.site);
    let rank = ranking(count);
    assert_eq!(rank.site, clause.site);
    assert_eq!(rank.component, [count.id]);
    assert_eq!(rank.relation, NATURAL_U64);
    assert_eq!(rank.source_rank(&count.body.parameters), Some(RankExpression::Operand(RankOperand::Input(formal(count, 0)))));
    assert_eq!(rank.edges[0].block, block.id);
    package
}

/// Locate the authored call from the body, independently of the ranking claim.
fn countdown_call<'a>(count: &'a LoweredMachine, source: &str) -> (&'a omega_lowering::BranchBlock, &'a omega_lowering::PsiOperation, ValueId) {
    let next = operation_result(count, IntegerPrimitive::Subtract, formal(count, 0));
    let site = sources::unique_span(source, "count(n - 1)");
    let body = &count.body;
    let mut calls = body.blocks.iter().flat_map(|block| body.operations[block.operations.clone()].iter().map(move |op| (block, op)))
        .filter(|(_, op)| matches!(&op.kind, OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), arguments, .. }
            if *target == count.id && arguments.as_slice() == [CallArgument::Scalar(next)]));
    let (block, op) = calls.next().expect("recursive call with the decremented actual");
    assert!(calls.next().is_none());
    assert_eq!(body.diagnostics.operations[op.id.index()].span.slice(source.as_bytes()), Some(&source.as_bytes()[site]));
    (block, op, next)
}

#[test]
fn ranked_ordinary_call_with_work_after_it_is_not_a_tail_transfer() {
    let source = ORDINARY.replace("count(n - 1),", "count(n - 1) + 1,");
    let package = lower_countdown_continuation(&source, false);
    let count = machine(&package, "count");
    let (block, call, next) = countdown_call(count, &source);
    let [claim] = ranking(count).edges.as_slice() else { panic!("one recursive invocation") };
    let OperationResult::Scalar(result) = call.result else { panic!("scalar call result") };
    let sum = operation_result(count, IntegerPrimitive::Add, result);
    let omega_lowering::ValueOrigin::Operation(add) = count.body.values[sum.index()].origin else { panic!("sum definition") };
    assert!(block.operations.contains(&add.index()) && call.id.index() < add.index());
    let OperationKind::Integer { operation, operands } = &count.body.operations[add.index()].kind else { panic!("addition") };
    let omega_lowering::ValueOrigin::Operation(one) = count.body.values[operands[1].index()].origin else { panic!("literal") };
    assert_eq!(count.body.operations[one.index()].kind, OperationKind::Constant(omega_lowering::ScalarLiteral::Integer(1.into())));
    assert_eq!(operation.policy, omega_lowering::ArithmeticPolicy::Exact);
    assert_eq!(operation.obligations, [omega_lowering::IntegerObligation::ResultFitsCarrier]);
    let BlockTerminator::Return { result: omega_lowering::ReturnEdge::Scalar(returned), cleanup, .. } = &block.terminator else { panic!("scalar return") };
    assert!(cleanup.is_empty());
    spec_corpus::observe_handoff!(psi "psi-output/rank-before-add", assert_eq!(
        (claim.block, claim.call, claim.forwarding_return, claim.target, &claim.successor, claim.comparison, *returned),
        (block.id, CallSite::Operation(call.id), None, count.id, &Some(RankExpression::Operand(RankOperand::Input(next))), RankComparison::Strict, sum)
    ));
    let tail = lower(ORDINARY).unwrap();
    ordinary_rank_actual(machine(&tail, "count"), machine(&tail, "count"), RankComparison::Strict);
}

#[test]
fn forwarding_the_call_result_with_cleanup_is_not_a_tail_transfer() {
    let source = "data Lease dropped by finish {mark:u8;}
        machine finish(x:Lease) {let Lease {mark as _}=x;}
        machine count(n:u64)->u64 terminates by n { let x=Lease {mark:0}; count(n - 1) }";
    let package = lower_countdown_continuation(source, true);
    let count = machine(&package, "count");
    let finish = machine(&package, "finish");
    let (block, call, next) = countdown_call(count, source);
    let [claim] = ranking(count).edges.as_slice() else { panic!("one in-component call, not finish") };
    assert_eq!(ranking(count).component, [count.id]);
    let BlockTerminator::Return { result: omega_lowering::ReturnEdge::Scalar(returned), cleanup, .. } = &block.terminator else { panic!("scalar return") };
    let [omega_lowering::CleanupAction::Invoke { place, ty, target }] = cleanup.as_slice() else { panic!("selected nominal cleanup") };
    let lease = count.body.operations[block.operations.start..call.id.index()].iter().find(|op| matches!(op.kind, OperationKind::EstablishRecord { .. })).unwrap();
    assert!(place.path.is_empty());
    assert_eq!(lease.result, OperationResult::Structural(place.root));
    spec_corpus::observe_handoff!(psi "psi-output/rank-before-cleanup", assert_eq!(
        (claim.block, claim.call, claim.forwarding_return, claim.target, &claim.successor, claim.comparison, call.result, *target, *ty),
        (block.id, CallSite::Operation(call.id), None, count.id, &Some(RankExpression::Operand(RankOperand::Input(next))), RankComparison::Strict, OperationResult::Scalar(*returned), finish.id, finish.parameter_types[0])
    ));
    let tail = lower(&source.replace("let x=Lease {mark:0}; ", "")).unwrap();
    ordinary_rank_actual(machine(&tail, "count"), machine(&tail, "count"), RankComparison::Strict);
}

#[test]
fn two_recursive_calls_before_one_return_have_distinct_rank_claims() {
    let source = ORDINARY.replace("count(n - 1),", "count(n - 1) + count(n),");
    let package = lower(&source).unwrap();
    let count = machine(&package, "count");
    let (block, first, decremented) = countdown_call(count, &source);
    let second = count.body.operations[block.operations.clone()].iter().find(|op| matches!(&op.kind,
        OperationKind::Call { callee: omega_lowering::CallTarget::Machine(id), arguments, .. }
            if *id == count.id && arguments.as_slice() == [CallArgument::Scalar(formal(count, 0))])).unwrap();
    let claims: Vec<_> = ranking(count).edges.iter().map(|c| (c.block, c.call, c.forwarding_return, &c.successor, c.comparison)).collect();
    assert_eq!(claims, [
        (block.id, CallSite::Operation(first.id), None, &Some(RankExpression::Operand(RankOperand::Input(decremented))), RankComparison::Strict),
        (block.id, CallSite::Operation(second.id), None, &Some(RankExpression::Operand(RankOperand::Input(formal(count, 0)))), RankComparison::Preserving),
    ]);
}

#[test]
fn a_forwarded_ordinary_call_and_its_terminal_twin_expose_the_same_recursive_edge() {
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
    assert_eq!(edge.successor, Some(RankExpression::Operand(RankOperand::Input(operation_result(count, IntegerPrimitive::Subtract, formal(count, 0))))));
}

/// Machine paths of the authored entry ranks that preparation hands to L5.
fn prepared_witnesses(source: &str) -> Vec<String> {
    let files = [("ranking.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    prepared.typed().ranking_witnesses.iter().map(|w| w.machine.path.clone()).collect()
}

/// The only terminal transfer from `caller` to `callee`: its block, edge and actual arguments.
fn terminal_call<'a>(
    caller: &'a LoweredMachine,
    callee: &LoweredMachine,
) -> (omega_lowering::BlockId, omega_lowering::EdgeId, &'a [CallArgument]) {
    let mut found = caller.body.blocks.iter().filter_map(|block| match &block.terminator {
        BlockTerminator::TerminalCall { id, callee: omega_lowering::CallTarget::Machine(target), arguments, .. } if *target == callee.id => {
            Some((block.id, *id, arguments.as_slice()))
        }
        _ => None,
    });
    let call = found.next().expect("terminal call to the callee");
    assert!(found.next().is_none());
    call
}

#[test]
fn an_unwitnessed_member_stays_in_the_ranked_component_without_a_target_rank() {
    let source = "machine first(n:u64)->u64 {transition {_ -> second(n),}}
         machine second(n:u64)->u64 terminates by n {transition n == 0 {true -> 9, false -> first(n - 1),}}";
    // Preparation supplies one authored entry rank, for second only.
    assert_eq!(prepared_witnesses(source), ["second"]);
    let package = lower(source).unwrap();
    let (first, second) = (machine(&package, "first"), machine(&package, "second"));
    assert!(first.contract.ranking.is_none() && !first.contract.promises_termination);
    assert!(second.contract.promises_termination);
    let mut members = vec![first.id, second.id];
    members.sort_unstable_by_key(|id| id.get());
    let witness = ranking(second);
    // The ranked member names the unranked one, so a checker sees the whole cycle.
    spec_corpus::observe_handoff!(psi "psi-output/unwitnessed-member-component", assert_eq!(witness.component, members));
    let (block, edge, arguments) = terminal_call(second, first);
    assert_eq!(arguments, [CallArgument::Scalar(operation_result(second, IntegerPrimitive::Subtract, formal(second, 0)))]);
    let [claim] = witness.edges.as_slice() else {
        panic!("expected the claim on the transfer into first")
    };
    assert_eq!((claim.block, claim.call, claim.forwarding_return, claim.target), (block, CallSite::Edge(edge), None, first.id));
    // first authored no entry rank, so the claim has no successor rank and selects no role.
    spec_corpus::observe_handoff!(
        psi "psi-output/unwitnessed-member-edge",
        assert_eq!((&claim.successor, claim.comparison), (&None, RankComparison::Pending))
    );
    // first's own edge back is its body's terminal transfer of its formal.
    assert_eq!(terminal_call(first, second).2, [CallArgument::Scalar(formal(first, 0))]);

    // Witnessing first as well supplies the target rank on the same edge.
    let twin = lower(&source.replacen("->u64 {", "->u64 terminates by n {", 1)).unwrap();
    let (first, second) = (machine(&twin, "first"), machine(&twin, "second"));
    let [claim] = ranking(second).edges.as_slice() else {
        panic!("expected the return claim")
    };
    assert_eq!(claim.target, first.id);
    assert_eq!(
        (&claim.successor, claim.comparison),
        (&Some(RankExpression::Operand(RankOperand::Input(operation_result(second, IntegerPrimitive::Subtract, formal(second, 0))))), RankComparison::Strict)
    );
}

#[test]
fn a_terminating_cycle_without_a_witness_keeps_its_promise_and_self_edge() {
    let source = "machine count(n:u64)->u64 terminates {transition n == 0 {true -> 7, false -> count(n - 1),}}";
    assert!(prepared_witnesses(source).is_empty());
    let package = lower(source).unwrap();
    let count = machine(&package, "count");
    // The promise reaches Psi without a witness; the checker judges it on this edge.
    spec_corpus::observe_handoff!(
        psi "psi-output/unwitnessed-terminating-cycle",
        assert_eq!((count.contract.promises_termination, &count.contract.ranking), (true, &None))
    );
    let (_, _, arguments) = terminal_call(count, count);
    spec_corpus::observe_handoff!(
        psi "psi-output/unwitnessed-terminating-self-edge",
        assert_eq!(arguments, [CallArgument::Scalar(operation_result(count, IntegerPrimitive::Subtract, formal(count, 0)))])
    );

    // Without the promise the same cycle owes nothing; with a witness the edge is claimed.
    let silent = lower(&source.replace(" terminates", "")).unwrap();
    assert!(!machine(&silent, "count").contract.promises_termination);
    let ranked = lower(&source.replace("terminates", "terminates by n")).unwrap();
    let count = machine(&ranked, "count");
    let [edge] = ranking(count).edges.as_slice() else {
        panic!("expected the self-edge claim")
    };
    assert_eq!((edge.call, edge.forwarding_return, edge.comparison), (CallSite::Edge(terminal_call(count, count).1), None, RankComparison::Strict));
}

#[test]
fn a_constant_rank_subject_publishes_its_prepared_value() {
    let constant = "const K:u64 = 3; machine f(n:u64) terminates by K {}";
    let package = lower(constant).unwrap();
    let f = machine(&package, "f");
    let witness = ranking(f);
    assert_eq!(witness.relation, NATURAL_U64);
    spec_corpus::observe_handoff!(
        psi "psi-output/constant-rank-subject",
        assert_eq!(witness.rank, RankExpression::Operand(RankOperand::Constant(3.into())))
    );
    assert_eq!(witness.component, [f.id]);
    assert!(witness.edges.is_empty()); // The acyclic body owes no comparison.

    // The prepared value decides the rank; a literal subject is the same constant.
    for (source, value) in [(constant.replace("= 3", "= 4"), 4), (constant.replace("by K", "by 3 -> omega\\language\\core\\Nat\\Descending"), 3)] {
        let package = lower(&source).unwrap();
        assert_eq!(ranking(machine(&package, "f")).rank, RankExpression::Operand(RankOperand::Constant(value.into())), "{source}");
    }
    // The formal twin names its input instead.
    let package = lower(&constant.replace("by K", "by n")).unwrap();
    let f = machine(&package, "f");
    let witness = ranking(f);
    assert_eq!(witness.relation, NATURAL_U64);
    assert_eq!(witness.rank, RankExpression::Operand(RankOperand::Input(0)));
    assert_eq!(witness.component, [f.id]);
    assert!(witness.edges.is_empty());

    // A computed subject `K - n` is one exact subtraction in authored order:
    // the prepared constant is the minuend and formal 0 the subtrahend.
    let computed = constant.replace("by K", "by K - n");
    let package = lower(&computed).unwrap();
    let f = machine(&package, "f");
    let witness = ranking(f);
    assert_eq!(witness.relation, NATURAL_U64);
    spec_corpus::observe_handoff!(
        psi "psi-output/computed-rank-subject",
        assert_eq!(witness.rank, RankExpression::ExactDifference { minuend: RankOperand::Constant(3.into()), subtrahend: RankOperand::Input(0) })
    );
    assert_eq!(witness.component, [f.id]);
    assert!(witness.edges.is_empty());
    // An independent Psi reader evaluates it; below zero the rank is undefined.
    assert_eq!(
        [0, 2, 3, 4].map(|n| psi_handoff_consumer::entry_rank(&witness.rank, &[n])),
        [Some(3), Some(1), Some(0), None]
    );

    // Swapping the sides, changing the prepared value or naming a literal or
    // two formals changes exactly the matching operand.
    use RankOperand::{Constant, Input};
    let difference = |minuend, subtrahend| RankExpression::ExactDifference { minuend, subtrahend };
    for (source, expected) in [
        (constant.replace("by K", "by n - K"), difference(Input(0), Constant(3.into()))),
        (
            constant.replace("= 3", "= 4").replace("by K", "by K - n"),
            difference(Constant(4.into()), Input(0)),
        ),
        (constant.replace("by K", "by 10 - n"), difference(Constant(10.into()), Input(0))),
        (constant.replace("by K", "by (K - n)"), difference(Constant(3.into()), Input(0))),
        ("machine g(m:u64, n:u64) terminates by m - n {}".into(), difference(Input(0), Input(1))),
        ("machine g(m:u64, n:u64) terminates by n - m {}".into(), difference(Input(1), Input(0))),
    ] {
        let package = lower(&source).unwrap();
        let subject = package
            .machines
            .iter()
            .find(|m| m.contract.ranking.is_some())
            .expect("one ranked machine");
        assert_eq!(ranking(subject).rank, expected, "{source}");
    }
    let swapped = lower(&constant.replace("by K", "by n - K")).unwrap();
    let swapped = &ranking(machine(&swapped, "f")).rank;
    assert_ne!(swapped, &witness.rank);
    assert_eq!([2, 3, 5].map(|n| psi_handoff_consumer::entry_rank(swapped, &[n])), [None, Some(0), Some(2)]);
}

#[test]
fn an_exact_difference_rank_uses_the_same_subtraction_over_the_call_actuals() {
    // The climbing cursor from the Terminal Psi page: rank `MAX - i`, call `climb(i + 1)`.
    let source = "const MAX:u64 = 10;
         machine climb(i:u64)->u64 terminates by MAX - i {
            transition i < MAX {true -> climb(i + 1), false -> i,}
         }";
    let package = lower(source).unwrap();
    let climb = machine(&package, "climb");
    let witness = ranking(climb);
    use RankOperand::{Constant, Input};
    assert_eq!(witness.relation, NATURAL_U64);
    assert_eq!(
        witness.rank,
        RankExpression::ExactDifference {
            minuend: Constant(10.into()),
            subtrahend: Input(0)
        }
    );
    assert_eq!(witness.component, [climb.id]);
    let [edge] = witness.edges.as_slice() else {
        panic!("expected one internal edge claim")
    };
    let (block, call, arguments) = terminal_call(climb, climb);
    let next = operation_result(climb, IntegerPrimitive::Add, formal(climb, 0));
    assert_eq!(arguments, [CallArgument::Scalar(next)]);
    // The edge keeps the constant minuend and substitutes the actual cursor.
    // A positive cursor step under an exact difference selects strict descent;
    // whether `MAX - (i + 1)` is defined there remains the checker's question.
    spec_corpus::observe_handoff!(
        psi "psi-output/computed-rank-edge",
        assert_eq!((edge.block, edge.call, edge.forwarding_return, edge.target, &edge.successor, edge.comparison),
                   (block, CallSite::Edge(call), None, climb.id, &Some(RankExpression::ExactDifference { minuend: Constant(10.into()), subtrahend: Input(next) }), RankComparison::Strict))
    );
    // An independent reader re-derives that successor from the published call.
    assert_eq!(
        psi_handoff_consumer::successor_from_call(&package, climb.id, edge).as_ref(),
        Ok(edge.successor.as_ref().unwrap())
    );

    // Swapped sides: the same call raises the minuend, so no role is selected.
    let swapped = lower(&source.replace("by MAX - i", "by i - MAX")).unwrap();
    let climb = machine(&swapped, "climb");
    let next = operation_result(climb, IntegerPrimitive::Add, formal(climb, 0));
    let [edge] = ranking(climb).edges.as_slice() else {
        panic!("expected one internal edge claim")
    };
    assert_eq!(
        ranking(climb).rank,
        RankExpression::ExactDifference {
            minuend: Input(0),
            subtrahend: Constant(10.into())
        }
    );
    assert_eq!(
        (&edge.successor, edge.comparison),
        (
            &Some(RankExpression::ExactDifference {
                minuend: Input(next),
                subtrahend: Constant(10.into())
            }),
            RankComparison::Pending
        )
    );
    assert_eq!(
        psi_handoff_consumer::successor_from_call(&swapped, climb.id, edge).as_ref(),
        Ok(edge.successor.as_ref().unwrap())
    );

    // Passing the cursor unchanged repeats the exact source rank.
    let repeated = lower(&source.replace("climb(i + 1)", "climb(i)")).unwrap();
    let climb = machine(&repeated, "climb");
    let [edge] = ranking(climb).edges.as_slice() else {
        panic!("expected one internal edge claim")
    };
    assert_eq!(edge.successor, ranking(climb).source_rank(&climb.body.parameters));
    assert_eq!(edge.comparison, RankComparison::Preserving);

    // A shrinking minuend over an unchanged subtrahend also descends strictly.
    let shrinking = lower("machine down(n:u64)->u64 terminates by n - 1 {transition n <= 1 {true -> n, false -> down(n - 1),}}").unwrap();
    let down = machine(&shrinking, "down");
    let [edge] = ranking(down).edges.as_slice() else {
        panic!("expected one internal edge claim")
    };
    let next = operation_result(down, IntegerPrimitive::Subtract, formal(down, 0));
    assert_eq!(
        (&edge.successor, edge.comparison),
        (
            &Some(RankExpression::ExactDifference {
                minuend: Input(next),
                subtrahend: Constant(1.into())
            }),
            RankComparison::Strict
        )
    );
}

#[test]
fn computed_rank_arithmetic_preserves_operations_and_operands() {
    use IntegerPrimitive::{Add, Subtract};
    use omega_lowering::{
        ArithmeticPolicy::{Exact, Saturating, Wrapping},
        IntegerObligation::ResultFitsCarrier,
        IntegerOperation,
        RankArithmeticStep::{Apply, Operand},
    };
    let carrier = ScalarType::Integer { signed: false, width: 64 };
    let n = Operand {
        operand: RankOperand::Input(0),
        carrier,
    };
    let k = Operand {
        operand: RankOperand::Constant(3.into()),
        carrier,
    };
    let one = Operand {
        operand: RankOperand::Constant(1.into()),
        carrier,
    };
    let operation = |primitive, policy, obligations: &[_]| {
        Apply(IntegerOperation {
            primitive,
            policy,
            obligations: obligations.to_vec(),
        })
    };
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    for (subject, steps, at_four, sites) in [
        (
            "K -% n",
            vec![k.clone(), n.clone(), operation(Subtract, Wrapping, &[])],
            Some(i128::from(u64::MAX)),
            vec!["K -% n"],
        ),
        (
            "K -| n",
            vec![k.clone(), n.clone(), operation(Subtract, Saturating, &[])],
            Some(0),
            vec!["K -| n"],
        ),
        (
            "K + n",
            vec![
                k.clone(),
                n.clone(),
                operation(Add, Exact, &[ResultFitsCarrier]),
            ],
            Some(7),
            vec!["K + n"],
        ),
        (
            "K - n - 1",
            vec![
                k,
                n,
                operation(Subtract, Exact, &[ResultFitsCarrier]),
                one,
                operation(Subtract, Exact, &[ResultFitsCarrier]),
            ],
            None,
            vec!["K - n", "K - n - 1"],
        ),
    ] {
        let text = format!("const K:u64 = 3; machine f(n:u64)->u64 terminates by {subject} {{transition n == 0 {{true -> 7, false -> f(n - 1),}}}}");
        let package = lower(&text).unwrap();
        let f = machine(&package, "f");
        let witness = ranking(f);
        let RankExpression::Arithmetic(actual) = &witness.rank else {
            panic!("missing computed rank")
        };
        // Replay each formerly refused program, then exercise the same rank on a recursive edge.
        let original = format!("const K:u64 = 3; machine f(n:u64) terminates by {subject} {{}}");
        let original_package = lower(&original).unwrap();
        let original_rank = &ranking(machine(&original_package, "f")).rank;
        let RankExpression::Arithmetic(original_steps) = original_rank else {
            panic!("original case lost its computation")
        };
        observed.push((original_steps.clone(), actual.clone()));
        expected.push((steps.clone(), steps.clone()));
        assert_eq!(
            witness
                .arithmetic_sites
                .iter()
                .map(|site| site.span.slice(text.as_bytes()).unwrap())
                .collect::<Vec<_>>(),
            sites.iter().map(|s| s.as_bytes()).collect::<Vec<_>>()
        );
        assert_eq!(psi_handoff_consumer::entry_rank(&witness.rank, &[4]), at_four);
        let [edge] = witness.edges.as_slice() else { panic!("expected one self call") };
        let next = operation_result(f, Subtract, formal(f, 0));
        let expected = RankExpression::Arithmetic(steps)
            .try_map(|index| (index == 0).then_some(next))
            .unwrap();
        assert_eq!(edge.successor.as_ref(), Some(&expected));
        assert_eq!(psi_handoff_consumer::successor_from_call(&package, f.id, edge).unwrap(), expected);
    }
    spec_corpus::observe_handoff!(psi "psi-output/computed-rank-arithmetic", assert_eq!(observed, expected));
}
#[test]
fn rank_intermediates_keep_their_carriers_and_distinct_formation_obligations() {
    use omega_lowering::{
        ArithmeticPolicy::Exact,
        IntegerObligation::{NonzeroDivisor, ResultFitsCarrier, ShiftCountInRange},
        IntegerOperation,
        RankArithmeticStep::{Apply, Operand},
    };
    let source = "machine width(n:u64, count:i8, divisor:u64) terminates by (n << count) / divisor {}";
    let package = lower(source).unwrap();
    let witness = ranking(machine(&package, "width"));
    let natural = ScalarType::Integer { signed: false, width: 64 };
    let signed_count = ScalarType::Integer { signed: true, width: 8 };
    let input = |slot, carrier| Operand {
        operand: RankOperand::Input(slot),
        carrier,
    };
    let operation = |primitive, obligations| {
        Apply(IntegerOperation {
            primitive,
            policy: Exact,
            obligations,
        })
    };
    assert_eq!(
        witness.rank,
        RankExpression::Arithmetic(vec![
            input(0, natural),
            input(1, signed_count),
            operation(IntegerPrimitive::ShiftLeft, vec![ShiftCountInRange, ResultFitsCarrier]),
            input(2, natural),
            operation(IntegerPrimitive::Divide, vec![NonzeroDivisor]),
        ])
    );
    assert_eq!(
        witness
            .arithmetic_sites
            .iter()
            .map(|site| site.span.slice(source.as_bytes()).unwrap())
            .collect::<Vec<_>>(),
        [b"n << count".as_slice(), b"(n << count) / divisor"]
    );
}

#[test]
fn a_constant_distance_limit_is_the_same_value_on_the_recursive_edge() {
    let package = lower(
        "const LIMIT:u64 = 10;
         machine climb(i:u64)->u64 terminates by (i, LIMIT) -> omega\\language\\core\\Nat\\BoundedDistance {
            transition i < LIMIT {true -> climb(i + 1), false -> i,}
         }",
    )
    .unwrap();
    let climb = machine(&package, "climb");
    let witness = ranking(climb);
    fn limit<Input>() -> RankOperand<Input> {
        RankOperand::Constant(10.into())
    }
    assert_eq!(witness.rank, RankExpression::SaturatingDistance { cursor: RankOperand::Input(0), limit: limit() });
    let [edge] = witness.edges.as_slice() else {
        panic!("expected one internal edge claim")
    };
    assert_eq!(
        edge.successor,
        Some(RankExpression::SaturatingDistance {
            cursor: RankOperand::Input(operation_result(climb, IntegerPrimitive::Add, formal(climb, 0))),
            limit: limit(),
        })
    );
    // Lowering recognizes no guard against a constant limit; the checker selects the role.
    assert_eq!(edge.comparison, RankComparison::Pending);
}

/// Every cleanup invocation of `target` in `caller`, in body order: its block,
/// list and ordinal and the whole local it cleans. Found from the body alone.
fn cleanup_invocations(
    caller: &LoweredMachine,
    target: &LoweredMachine,
) -> Vec<(omega_lowering::BlockId, omega_lowering::CleanupSite, omega_lowering::PlaceId)> {
    use omega_lowering::{CleanupAction, CleanupList, CleanupSite};
    let mut found = Vec::new();
    let mut scan = |block: &omega_lowering::BranchBlock, list: CleanupList, actions: &[CleanupAction]| {
        for (action, row) in actions.iter().enumerate() {
            if let CleanupAction::Invoke { place, ty, target: invoked } = row
                && *invoked == target.id
            {
                assert!(place.path.is_empty() && *ty == target.parameter_types[0]);
                found.push((block.id, CleanupSite { list, action }, place.root));
            }
        }
    };
    for block in &caller.body.blocks {
        for op in &caller.body.operations[block.operations.clone()] {
            if let OperationKind::StructuralStore { disposition: omega_lowering::StructuralStoreDisposition::Dispose(actions), .. } = &op.kind {
                scan(block, CleanupList::Store(op.id), actions);
            }
        }
        match &block.terminator {
            BlockTerminator::Return { id, cleanup, .. } | BlockTerminator::TerminalCall { id, cleanup, .. } => scan(block, CleanupList::Edge(*id), cleanup),
            BlockTerminator::Jump(edge) => scan(block, CleanupList::Edge(edge.id), &edge.cleanup),
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                scan(block, CleanupList::Edge(when_true.id), &when_true.cleanup);
                scan(block, CleanupList::Edge(when_false.id), &when_false.cleanup);
            }
            BlockTerminator::ExternalExit(_) => {}
        }
    }
    found
}

/// The place the only record establishment in `machine` creates.
fn established_record(machine: &LoweredMachine) -> omega_lowering::PlaceId {
    let mut found = machine.body.operations.iter().filter_map(|op| match (&op.kind, op.result) {
        (OperationKind::EstablishRecord { .. }, OperationResult::Structural(place)) => Some(place),
        _ => None,
    });
    let place = found.next().expect("the Lease establishment");
    assert!(found.next().is_none());
    place
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

    // again's return runs finish on its Lease, and finish calls again(3): the
    // cycle can repeat rank 3, so the checker must refuse the promise. L5
    // publishes the cleanup as again's in-component edge so the checker can.
    let package = sources::lower_prepared(&prepared, &files).unwrap();
    let again = machine(&package, "again");
    let finish = machine(&package, "finish");
    let [(block, at, lease)] = cleanup_invocations(again, finish)[..] else { panic!("one cleanup of again's Lease") };
    assert_eq!(lease, established_record(again));
    assert!(matches!(at.list, omega_lowering::CleanupList::Edge(id)
        if matches!(&again.body.blocks[block.get() - 1].terminator, BlockTerminator::Return { id: edge, .. } if *edge == id)));
    // finish has no rank of its own but stays in the component; its call back
    // to again stays in its body for the checker to follow.
    assert!(finish.contract.ranking.is_none());
    let rank = ranking(again);
    spec_corpus::observe_handoff!(psi "psi-output/ranked-cleanup-edge", assert_eq!(
        (&rank.component, rank.edges.as_slice()),
        (&vec![again.id, finish.id], [omega_lowering::RankedEdge {
            block, call: CallSite::Cleanup(at), forwarding_return: None, target: finish.id, successor: None, comparison: RankComparison::Pending,
        }].as_slice())
    ));
    let three = finish.body.operations.iter().find(|op| op.kind == OperationKind::Constant(omega_lowering::ScalarLiteral::Integer(3.into())));
    let OperationResult::Scalar(three) = three.expect("the rank at re-entry").result else { panic!("expected a scalar argument") };
    assert!(finish.body.operations.iter().any(|op| matches!(&op.kind,
        OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), arguments, .. }
            if *target == again.id && arguments.as_slice() == [CallArgument::Scalar(three)])));
    // The independent consumer reads the invocation back from the published
    // body by list and ordinal: the cleaned place is again's own Lease, moved
    // whole into finish's one formal, and there is no target rank to substitute.
    let [edge] = rank.edges.as_slice() else { unreachable!() };
    let (place, ty, invoked) = psi_handoff_consumer::claimed_cleanup(again, edge).unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/ranked-cleanup-actual", assert_eq!(
        ((place.root, place.path.as_slice(), ty, invoked), psi_handoff_consumer::successor_from_call(&package, again.id, edge)),
        ((lease, [].as_slice(), finish.parameter_types[0], finish.id), Err("the target publishes no rank".to_string()))
    ));
    assert!(matches!(finish.body.parameters.as_slice(), [Parameter::Structural(_)]));

    // Removing the promise, not the cleanup, leaves the same cleanup in the
    // body with no ranking to claim it.
    let package = lower(&source.replace(" terminates by n", "")).unwrap();
    let (again, finish) = (machine(&package, "again"), machine(&package, "finish"));
    assert_eq!(cleanup_invocations(again, finish).len(), 1);
    assert!(again.contract.ranking.is_none());

    // The lawful twin keeps the same cleanup edge; only the call back to again
    // is gone. That edge now leaves the component, so it owes no rank claim.
    let package = lower(&source.replace("again(3);", "")).unwrap();
    let (caller, finish) = (machine(&package, "again"), machine(&package, "finish"));
    assert_eq!(cleanup_invocations(caller, finish).len(), 1);
    assert_eq!(ranking(caller).component, vec![caller.id]);
    assert!(ranking(caller).edges.is_empty());
}

#[test]
fn a_ranked_cleanup_claim_names_its_list_ordinal_and_constant_target_rank() {
    use omega_lowering::{CleanupList, RankedEdge};
    let claim = |block, at, target, successor| RankedEdge {
        block, call: CallSite::Cleanup(at), forwarding_return: None, target, successor, comparison: RankComparison::Pending,
    };
    // A ranked cleanup target: its constant rank is the successor, and the
    // consumer derives the same constant from the published invocation.
    let source = "const Z:u64 = 0;
data Lease dropped by finish {mark:u8;}
        machine again(n:u64)->u64 terminates by n { let lease=Lease {mark:0}; n }
        machine finish(x:Lease) terminates by Z { let Lease {mark as _}=x; let r=again(3); }";
    let package = lower(source).unwrap();
    let (again, finish) = (machine(&package, "again"), machine(&package, "finish"));
    let [(block, at, _)] = cleanup_invocations(again, finish)[..] else { panic!("one cleanup") };
    let zero = RankExpression::Operand(RankOperand::Constant(0.into()));
    let call = finish.body.operations.iter().find(|op| matches!(&op.kind,
        OperationKind::Call { callee: omega_lowering::CallTarget::Machine(target), .. } if *target == again.id)).unwrap();
    let OperationKind::Call { arguments, .. } = &call.kind else { unreachable!() };
    let [CallArgument::Scalar(three)] = arguments.as_slice() else { panic!("one scalar actual") };
    let back = finish.body.blocks.iter().find(|b| b.operations.contains(&call.id.index())).unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/ranked-cleanup-target-rank", assert_eq!(
        (ranking(again).edges.as_slice(), ranking(finish).edges.as_slice(), psi_handoff_consumer::successor_from_call(&package, again.id, &ranking(again).edges[0])),
        ([claim(block, at, finish.id, Some(zero.clone()))].as_slice(),
         [RankedEdge { block: back.id, call: CallSite::Operation(call.id), forwarding_return: None, target: again.id,
             successor: Some(RankExpression::Operand(RankOperand::Input(*three))), comparison: RankComparison::Pending }].as_slice(),
         Ok(zero))
    ));

    // A disposing store carries its own list. Replacing the Lease runs finish
    // on the old value at the store; the return then cleans the new one.
    let source = "data Lease dropped by finish {mark:u8;}
        machine again(n:u64) terminates by n { let mutable lease=Lease {mark:0}; lease=Lease {mark:1}; }
        machine finish(x:Lease) { let Lease {mark as _}=x; again(3); }";
    let package = lower(source).unwrap();
    let (again, finish) = (machine(&package, "again"), machine(&package, "finish"));
    let found = cleanup_invocations(again, finish);
    let [(store_block, store, _), (return_block, ret, _)] = found[..] else { panic!("store and return cleanups: {found:?}") };
    assert!(matches!(store.list, CleanupList::Store(_)) && matches!(ret.list, CleanupList::Edge(_)));
    assert_eq!(ranking(again).edges, [claim(store_block, store, finish.id, None), claim(return_block, ret, finish.id, None)]);
    for edge in &ranking(again).edges {
        assert_eq!(psi_handoff_consumer::claimed_cleanup(again, edge).unwrap().2, finish.id);
    }

    // On a terminal call edge the cleanup runs before the transfer, and its
    // claim comes first; the transfer keeps its own strict claim.
    let source = "data Lease dropped by finish {mark:u8;}
        machine again(n:u64)->u64 terminates by n { let lease=Lease {mark:0}; transition {_ -> again(n - 1),} }
        machine finish(x:Lease) { let Lease {mark as _}=x; let r=again(3); }";
    let package = lower(source).unwrap();
    let (again, finish) = (machine(&package, "again"), machine(&package, "finish"));
    let [(block, at, _)] = cleanup_invocations(again, finish)[..] else { panic!("one cleanup") };
    let (transfer_block, edge, _) = terminal_call(again, again);
    assert_eq!((block, at.list, at.action), (transfer_block, CleanupList::Edge(edge), 0));
    let next = operation_result(again, IntegerPrimitive::Subtract, formal(again, 0));
    assert_eq!(ranking(again).edges, [
        claim(block, at, finish.id, None),
        RankedEdge { block, call: CallSite::Edge(edge), forwarding_return: None, target: again.id,
            successor: Some(RankExpression::Operand(RankOperand::Input(next))), comparison: RankComparison::Strict },
    ]);
}

/// A ranked cycle that owns a `Lease`: two back edges enter `count` with
/// different actuals, and the first arm leaves the cycle. Here that arm returns
/// the result of a trapping division. Bytes equal `sources/returning.omg`.
const RETURNING_CYCLE: &str = r#"use omega\language\core;
data Lease dropped by finish {mark:u8;}
machine finish(l:Lease) {let Lease{mark as _}=l;}
machine count(n:u64, m:u64, d:u64)->u64 crashes Trap d == 0 terminates by n {
    let lease = Lease{mark:0};
    transition {
        n == 0 -> n /! d,
        n < m -> count(n - 1, m, d),
        _ -> count(m - 1, n, d),
    }
}
"#;

/// The twin: the leaving arm exits the process. The back edges are unchanged.
const EXITING_CYCLE: &str = r#"use omega\language\core;
data Lease dropped by finish {mark:u8;}
machine finish(l:Lease) {let Lease{mark as _}=l;}
machine count(n:u64, m:u64, code:i32)->core\Never reaches core\ProcessExit terminates by n {
    let lease = Lease{mark:0};
    transition {
        n == 0 -> core\ProcessExit::exit_process(code),
        n < m -> count(n - 1, m, code),
        _ -> count(m - 1, n, code),
    }
}
"#;

/// The conditional of a block, and both successor targets.
fn branch(body: &omega_lowering::MachineBody, block: omega_lowering::BlockId) -> (ValueId, omega_lowering::BlockId, omega_lowering::BlockId) {
    let BlockTerminator::Conditional { condition, when_true, when_false } = &body.blocks[block.get() - 1].terminator else {
        panic!("expected a guard at {block:?}")
    };
    (*condition, when_true.target, when_false.target)
}

#[test]
fn each_back_edge_into_a_ranked_cycle_keeps_its_own_actuals_cleanup_and_claim() {
    for (source, exits) in [(RETURNING_CYCLE, false), (EXITING_CYCLE, true)] {
        let files = [("cycle.omg", source)];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let typed = prepared.typed();
        let [witness] = typed.ranking_witnesses.as_slice() else { panic!("count's rank") };
        assert_eq!(witness.machine.path, "count");
        // L3 types both back edges as calls of count, each with its own first actual.
        let actuals: Vec<_> = typed.calls.iter().filter(|c| c.caller.as_ref() == Some(&witness.machine) && c.machine == witness.machine)
            .map(|c| c.operands[0].site.span.slice(source.as_bytes())).collect();
        assert_eq!(actuals, [Some(b"n - 1".as_slice()), Some(b"m - 1".as_slice())]);
        let [cleanup] = typed.cleanups.as_slice() else { panic!("Lease's cleanup selection") };
        assert_eq!((cleanup.owner.path.as_str(), cleanup.machine.path.as_str()), ("Lease", "finish"));
        let package = sources::lower_prepared(&prepared, &files).unwrap();
        drop(prepared);
        drop(resolved);
        let count = machine(&package, "count");
        let finish = machine(&package, "finish");
        let (n, m, third) = (formal(count, 0), formal(count, 1), formal(count, 2));
        let body = &count.body;
        let lease = body.operations.iter().find_map(|op| match (&op.kind, op.result) {
            (OperationKind::EstablishRecord { .. }, OperationResult::Structural(place)) => Some(place),
            _ => None,
        }).expect("the Lease local");

        // Follow the guards from the entry, independently of the claims.
        let (leaving_guard, leaving, rest) = branch(body, body.blocks[0].id);
        assert_eq!(leaving_guard, operation_result(count, IntegerPrimitive::Equal, n));
        let (descending_guard, down_block, swap_block) = branch(body, rest);
        assert_eq!(descending_guard, operation_result(count, IntegerPrimitive::LessThan, n));
        let (down, swap) = (operation_result(count, IntegerPrimitive::Subtract, n), operation_result(count, IntegerPrimitive::Subtract, m));
        let back_edge = |block: omega_lowering::BlockId| {
            let BlockTerminator::TerminalCall { id, callee, arguments, .. } = &body.blocks[block.get() - 1].terminator else {
                panic!("expected a back edge at {block:?}")
            };
            assert_eq!(*callee, omega_lowering::CallTarget::Machine(count.id));
            (*id, arguments.clone())
        };
        let (down_edge, down_actuals) = back_edge(down_block);
        let (swap_edge, swap_actuals) = back_edge(swap_block);
        // Both predecessors enter the same member; only their actuals tell them apart.
        assert_eq!(down_actuals, [CallArgument::Scalar(down), CallArgument::Scalar(m), CallArgument::Scalar(third)]);
        assert_eq!(swap_actuals, [CallArgument::Scalar(swap), CallArgument::Scalar(n), CallArgument::Scalar(third)]);

        // The `n < m` guard selects descent. On the other edge `m - 1 < n` also
        // needs `m >= 1`, the subtraction's own obligation, so L6 decides the role.
        let claims: Vec<_> = ranking(count).edges.iter()
            .map(|e| (e.block, e.call, e.forwarding_return, e.target, e.successor.clone(), e.comparison)).collect();
        spec_corpus::observe_handoff!(psi "psi-output/cycle-predecessor-claims", assert_eq!(claims, [
            (down_block, CallSite::Edge(down_edge), None, count.id, Some(RankExpression::Operand(RankOperand::Input(down))), RankComparison::Strict),
            (swap_block, CallSite::Edge(swap_edge), None, count.id, Some(RankExpression::Operand(RankOperand::Input(swap))), RankComparison::Pending),
        ]));
        psi_handoff_consumer::product_witness::cycle_claims(&package, count.id).unwrap();

        // Cleanup belongs to each completion of the activation, never to a guard edge.
        let disposing = psi_handoff_consumer::product_witness::completion_cleanup(body, lease, finish.id).unwrap();
        let leaving = &body.blocks[leaving.get() - 1];
        if exits {
            let BlockTerminator::ExternalExit(exit) = &leaving.terminator else { panic!("the leaving arm exits") };
            assert_eq!(psi_handoff_consumer::external_exits(body), [exit]);
            assert_eq!(exit.arguments, [CallArgument::Scalar(third)]);
            spec_corpus::observe_handoff!(psi "psi-output/cycle-exit-cleanup", assert_eq!(disposing, [down_edge, swap_edge]));
        } else {
            let BlockTerminator::Return { id, result: omega_lowering::ReturnEdge::Scalar(returned), .. } = leaving.terminator else {
                panic!("the leaving arm returns")
            };
            // The division traps inside the returning block, before its cleanup-bearing edge.
            let ops = &body.operations[leaving.operations.clone()];
            let division = ops.iter().position(|op| matches!(op.kind,
                OperationKind::TrappingInteger { primitive: omega_lowering::TrappingIntegerPrimitive::Divide, left, right } if (left, right) == (n, third)));
            assert_eq!(division, Some(ops.len() - 1));
            assert_eq!(ops[ops.len() - 1].result, OperationResult::Scalar(returned));
            assert_eq!(count.contract.crash_routes.len(), 1);
            spec_corpus::observe_handoff!(psi "psi-output/cycle-return-cleanup", assert_eq!(disposing, [id, down_edge, swap_edge]));
        }
    }
}

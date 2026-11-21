//! Runtime callable contracts meeting joins and terminal edges. A runtime call
//! owes each `requires` clause of the contract it names at its own call site,
//! whether it is an operation or a terminal edge, and its guarantee describes
//! that call's result. Descriptors chosen at a join keep each arm's selection,
//! and an exclusive view chosen by an earlier `match` still reaches the call as
//! the join's own parameter. Every refinement and use verdict here belongs to
//! the checker; these tests pin only what Psi hands it.
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CallArgument, CallRequirement, CallSite, CallTarget, CallableContractId, LoweredMachine, LoweredPackage, MachineBody, OperationKind,
    OperationResult, Parameter, PredicateOperator, PredicateStep, ValueId,
};
use psi_handoff_consumer::product_witness::{
    CallTerm, JoinedRuntimeCall, MeasuredView, RuntimeCallAt, joined_runtime_call, runtime_call_contract_at, runtime_calls,
};

const LEGAL: &str = include_str!("fixtures/runtime_joins/join-distinct-machines-legal.omg");
const NARROWED: &str = include_str!("fixtures/runtime_joins/join-distinct-machines-narrowed-input.omg");
const WEAKER: &str = include_str!("fixtures/runtime_joins/join-distinct-machines-weaker-guarantee.omg");
const CONTRACTS: &str = include_str!("fixtures/runtime_joins/join-distinct-contracts.omg");
const ORDINARY: &str = include_str!("fixtures/runtime_joins/ordered-debts-ordinary.omg");
const TERMINAL: &str = include_str!("fixtures/runtime_joins/ordered-debts-terminal.omg");
const RETURN_OR_TERMINAL: &str = include_str!("fixtures/runtime_joins/ordered-debts-return-or-terminal.omg");
const COMBINED: &str = include_str!("fixtures/runtime_joins/exclusive-and-descriptor-join.omg");
const SAME_FORMAL: &str = include_str!("fixtures/runtime_joins/exclusive-same-formal-both-arms.omg");
const MOVED_THEN_USED: &str = include_str!("fixtures/runtime_joins/exclusive-moved-then-used.omg");

fn lower(source: &str) -> LoweredPackage {
    let package = sources::lower(&[("case.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    for machine in &package.machines {
        sources::control_flow::check_operation_ownership(&machine.body).unwrap_or_else(|e| panic!("{}: {e}", machine.declaration.path));
    }
    package
}

fn machine<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|m| m.declaration.path == path)
        .unwrap_or_else(|| panic!("missing machine {path}"))
}

fn scalar(
    machine: &LoweredMachine,
    index: usize,
) -> ValueId {
    match machine.body.parameters[index] {
        Parameter::Scalar(value) => value,
        other => panic!("expected scalar formal {index}, found {other:?}"),
    }
}

/// For each side of the entry dispatch, the operations of its arm and the one value it hands to the join.
fn arms(body: &MachineBody) -> [(Vec<&OperationKind>, ValueId); 2] {
    let BlockTerminator::Conditional { when_true, when_false, .. } = &body.blocks[0].terminator else {
        panic!("expected an entry dispatch")
    };
    [when_true, when_false].map(|edge| {
        let arm = &body.blocks[edge.target.get() - 1];
        let BlockTerminator::Jump(arrival) = &arm.terminator else {
            panic!("each arm jumps to the join")
        };
        let [value] = arrival.arguments[..] else {
            panic!("each arm hands one descriptor to the join")
        };
        (
            body.operations[arm.operations.clone()]
                .iter()
                .map(|op| &op.kind)
                .collect(),
            value,
        )
    })
}

/// The join's only scalar parameter, which every arm feeds.
fn joined(body: &MachineBody) -> ValueId {
    let BlockTerminator::Conditional { when_true, .. } = &body.blocks[0].terminator else {
        panic!("entry dispatch")
    };
    let BlockTerminator::Jump(arrival) = &body.blocks[when_true.target.get() - 1].terminator else {
        panic!("arm jump")
    };
    let [value] = body.blocks[arrival.target.get() - 1].scalar_parameters[..] else {
        panic!("one join parameter")
    };
    value
}

fn lt(
    left: CallTerm,
    right: CallTerm,
) -> Vec<CallTerm> {
    vec![
        left,
        right,
        CallTerm::Step(PredicateStep::Apply(PredicateOperator::LessThan)),
    ]
}

fn int(n: u32) -> CallTerm {
    CallTerm::Step(PredicateStep::Integer(n.into()))
}

/// Two named machines of the same shape are constructed under one contract, one
/// per arm, and the join hands the arriving descriptor to `run`. Narrowing
/// `low`'s input or weakening its guarantee changes only `low`'s own published
/// clauses: `choose`, `run` and the contract are unchanged, so the checker's
/// refinement judgment at each construction is the only thing that can refuse
/// those twins.
#[test]
fn distinct_machines_joined_under_one_contract_leave_each_refinement_to_the_checker() {
    let package = lower(LEGAL);
    let reader = CallableContractId::from_index(0).unwrap();
    let [contract] = &package.callable_contracts[..] else {
        panic!("Reader is the only contract")
    };
    let choose = machine(&package, "choose");
    let [(true_ops, true_value), (false_ops, false_value)] = arms(&choose.body);
    let made = |ops: &[&OperationKind]| match ops {
        [OperationKind::ConstructCallableDescriptor { machine, contract }] => (*machine, *contract),
        other => panic!("expected one construction, found {other:?}"),
    };
    spec_corpus::observe_handoff!(psi "psi-output/runtime-join-distinct-machines", assert_eq!(
        [made(&true_ops), made(&false_ops)],
        [(machine(&package, "low").id, reader), (machine(&package, "high").id, reader)]
    ));
    let constructed = |kind: &OperationKind| {
        choose
            .body
            .operations
            .iter()
            .find(|op| &op.kind == kind)
            .map(|op| op.result)
            .unwrap()
    };
    assert_eq!(
        (OperationResult::Scalar(true_value), OperationResult::Scalar(false_value)),
        (constructed(true_ops[0]), constructed(false_ops[0]))
    );
    let [OperationKind::Call { callee, arguments, .. }] = choose.body.operations[choose.body.blocks[3].operations.clone()]
        .iter()
        .map(|op| &op.kind)
        .collect::<Vec<_>>()[..]
    else {
        panic!("the join calls run")
    };
    assert_eq!(
        (callee, arguments.as_slice()),
        (
            &CallTarget::Machine(machine(&package, "run").id),
            [
                CallArgument::Scalar(joined(&choose.body)),
                CallArgument::Scalar(scalar(choose, 1))
            ]
            .as_slice()
        )
    );
    let run = machine(&package, "run");
    let found = runtime_call_contract_at(&package, &run.body, RuntimeCallAt::Operation(0)).unwrap();
    let OperationResult::Scalar(result) = run.body.operations[0].result else {
        panic!("u8 result")
    };
    assert_eq!(
        (found.contract, found.debts, found.guarantees),
        (
            reader,
            vec![lt(CallTerm::Argument(scalar(run, 1)), int(100))],
            vec![lt(CallTerm::Result(result), int(50))]
        )
    );

    let clauses = |m: &LoweredMachine| {
        (
            m.contract
                .requires
                .iter()
                .map(|c| c.predicate.clone())
                .collect::<Vec<_>>(),
            m.contract
                .ensures
                .iter()
                .map(|c| c.predicate.clone())
                .collect::<Vec<_>>(),
        )
    };
    let bound = |formal: PredicateStep, n: u32| {
        vec![
            formal,
            PredicateStep::Integer(n.into()),
            PredicateStep::Apply(PredicateOperator::LessThan),
        ]
    };
    for (source, low_requires, low_ensures) in [(LEGAL, 1000, 5), (NARROWED, 10, 5), (WEAKER, 1000, 200)] {
        let twin = lower(source);
        assert_eq!(
            clauses(machine(&twin, "low")),
            (
                vec![bound(PredicateStep::Formal(0), low_requires)],
                vec![bound(PredicateStep::Result, low_ensures)]
            )
        );
        assert_eq!(&twin.callable_contracts[..], std::slice::from_ref(contract));
        for name in ["choose", "run", "high"] {
            let (a, b) = (machine(&twin, name), machine(&package, name));
            assert_eq!(
                (&a.body.operations, &a.body.blocks, clauses(a)),
                (&b.body.operations, &b.body.blocks, clauses(b)),
                "{name}"
            );
        }
    }
}

/// Two aliases with the same shape and different clauses refine into a third
/// contract, one per arm. `Loose` states exactly `Reader`'s clauses yet keeps its
/// own identity, and the call through the join owes `Reader`'s precondition.
#[test]
fn distinct_contracts_joined_refine_into_the_called_contract() {
    let package = lower(CONTRACTS);
    let choose = machine(&package, "choose");
    let id = |n| CallableContractId::from_index(n).unwrap();
    let (reader, strict, loose) = (id(0), id(1), id(2));
    let contract_of = |value: ValueId| match package.types.get(choose.body.values[value.index()].ty) {
        Some(omega_lowering::TypeShape::Callable(c)) => *c,
        other => panic!("not a descriptor: {other:?}"),
    };
    assert_eq!((contract_of(scalar(choose, 1)), contract_of(scalar(choose, 2))), (strict, loose));
    let [(true_ops, _), (false_ops, _)] = arms(&choose.body);
    spec_corpus::observe_handoff!(psi "psi-output/runtime-join-distinct-contracts", assert_eq!(
        (true_ops, false_ops),
        (
            vec![&OperationKind::RefineCallableDescriptor { source: scalar(choose, 1), contract: reader }],
            vec![&OperationKind::RefineCallableDescriptor { source: scalar(choose, 2), contract: reader }]
        )
    ));
    let predicates = |c: CallableContractId| {
        let row = &package.callable_contracts[c.index()];
        (
            row.requires
                .iter()
                .map(|c| c.predicate.clone())
                .collect::<Vec<_>>(),
            row.ensures
                .iter()
                .map(|c| c.predicate.clone())
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(predicates(loose), predicates(reader));
    assert_ne!(predicates(strict), predicates(reader));
    let [at] = runtime_calls(&choose.body)[..] else { panic!("one runtime call") };
    let found = runtime_call_contract_at(&package, &choose.body, at).unwrap();
    assert_eq!(
        (found.contract, found.debts),
        (reader, vec![lt(CallTerm::Argument(scalar(choose, 3)), int(100))])
    );
    let RuntimeCallAt::Operation(op) = at else { panic!("ordinary call") };
    assert!(
        matches!(choose.body.operations[op].kind, OperationKind::Call { callee: CallTarget::Runtime { descriptor, .. }, .. } if descriptor == joined(&choose.body))
    );
}

/// `Pair` has two ordered preconditions and a guarantee over its first formal;
/// the caller passes `(y, x)`. An ordinary call owes both clauses at its
/// operation, and a terminal call at its own edge, in clause order and under the
/// swapped actuals. A terminal call's result is the caller's own return. When
/// one arm returns a constant and the other calls, only the calling edge owes.
#[test]
fn ordered_debts_follow_the_call_on_ordinary_and_terminal_edges() {
    let debts = |m: &LoweredMachine| {
        let (x, y) = (CallTerm::Argument(scalar(m, 1)), CallTerm::Argument(scalar(m, 2)));
        vec![lt(y.clone(), int(100)), lt(x, y)]
    };
    let ordinary = lower(ORDINARY);
    let apply = machine(&ordinary, "apply");
    let [at @ RuntimeCallAt::Operation(index)] = runtime_calls(&apply.body)[..] else {
        panic!("one ordinary runtime call")
    };
    let OperationResult::Scalar(result) = apply.body.operations[index].result else {
        panic!("scalar result")
    };
    let OperationKind::Call { requirements, .. } = &apply.body.operations[index].kind else {
        unreachable!()
    };
    let site = CallSite::Operation(apply.body.operations[index].id);
    assert_eq!(
        requirements,
        &[
            CallRequirement { site, clause: 0 },
            CallRequirement { site, clause: 1 }
        ]
    );
    let found = runtime_call_contract_at(&ordinary, &apply.body, at).unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/runtime-ordered-debts", assert_eq!(
        (found.debts, found.guarantees),
        (debts(apply), vec![lt(CallTerm::Result(result), CallTerm::Argument(scalar(apply, 2)))])
    ));

    let mut terminal_edges = Vec::new();
    for source in [TERMINAL, RETURN_OR_TERMINAL] {
        let package = lower(source);
        let apply = machine(&package, "apply");
        let [at @ RuntimeCallAt::TerminalEdge(block)] = runtime_calls(&apply.body)[..] else {
            panic!("one terminal runtime call")
        };
        let BlockTerminator::TerminalCall { id, requirements, .. } = &apply.body.blocks[block].terminator else {
            unreachable!()
        };
        let site = CallSite::Edge(*id);
        assert_eq!(
            requirements,
            &[
                CallRequirement { site, clause: 0 },
                CallRequirement { site, clause: 1 }
            ]
        );
        let found = runtime_call_contract_at(&package, &apply.body, at).unwrap();
        terminal_edges.push((found.debts == debts(apply), found.guarantees));
        let returns = apply
            .body
            .blocks
            .iter()
            .filter(|b| matches!(b.terminator, BlockTerminator::Return { .. }))
            .count();
        assert_eq!(returns, usize::from(source == RETURN_OR_TERMINAL), "only the constant arm returns");
    }
    let guarantee = |m: &LoweredMachine| vec![lt(CallTerm::Returned, CallTerm::Argument(scalar(m, 2)))];
    let witness = guarantee(machine(&lower(TERMINAL), "apply"));
    spec_corpus::observe_handoff!(psi "psi-output/runtime-terminal-debts", assert_eq!(
        terminal_edges,
        [(true, witness.clone()), (true, witness)]
    ));
}

/// An exclusive view chosen by one `match` and a descriptor chosen by a second
/// `match` on the same flag meet at one runtime call. Each view arrives by
/// transfer, the call measures the join's own parameter and invokes the joined
/// descriptor, and it owes the contract's precondition on that length. Choosing
/// the same view on both arms transfers it on both edges. Measuring `left` after
/// the join keeps that read on the formal, which the true edge transferred; only
/// the checker can refuse that path.
#[test]
fn exclusive_and_descriptor_joins_meet_at_one_runtime_call() {
    let contract = CallableContractId::from_index(0).unwrap();
    let expected = |package: &LoweredPackage, view_arrivals, descriptor_arrivals, measured, measured_after_transfer| {
        let pick = machine(package, "pick");
        let block = pick
            .body
            .blocks
            .iter()
            .find(|b| !pick.body.operations[b.operations.clone()].is_empty())
            .unwrap();
        let [length, call] = &pick.body.operations[block.operations.clone()] else {
            panic!("a length then a call")
        };
        let (OperationResult::Scalar(length), OperationResult::Scalar(result)) = (length.result, call.result) else {
            panic!("scalars")
        };
        JoinedRuntimeCall {
            view_arrivals,
            descriptor_arrivals,
            measured,
            measured_after_transfer,
            call: psi_handoff_consumer::product_witness::RuntimeCallContract {
                contract,
                debts: vec![lt(CallTerm::Argument(length), int(1000))],
                guarantees: vec![lt(CallTerm::Result(result), int(200))],
            },
        }
    };
    let combined = lower(COMBINED);
    let found = joined_runtime_call(&combined, &machine(&combined, "pick").body).unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/exclusive-descriptor-joined-call", assert_eq!(
        found,
        expected(&combined, [(true, 1), (false, 2)], Some([(true, 3), (false, 4)]), MeasuredView::Joined, vec![])
    ));

    let same = lower(SAME_FORMAL);
    assert_eq!(
        joined_runtime_call(&same, &machine(&same, "pick").body).unwrap(),
        expected(&same, [(true, 1), (false, 1)], None, MeasuredView::Joined, vec![])
    );

    let reused = lower(MOVED_THEN_USED);
    spec_corpus::observe_handoff!(psi "psi-output/exclusive-view-read-after-transfer", assert_eq!(
        joined_runtime_call(&reused, &machine(&reused, "pick").body).unwrap(),
        expected(&reused, [(true, 1), (false, 2)], None, MeasuredView::Formal(1), vec![true])
    ));
}

#[path = "support/source.rs"]
mod sources;

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
    let package = lower(
        "machine leaf(x:u64)->u64 {x}
         machine count(n:u64)->u64 terminates by n {
            transition n == 0 {true -> leaf(n), false -> count(n - 1),}
         }",
    )
    .unwrap();
    let count = machine(&package, "count");
    let leaf = machine(&package, "leaf");
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
    assert_eq!(edge.target, count.id);
    assert_eq!(*arguments, [CallArgument::Scalar(decremented)]);
    assert_eq!(edge.successor, RankExpression::Operand(decremented));
    assert_ne!(decremented, n);
    assert_eq!(edge.comparison, RankComparison::Strict);
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
    assert_eq!(
        edge.successor,
        RankExpression::SaturatingDistance {
            cursor: operation_result(climb, IntegerPrimitive::Add, i),
            limit,
        }
    );
    assert_eq!(edge.comparison, RankComparison::Strict);
}

#[test]
fn an_acyclic_witness_keeps_its_bounds_and_claims_no_edges() {
    for (range, inclusive, upper) in [("0..=9", true, 9), ("0..10", false, 10)] {
        let package = lower(&format!("machine visit(n:u64) terminates by n in {range} {{}}")).unwrap();
        let visit = machine(&package, "visit");
        let witness = ranking(visit);
        assert_eq!(witness.component, [visit.id]);
        assert!(witness.edges.is_empty());
        let bounds = witness.bounds.as_ref().unwrap();
        assert_eq!(
            (bounds.lower.clone(), bounds.upper.clone(), bounds.upper_inclusive),
            (0.into(), upper.into(), inclusive)
        );
    }
}

#[test]
fn an_unranked_productive_cycle_stays_unranked() {
    let package = lower("machine spin(n:u64)->u64 {transition {_ -> spin(n),}}").unwrap();
    assert!(machine(&package, "spin").contract.ranking.is_none());
}

#[test]
fn unsupported_or_incomplete_ranked_components_refuse() {
    for (source, expected) in [
        (
            "machine count(n:u64)->u64 terminates by n {match n == 0 {true -> 7, false -> count(n - 1),}}",
            "ranked recursion through an ordinary call cannot lower to constant-stack iteration",
        ),
        (
            "machine first(n:u64)->u64 {transition {_ -> second(n),}}
             machine second(n:u64)->u64 terminates by n {transition n == 0 {true -> 9, false -> first(n - 1),}}",
            "every member of a ranked recursive component requires its own `terminates by` witness",
        ),
        (
            "machine count(n:u64)->u64 terminates {transition n == 0 {true -> 7, false -> count(n - 1),}}",
            "a cyclic terminating component requires a `terminates by` witness",
        ),
        (
            "const K:u64 = 3; machine f(n:u64) terminates by K {}",
            "ranking subject must name a machine formal",
        ),
    ] {
        let error = lower(source).unwrap_err();
        assert_eq!(error.message, expected, "{source}");
    }
}

#[test]
fn selected_cleanup_participates_in_the_recursive_component() {
    let source = "data Lease dropped by finish {mark:u8;}
        machine again(n:u64) terminates by n { let lease=Lease {mark:0}; }
        machine finish(x:Lease) { let Lease {mark as _}=x; again(3); }";
    let error = lower(source).unwrap_err();
    assert!(
        error
            .message
            .contains("every member of a ranked recursive component"),
        "{error:?}"
    );
    let ordinary = source.replace("again(3);", "");
    let package = lower(&ordinary).unwrap();
    let caller = machine(&package, "again");
    assert_eq!(ranking(caller).component, vec![caller.id]);
}

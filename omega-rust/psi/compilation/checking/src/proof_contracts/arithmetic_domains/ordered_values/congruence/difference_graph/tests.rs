use super::super::super::RelationConstraint;
use super::super::ValueComparison;
use super::{DifferenceGraph, Operand, Relation};

fn place(position: u32) -> Operand {
    Operand::Place {
        root: symbols::SymbolHandle::from_arena_index(position),
        path: format!("place{position}"),
    }
}

fn relation(left: Operand, right: Operand, floor: i64) -> Relation {
    Relation {
        left,
        right,
        constraint: RelationConstraint::MinimumDifference(floor),
    }
}

// The former symbolic walk, deliberately kept independent of vertex indices,
// edge storage and cached rows. It pins order-sensitive finite precision even
// for contradictory cycles, rather than assuming unbounded shortest paths.
fn reference_gaps<'relations>(
    relations: &'relations [Relation],
    value: &'relations Operand,
) -> Vec<(&'relations Operand, i64)> {
    let mut operands = vec![value];
    for relation in relations {
        for operand in [&relation.left, &relation.right] {
            if !operands.contains(&operand) {
                operands.push(operand);
            }
        }
    }
    let mut distances = vec![(value, 0_i64)];
    for _ in 1..operands.len() {
        let mut improved = false;
        for relation in relations {
            let Some(floor) = relation.minimum_difference() else {
                continue;
            };
            let Some(gap) = distances
                .iter()
                .find(|(known, _)| *known == &relation.right)
                .map(|(_, gap)| *gap)
            else {
                continue;
            };
            let Some(gap) = gap.checked_add(floor) else {
                continue;
            };
            match distances
                .iter_mut()
                .find(|(known, _)| *known == &relation.left)
            {
                Some((_, known)) if *known >= gap => {}
                Some((_, known)) => {
                    *known = gap;
                    improved = true;
                }
                None => {
                    distances.push((&relation.left, gap));
                    improved = true;
                }
            }
        }
        if !improved {
            break;
        }
    }
    distances
}

#[test]
fn compact_difference_rows_match_symbolic_walk_with_cycles_and_overflow() {
    let operands = (1..=7).map(place).collect::<Vec<_>>();
    let floors = [i64::MIN, -4, -1, 0, 1, 4, i64::MAX];
    let mut random = 0x853c_49e6_748f_ea9b_u64;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        random
    };
    for case in 0..256 {
        let mut relations = Vec::new();
        for _ in 0..case % 19 {
            let left = operands[(next() % 6) as usize].clone();
            let right = operands[(next() % 6) as usize].clone();
            let constraint = if next() % 5 == 0 {
                RelationConstraint::Unequal
            } else {
                RelationConstraint::MinimumDifference(
                    floors[(next() % floors.len() as u64) as usize],
                )
            };
            relations.push(Relation {
                left,
                right,
                constraint,
            });
        }
        let mut graph = DifferenceGraph::new(&relations);
        // Repeated and reversed query order also checks cached row isolation.
        for from in operands.iter().chain(operands.iter().rev()) {
            let expected = reference_gaps(&relations, from);
            for to in &operands {
                assert_eq!(
                    graph.nonnegative_gap(from, to),
                    expected
                        .iter()
                        .any(|(operand, gap)| *operand == to && *gap >= 0),
                    "case {case}, {from:?} -> {to:?}, relations {relations:?}"
                );
            }
        }
        let endpoints = graph.operands().to_vec();
        for (source, from) in endpoints.iter().enumerate() {
            let forward = reference_gaps(&relations, from);
            for (destination, to) in endpoints.iter().enumerate() {
                let reverse = reference_gaps(&relations, to);
                let expected = from == to
                    || (forward
                        .iter()
                        .any(|(operand, gap)| operand == to && *gap >= 0)
                        && reverse
                            .iter()
                            .any(|(operand, gap)| operand == from && *gap >= 0));
                assert_eq!(
                    graph.mutual_nonnegative_gap_indexed(source, destination),
                    expected,
                    "indexed mutual case {case}, {from:?} <-> {to:?}, relations {relations:?}"
                );
            }
        }
    }
}

#[test]
fn endpoint_order_retains_unequal_vertices_and_original_edge_order() {
    let first = place(3);
    let second = place(1);
    let third = place(2);
    let relations = [
        Relation {
            left: first.clone(),
            right: second.clone(),
            constraint: RelationConstraint::Unequal,
        },
        relation(third.clone(), second.clone(), i64::MAX),
        relation(second.clone(), third.clone(), i64::MIN),
        relation(third.clone(), second.clone(), 0),
    ];
    let mut graph = DifferenceGraph::new(&relations);
    assert_eq!(graph.operands(), [&first, &second, &third]);
    assert_eq!(
        graph.indexed_relations().collect::<Vec<_>>(),
        [(2, 1, i64::MAX), (1, 2, i64::MIN), (2, 1, 0)]
    );
    assert!(graph.mutual_nonnegative_gap_indexed(0, 0));
    assert!(!graph.mutual_nonnegative_gap_indexed(0, 1));
}

#[test]
fn constructor_congruence_keeps_denotations_events_and_declaration_generations() {
    let left = place(1);
    let right = place(2);
    let relations = [
        relation(left.clone(), right.clone(), 0),
        relation(right.clone(), left.clone(), 0),
    ];
    let mut comparison = ValueComparison::new(&relations);
    // Only admitted repeatable denotations enter Operand::Call. Runtime
    // event leaves keep exact occurrence identity and gain no child congruence.
    let call = |argument: Operand, target| Operand::Call {
        target,
        arguments: vec![argument],
    };
    let target = symbols::SymbolHandle::from_parts(3, 1);
    let repeated_left = call(left.clone(), target);
    let repeated_right = call(right.clone(), target);
    assert!(comparison.same_value(&repeated_left, &repeated_right));
    let different_target = call(right.clone(), symbols::SymbolHandle::from_parts(4, 1));
    let stale_target = call(right.clone(), symbols::SymbolHandle::from_parts(3, 2));
    assert!(!comparison.same_value(&repeated_left, &different_target));
    assert!(!comparison.same_value(&repeated_left, &stale_target));
    let first_event = Operand::CompletedCall {
        target: symbols::SymbolHandle::from_arena_index(3),
        occurrence: typed_trees::expression::ExpressionHandle::from_parts(4, 1),
    };
    let second_event = Operand::CompletedCall {
        target: symbols::SymbolHandle::from_arena_index(3),
        occurrence: typed_trees::expression::ExpressionHandle::from_parts(4, 2),
    };
    assert!(!comparison.same_value(&first_event, &second_event));
    let another_event = Operand::CompletedCall {
        target: symbols::SymbolHandle::from_arena_index(3),
        occurrence: typed_trees::expression::ExpressionHandle::from_parts(5, 1),
    };
    assert!(!comparison.same_value(&first_event, &another_event));
    assert!(first_event.survives(&[]));
    assert!(!first_event.survives(&["unrelated".into()]));
    assert!(
        first_event
            .rebound(&[(left.clone(), right.clone())])
            .is_empty()
    );
    assert!(!comparison.same_value(&first_event, &repeated_left));
    let stale = Operand::Place {
        root: symbols::SymbolHandle::from_parts(1, 2),
        path: "place1".into(),
    };
    assert!(!comparison.same_value(&left, &stale));
    assert!(comparison.same_value(
        &Operand::CollectionLength(Box::new(left)),
        &Operand::CollectionLength(Box::new(right))
    ));
}

#[test]
fn a_new_relation_environment_does_not_inherit_equalities() {
    let left = place(1);
    let right = place(2);
    let forward = relation(left.clone(), right.clone(), 0);
    let reverse = relation(right.clone(), left.clone(), 0);
    assert!(ValueComparison::new(&[forward.clone(), reverse]).same_value(&left, &right));
    assert!(!ValueComparison::new(&[forward]).same_value(&left, &right));
    assert!(
        !ValueComparison::new(&[Relation {
            left: left.clone(),
            right: right.clone(),
            constraint: RelationConstraint::Unequal
        }])
        .same_value(&left, &right)
    );
    assert!(!ValueComparison::new(&[]).same_value(&left, &right));
}

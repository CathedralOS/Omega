//! The ignored driver retains the same input and query mix across producer builds.
use super::*;
use semantic_vocabulary::{ContractId, EdgeId};
use std::hint::black_box;
use std::time::Instant;
use terminal_check_tables::edges::EdgeRow;
use terminal_psi::{Block, MachineContract, Terminator};

fn block(raw: usize) -> BlockId {
    BlockId::new(raw as u64 + 1).unwrap()
}

fn fixture(count: usize, edges: &[(usize, usize)]) -> (TerminalMachine, MachineTables) {
    let mut tables = MachineTables::default();
    for &(source, target) in edges {
        tables.edges.push(EdgeRow {
            source: block(source),
            destination: Destination::Block(block(target)),
            dispositions: Vec::new(),
        });
    }
    let lowered = TerminalMachine {
        call_formations: Vec::new(),
        assembly_target_requirement: Default::default(),
        id: MachineId::new(1).unwrap(),
        attachment: None,
        nominal_cleanup_subject: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block(0),
        blocks: (0..count)
            .map(|position| {
                Block::plain(
                    block(position),
                    Vec::new(),
                    Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                )
            })
            .collect(),
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    };
    (lowered, tables)
}

fn shaped_edges(shape: &str, count: usize) -> Vec<(usize, usize)> {
    match shape {
        "chain" => (1..count).map(|target| (target - 1, target)).collect(),
        "reverse_chain" => std::iter::once((0, count - 1))
            .chain((2..count).map(|source| (source, source - 1)))
            .collect(),
        "diamonds" => (0..count.saturating_sub(3))
            .step_by(3)
            .flat_map(|start| {
                [
                    (start, start + 1),
                    (start, start + 2),
                    (start + 1, start + 3),
                    (start + 2, start + 3),
                ]
            })
            .collect(),
        "loops" => (1..count)
            .map(|target| (target - 1, target))
            .chain((2..count).step_by(8).map(|source| (source, source - 2)))
            .collect(),
        "disconnected" => (1..count)
            .filter(|target| target % 8 != 0)
            .map(|target| (target - 1, target))
            .collect(),
        _ => panic!("unknown dominance shape"),
    }
}

fn retained_bytes(graph: &Graph) -> usize {
    std::mem::size_of::<Graph>()
        + graph.dominators.capacity() * std::mem::size_of::<Vec<bool>>()
        + graph
            .dominators
            .iter()
            .map(|row| row.capacity())
            .sum::<usize>()
        + graph.operations.capacity() * std::mem::size_of::<Option<(BlockId, usize)>>()
        + graph.predecessors.capacity() * std::mem::size_of::<Vec<BlockId>>()
        + graph
            .predecessors
            .iter()
            .map(|row| row.capacity() * std::mem::size_of::<BlockId>())
            .sum::<usize>()
        + graph.tables_edges.capacity() * std::mem::size_of::<(BlockId, Destination)>()
}

#[test]
#[ignore = "one retained scaling observation; set OMEGA_DOMINANCE_SHAPE and OMEGA_DOMINANCE_BLOCKS"]
fn scaling_observation() {
    let shape = std::env::var("OMEGA_DOMINANCE_SHAPE").unwrap_or_else(|_| "chain".into());
    let count = std::env::var("OMEGA_DOMINANCE_BLOCKS")
        .unwrap_or_else(|_| "1000".into())
        .parse::<usize>()
        .unwrap();
    assert!(count >= 4);
    let edges = shaped_edges(&shape, count);
    let (lowered, tables) = fixture(count, &edges);
    let started = Instant::now();
    let graph = Graph::new(lowered.id, black_box(&lowered), black_box(&tables)).unwrap();
    let build_ns = started.elapsed().as_nanos();
    let retained_bytes = retained_bytes(&graph);
    let started = Instant::now();
    let mut checksum = 0u64;
    let queries = 100_000usize;
    for query in 0..queries {
        let source = query.wrapping_mul(17) % count;
        let target = query.wrapping_mul(31).wrapping_add(7) % count;
        checksum += u64::from(
            black_box(&graph)
                .dominates(
                    ProgramPoint::BlockEntry(block(source)),
                    ProgramPoint::BlockEntry(block(target)),
                )
                .unwrap(),
        );
    }
    let query_ns = started.elapsed().as_nanos();
    println!(
        "{{\"shape\":\"{shape}\",\"blocks\":{count},\"edges\":{},\"queries\":{queries},\"build_ns\":{build_ns},\"query_ns\":{query_ns},\"retained_bytes\":{retained_bytes},\"checksum\":{checksum}}}",
        edges.len()
    );
}

/// An independent oracle: infer source SCCs by transitive closure, then remove
/// one candidate block and search from the explicit virtual entries. It shares
/// neither the immediate-dominator recurrence nor traversal intervals.
fn oracle(count: usize, edges: &[(usize, usize)]) -> Vec<Vec<bool>> {
    let mut reaches = vec![vec![false; count]; count];
    for (source, row) in reaches.iter_mut().enumerate() {
        row[source] = true;
    }
    for &(source, target) in edges {
        reaches[source][target] = true;
    }
    for middle in 0..count {
        for source in 0..count {
            for target in 0..count {
                let through = reaches[source][middle] && reaches[middle][target];
                reaches[source][target] |= through;
            }
        }
    }
    let roots: Vec<_> = (0..count)
        .filter(|&candidate| {
            candidate == 0
                || (!reaches[0][candidate]
                    && !edges.iter().any(|&(source, target)| {
                        reaches[candidate][target]
                            && reaches[target][candidate]
                            && !(reaches[candidate][source] && reaches[source][candidate])
                    }))
        })
        .collect();
    (0..count)
        .map(|removed| {
            let mut visited = vec![false; count];
            let mut pending = roots.clone();
            while let Some(source) = pending.pop() {
                if source == removed || std::mem::replace(&mut visited[source], true) {
                    continue;
                }
                pending.extend(
                    edges
                        .iter()
                        .filter_map(|&(from, target)| (from == source).then_some(target)),
                );
            }
            visited.into_iter().map(|reachable| !reachable).collect()
        })
        .collect()
}

#[test]
fn every_four_block_graph_matches_independent_path_exclusion() {
    let count = 4;
    for mask in 0..(1u32 << (count * count)) {
        let edges: Vec<_> = (0..count)
            .flat_map(|source| (0..count).map(move |target| (source, target)))
            .filter(|&(source, target)| mask & (1 << (source * count + target)) != 0)
            .collect();
        let (lowered, tables) = fixture(count, &edges);
        let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
        let expected = oracle(count, &edges);
        for (source, row) in expected.iter().enumerate() {
            for (target, &expected) in row.iter().enumerate() {
                assert_eq!(
                    graph
                        .dominates(
                            ProgramPoint::BlockEntry(block(source)),
                            ProgramPoint::BlockEntry(block(target))
                        )
                        .unwrap(),
                    expected,
                    "edges={edges:?}, source={source}, target={target}"
                );
            }
        }
    }
}

#[test]
fn rootless_cycle_cannot_donate_facts_but_its_exit_keeps_local_dominance() {
    let (lowered, tables) = fixture(5, &[(1, 2), (2, 1), (2, 3), (3, 4)]);
    let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
    let dominates = |source, target| {
        graph
            .dominates(
                ProgramPoint::BlockEntry(block(source)),
                ProgramPoint::BlockEntry(block(target)),
            )
            .unwrap()
    };
    assert!(!dominates(0, 1));
    assert!(!dominates(1, 2));
    assert!(!dominates(2, 1));
    assert!(dominates(2, 3));
    assert!(dominates(3, 4));
}

#[test]
fn an_entry_backedge_does_not_establish_an_initial_entry_fact() {
    let (lowered, tables) = fixture(1, &[(0, 0)]);
    let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
    assert!(
        !graph
            .dominates(
                ProgramPoint::Edge(EdgeId::new(1).unwrap()),
                ProgramPoint::BlockEntry(block(0))
            )
            .unwrap()
    );
}

#[test]
fn duplicate_incoming_edges_do_not_make_either_edge_available_after_the_join() {
    let (lowered, tables) = fixture(3, &[(0, 1), (0, 1), (1, 2)]);
    let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
    assert_eq!(graph.predecessors(block(1)).unwrap(), &[block(0), block(0)]);
    assert!(
        !graph
            .dominates(
                ProgramPoint::Edge(EdgeId::new(1).unwrap()),
                ProgramPoint::BlockEntry(block(2))
            )
            .unwrap()
    );
    assert!(
        graph
            .dominates(
                ProgramPoint::Edge(EdgeId::new(3).unwrap()),
                ProgramPoint::BlockEntry(block(2))
            )
            .unwrap()
    );
}

#[test]
fn foreign_points_refuse_even_when_compared_with_themselves() {
    use semantic_vocabulary::OperationId;
    let (lowered, tables) = fixture(1, &[]);
    let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
    for point in [
        ProgramPoint::BlockEntry(block(1)),
        ProgramPoint::AfterOperation(OperationId::new(1).unwrap()),
        ProgramPoint::Edge(EdgeId::new(1).unwrap()),
    ] {
        assert!(graph.dominates(point, point).is_err());
        assert!(
            graph
                .dominates(ProgramPoint::BlockEntry(block(0)), point)
                .is_err()
        );
    }
}

#[test]
fn malformed_topology_refuses_before_queries() {
    let (mut lowered, mut tables) = fixture(2, &[(0, 1)]);
    tables
        .edges
        .get_mut(EdgeId::new(1).unwrap())
        .unwrap()
        .source = block(2);
    assert!(Graph::new(lowered.id, &lowered, &tables).is_err());
    tables
        .edges
        .get_mut(EdgeId::new(1).unwrap())
        .unwrap()
        .source = block(0);
    tables
        .edges
        .get_mut(EdgeId::new(1).unwrap())
        .unwrap()
        .destination = Destination::Block(block(2));
    assert!(Graph::new(lowered.id, &lowered, &tables).is_err());
    lowered.blocks[1].id = block(0);
    assert!(Graph::new(lowered.id, &lowered, &tables).is_err());
}

#[test]
fn operation_order_and_normal_successor_availability_are_preserved() {
    use semantic_vocabulary::{OperationId, ScalarQualificationSetId, ScalarType, ValueId};
    use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};
    let (mut lowered, tables) = fixture(2, &[(0, 1)]);
    lowered.blocks[0].operations = (1..=2)
        .map(|raw| {
            Operation::unsuspended(
                OperationId::new(raw).unwrap(),
                OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(raw).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    qualifications: ScalarQualificationSetId::ZERO,
                }),
                OperationKind::BooleanConstant { value: true },
            )
        })
        .collect();
    let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
    let entry = ProgramPoint::BlockEntry(block(0));
    let first = ProgramPoint::AfterOperation(OperationId::new(1).unwrap());
    let second = ProgramPoint::AfterOperation(OperationId::new(2).unwrap());
    let edge = ProgramPoint::Edge(EdgeId::new(1).unwrap());
    assert!(graph.dominates(entry, first).unwrap());
    assert!(!graph.dominates(first, entry).unwrap());
    assert!(graph.dominates(first, second).unwrap());
    assert!(!graph.dominates(second, first).unwrap());
    assert!(graph.dominates(second, edge).unwrap());
    assert!(
        graph
            .dominates(first, ProgramPoint::BlockEntry(block(1)))
            .unwrap()
    );
    let duplicate = lowered.blocks[0].operations[0].clone();
    lowered.blocks[1].operations.push(duplicate);
    assert!(Graph::new(lowered.id, &lowered, &tables).is_err());
}

#[test]
fn large_graphs_keep_iterative_traversal_and_exact_answers() {
    for shape in [
        "chain",
        "reverse_chain",
        "diamonds",
        "loops",
        "disconnected",
    ] {
        let count = 20_000;
        let edges = shaped_edges(shape, count);
        let (lowered, tables) = fixture(count, &edges);
        let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
        assert_eq!(graph.reverse_postorder().unwrap().len(), count);
        for position in (0..count).step_by(97) {
            assert!(
                graph
                    .dominates(
                        ProgramPoint::BlockEntry(block(position)),
                        ProgramPoint::BlockEntry(block(position))
                    )
                    .unwrap()
            );
        }
        assert!(
            retained_bytes(&graph) < count * 256,
            "{shape} retains nonlinear metadata"
        );
    }
}

#[test]
fn extra_table_edges_remove_dominance_without_changing_terminators() {
    let (lowered, mut tables) = fixture(3, &[(0, 1), (1, 2)]);
    let definition = ProgramPoint::BlockEntry(block(1));
    let used = ProgramPoint::BlockEntry(block(2));
    assert!(
        Graph::new(lowered.id, &lowered, &tables)
            .unwrap()
            .dominates(definition, used)
            .unwrap()
    );
    tables.edges.push(EdgeRow {
        source: block(0),
        destination: Destination::Block(block(2)),
        dispositions: Vec::new(),
    });
    assert!(
        !Graph::new(lowered.id, &lowered, &tables)
            .unwrap()
            .dominates(definition, used)
            .unwrap()
    );
}

#[test]
fn traversal_order_and_a_nonfirst_entry_keep_their_meaning() {
    let (mut lowered, tables) = fixture(6, &[(0, 1), (0, 2), (1, 3), (2, 3), (4, 5)]);
    let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
    assert_eq!(
        graph
            .reverse_postorder()
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![block(4), block(5), block(0), block(2), block(1), block(3)]
    );
    lowered.entry = block(2);
    let graph = Graph::new(lowered.id, &lowered, &tables).unwrap();
    assert!(
        !graph
            .dominates(
                ProgramPoint::BlockEntry(block(2)),
                ProgramPoint::BlockEntry(block(3))
            )
            .unwrap()
    );
    assert_eq!(graph.entry(), block(2));
}

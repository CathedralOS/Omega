use super::{
    BTreeMap, BTreeSet, BlockId, EdgeId, TerminalMachine, Terminator, dominators, successors,
};
use semantic_vocabulary::{ContractId, MachineId, PlaceId, StructuralCaseId};
use terminal_psi::{Block, MachineContract, StructuralCaseSuccessorEdge, TerminalMachineResult};

// Only graph topology is under test. Structural cases let the fixture encode
// arbitrary fanout without inventing scalar operations or proof evidence.
fn machine(identities: &[u64], adjacency: &[Vec<usize>], entry: usize) -> TerminalMachine {
    let mut next_edge = 1;
    TerminalMachine {
        id: MachineId::new(1).unwrap(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block(identities[entry]),
        blocks: adjacency
            .iter()
            .enumerate()
            .map(|(position, targets)| Block {
                erased_scalar_formals: Vec::new(),
                id: block(identities[position]),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: if targets.is_empty() {
                    let edge = EdgeId::new(next_edge).unwrap();
                    next_edge += 1;
                    Terminator::ReturnUnit {
                        edge,
                        trivial_affine_discards: Vec::new(),
                    }
                } else {
                    Terminator::StructuralCase {
                        source: PlaceId::new(1).unwrap(),
                        cases: targets
                            .iter()
                            .enumerate()
                            .map(|(case, &target)| {
                                let edge = EdgeId::new(next_edge).unwrap();
                                next_edge += 1;
                                StructuralCaseSuccessorEdge {
                                    edge,
                                    target: block(identities[target]),
                                    case: StructuralCaseId::new(case as u64 + 1).unwrap(),
                                    payload_fields: Vec::new(),
                                    trivial_affine_discards: Vec::new(),
                                }
                            })
                            .collect(),
                    }
                },
            })
            .collect(),
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn block(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

/// The former production set-intersection algorithm remains test-only as an
/// independent oracle: it knows neither tree parents nor traversal intervals.
fn reference_dominators(machine: &TerminalMachine) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let outgoing = successors(machine);
    let all_blocks = outgoing.keys().copied().collect::<BTreeSet<_>>();
    let mut predecessors = all_blocks
        .iter()
        .map(|block| (*block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (block, edges) in &outgoing {
        for (_, target) in edges {
            predecessors.get_mut(target).unwrap().insert(*block);
        }
    }
    let mut result = all_blocks
        .iter()
        .map(|block| {
            (
                *block,
                if *block == machine.entry {
                    BTreeSet::from([*block])
                } else {
                    all_blocks.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in all_blocks
            .iter()
            .copied()
            .filter(|block| *block != machine.entry)
        {
            let mut incoming = predecessors[&block].iter();
            let mut common = result[incoming.next().unwrap()].clone();
            for predecessor in incoming {
                common.retain(|candidate| result[predecessor].contains(candidate));
            }
            common.insert(block);
            if result[&block] != common {
                result.insert(block, common);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

fn compare_reference(machine: &TerminalMachine) {
    let expected = reference_dominators(machine);
    let actual = dominators(machine);
    assert_eq!(actual.positions.len(), machine.blocks.len());
    assert_eq!(actual.nodes.len(), machine.blocks.len());
    for definition in &machine.blocks {
        assert_eq!(
            actual.depth(definition.id),
            Some(expected[&definition.id].len())
        );
        for use_block in &machine.blocks {
            assert_eq!(
                actual.dominates(definition.id, use_block.id),
                expected[&use_block.id].contains(&definition.id),
                "definition {:?}, use {:?}, graph {:?}",
                definition.id,
                use_block.id,
                successors(machine),
            );
        }
    }
}

#[test]
fn dominance_matches_sets_for_every_reachable_graph_through_four_blocks() {
    for count in 1..=4 {
        let identities = (1..=count as u64).collect::<Vec<_>>();
        for edge_mask in 0u32..(1 << (count * count)) {
            let adjacency = (0..count)
                .map(|source| {
                    (0..count)
                        .filter(|target| edge_mask & (1 << (source * count + target)) != 0)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            // Reachability filtering is independent of the production DFS.
            let mut reached = BTreeSet::from([0]);
            loop {
                let previous = reached.len();
                for (source, targets) in adjacency.iter().enumerate() {
                    if reached.contains(&source) {
                        reached.extend(targets.iter().copied());
                    }
                }
                if reached.len() == previous {
                    break;
                }
            }
            if reached.len() == count {
                compare_reference(&machine(&identities, &adjacency, 0));
            }
        }
    }
}

#[test]
fn generated_cyclic_graphs_match_sets_with_permuted_sparse_identities() {
    let mut state = 0x7b25_ade6_8294_103du64;
    for count in 5..=32 {
        for variant in 0..16 {
            let identities = (0..count)
                .map(|position| u64::MAX - ((position * 19 % count) as u64 * 1_000_003))
                .collect::<Vec<_>>();
            // Multiplication by 19 is a permutation except at count 19.
            let identities = if count == 19 {
                (0..count)
                    .map(|position| u64::MAX - position as u64 * 1_000_003)
                    .collect()
            } else {
                identities
            };
            let mut adjacency = vec![Vec::new(); count];
            for (source, targets) in adjacency.iter_mut().enumerate() {
                // A ring guarantees reachability from any selected entry.
                targets.push((source + 1) % count);
                for target in 0..count {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if state >> 60 == 0 {
                        targets.push(target);
                    }
                }
            }
            let mut graph = machine(&identities, &adjacency, variant % count);
            graph.blocks.reverse();
            compare_reference(&graph);
        }
    }
}

#[test]
fn irreducible_entries_and_entry_backedges_do_not_invent_dominance() {
    let graph = machine(
        &[900, 7, u64::MAX, 42, 103],
        &[vec![1, 2], vec![2, 3], vec![1, 3], vec![0, 4], vec![]],
        0,
    );
    compare_reference(&graph);
    let tree = dominators(&graph);
    assert!(!tree.dominates(block(7), block(u64::MAX)));
    assert!(!tree.dominates(block(u64::MAX), block(7)));
    assert!(tree.dominates(block(42), block(103)));
    assert_eq!(tree.depth(block(900)), Some(1));
    let absent = block(1);
    assert_eq!(tree.depth(absent), None);
    assert!(!tree.dominates(absent, graph.entry));
    assert!(!tree.dominates(graph.entry, absent));
    assert!(!tree.dominates(absent, absent));
}

#[test]
fn cyclic_header_backedge_preserves_acyclic_dominance() {
    // A counted-loop shape enters the header directly and retains exactly one
    // edge from a different block back to that header.
    let full = machine(
        &[600, 17, 903, 41, 201, 3],
        &[vec![1], vec![2, 5], vec![3, 4], vec![1], vec![5], vec![]],
        0,
    );
    let mut cut = full.clone();
    cut.blocks[3].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(999).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    compare_reference(&full);
    compare_reference(&cut);
    let full_tree = dominators(&full);
    let cut_tree = dominators(&cut);
    for definition in &full.blocks {
        assert_eq!(
            full_tree.depth(definition.id),
            cut_tree.depth(definition.id)
        );
        for use_block in &full.blocks {
            assert_eq!(
                full_tree.dominates(definition.id, use_block.id),
                cut_tree.dominates(definition.id, use_block.id),
            );
        }
    }
}

#[test]
fn long_chains_retain_linear_entries_and_use_iterative_traversal() {
    for count in [1, 64, 4096, 32768] {
        let identities = (0..count)
            .map(|position| u64::MAX - position as u64 * 101)
            .collect::<Vec<_>>();
        let adjacency = (0..count)
            .map(|position| {
                if position + 1 < count {
                    vec![position + 1]
                } else {
                    vec![]
                }
            })
            .collect::<Vec<_>>();
        let graph = machine(&identities, &adjacency, 0);
        let tree = dominators(&graph);
        // The complete retained representation has one map and one fixed-size
        // node vector, with no per-node ancestor or child collection.
        assert_eq!(tree.positions.len() + tree.nodes.len(), count * 2);
        for (position, &identity) in identities.iter().enumerate() {
            assert_eq!(tree.depth(block(identity)), Some(position + 1));
            assert!(tree.dominates(block(identity), block(identities[count - 1])));
            if position > 0 {
                assert!(!tree.dominates(block(identity), block(identities[position - 1])));
            }
        }
        if count <= 64 {
            compare_reference(&graph);
        }
    }
}

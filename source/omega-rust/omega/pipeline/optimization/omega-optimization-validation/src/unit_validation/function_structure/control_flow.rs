//! Block indexing, node metadata, edge indexing, and total-CFG validation.

use super::*;

pub(super) struct FunctionControlFlow<'a> {
    pub(super) blocks: BTreeMap<BlockId, &'a omega_optimization_unit::OptimizationBlock>,
    pub(super) predecessors: BTreeMap<BlockId, BTreeSet<BlockId>>,
    pub(super) successors: BTreeMap<BlockId, Vec<BlockId>>,
}

pub(super) fn index_blocks(
    function: &PsiOptimizationFunction,
) -> Result<
    BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    OptimizationUnitValidationError,
> {
    let mut blocks = BTreeMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBlock {
                machine: function.machine,
                block: block.id,
            });
        }
    }
    if !blocks.contains_key(&function.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryBlock {
            machine: function.machine,
            block: function.entry,
        });
    }
    if !blocks[&function.entry].parameters.is_empty() {
        return Err(OptimizationUnitValidationError::EntryBlockHasParameters {
            machine: function.machine,
            block: function.entry,
        });
    }
    Ok(blocks)
}

pub(super) fn validate_nodes_and_edges<'a>(
    function: &'a PsiOptimizationFunction,
    blocks: BTreeMap<BlockId, &'a omega_optimization_unit::OptimizationBlock>,
) -> Result<FunctionControlFlow<'a>, OptimizationUnitValidationError> {
    let mut edge_ids = BTreeSet::new();
    let mut predecessor = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut successors = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        if block.nodes.is_empty() {
            return Err(OptimizationUnitValidationError::EmptyBlock {
                machine: function.machine,
                block: block.id,
            });
        }
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).expect("unit node index was built as u32");
            if !provenance_matches_operation(&node.operation, &node.provenance)
                || node.definitions != expected_definitions(&node.operation, block.id, node_index)
                || node.uses != expected_uses(&node.operation, block.id, node_index)
                || !successors_match_operation(&node.operation, &node.successors)
                || node.ownership != expected_ownership(&node.operation)
            {
                return Err(OptimizationUnitValidationError::OperationMetadataMismatch {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                });
            }
            let terminal = is_terminator(&node.operation);
            if terminal && index + 1 != block.nodes.len() {
                return Err(OptimizationUnitValidationError::TerminatorNotLast {
                    machine: function.machine,
                    block: block.id,
                });
            }
            for edge in &node.successors {
                if !blocks.contains_key(&edge.target) {
                    return Err(OptimizationUnitValidationError::UnknownSuccessor {
                        machine: function.machine,
                        block: block.id,
                        target: edge.target,
                    });
                }
                if !edge_ids.insert(edge.psi_edge) {
                    return Err(OptimizationUnitValidationError::DuplicateEdge(
                        edge.psi_edge,
                    ));
                }
                predecessor
                    .get_mut(&edge.target)
                    .expect("known target")
                    .insert(block.id);
                successors
                    .get_mut(&block.id)
                    .expect("every block has a successor row")
                    .push(edge.target);
            }
        }
        if !is_terminator(&block.nodes.last().expect("nonempty").operation) {
            return Err(OptimizationUnitValidationError::MissingTerminator {
                machine: function.machine,
                block: block.id,
            });
        }
    }

    Ok(FunctionControlFlow {
        blocks,
        predecessors: predecessor,
        successors,
    })
}

pub(crate) fn validate_total_cfg(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors[&block].iter().copied());
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different block counts have an unreachable block");
        return Err(OptimizationUnitValidationError::UnreachableBlock {
            machine: function.machine,
            block,
        });
    }

    let mut indegree = blocks
        .keys()
        .copied()
        .map(|block| (block, 0usize))
        .collect::<BTreeMap<_, _>>();
    for target in successors.values().flatten() {
        *indegree.get_mut(target).expect("successor was validated") += 1;
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut visited = 0usize;
    while let Some(block) = ready.pop_first() {
        visited += 1;
        for target in &successors[&block] {
            let count = indegree.get_mut(target).expect("successor was validated");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if visited != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(OptimizationUnitValidationError::ControlCycle {
            machine: function.machine,
            block,
        });
    }
    Ok(())
}

pub(crate) fn block_reaches(
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    start: BlockId,
    target: BlockId,
) -> bool {
    let mut visited = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if visited.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    false
}

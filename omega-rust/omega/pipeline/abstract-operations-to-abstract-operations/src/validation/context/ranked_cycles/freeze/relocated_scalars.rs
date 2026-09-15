//! Optimizer module role: validation leaf. Exact ranked-body relocation normalization.
//!
//! A transformed cyclic body stays frozen against the reconstructed seed
//! except for independently admitted scalar motion: an admissible
//! source-owned scalar constant leaf, or an admissible scalar computation
//! whose uses are all defined outside the component or name provably
//! invariant parameters of the entry target, may relocate from one of its
//! component's member blocks into the tail of that component's unique-entry
//! preheader. Moved computations rebind each invariant-parameter operand to
//! the representative the entry edge binds — the substitution is re-derived
//! here from the seed, not trusted from the transformed unit. Definition/use
//! sites and the function-wide effect sequence are derived coordinates
//! rebuilt by core validation, so the comparison below retains every
//! source-owned field rather than the refreshed coordinates.

use super::*;

struct Moved<'function> {
    home: &'function OptimizerCycleComponent,
    expected_block: BlockId,
    expected: &'function OptimizationNode,
    current_block: BlockId,
    current_index: usize,
    current: &'function OptimizationNode,
}

pub(super) fn validate(
    machine: MachineId,
    expected: &PsiOptimizationFunction,
    current: &PsiOptimizationFunction,
    components: &[&OptimizerCycleComponent],
) -> Result<(), OptimizationUnitValidationError> {
    // A source operation left a component's member blocks when its unique
    // current occurrence sits outside that component. Missing or duplicated
    // occurrences are never admitted moves; the frozen comparison below
    // rejects them.
    let mut moved = Vec::new();
    for component in components {
        for member in &component.members {
            let expected_block =
                block(expected, *member).ok_or_else(|| mismatch(machine, *member))?;
            for node in &expected_block.nodes {
                let Some(PsiProvenance::Operation(operation)) = node.provenance.first().copied()
                else {
                    continue;
                };
                let found = occurrences(current, operation);
                let [(current_block, current_index, current_node)] = found.as_slice() else {
                    continue;
                };
                if component.members.contains(current_block) {
                    continue;
                }
                moved.push(Moved {
                    home: component,
                    expected_block: *member,
                    expected: node,
                    current_block: *current_block,
                    current_index: *current_index,
                    current: current_node,
                });
            }
        }
    }

    if moved.is_empty() {
        // Without admitted motion the complete cyclic body, including prefix
        // definitions and exit observations, stays byte-exact frozen.
        for expected_block in &expected.blocks {
            if block(current, expected_block.id) != Some(expected_block) {
                return Err(mismatch(machine, expected_block.id));
            }
        }
        for current_block in &current.blocks {
            if block(expected, current_block.id).is_none() {
                return Err(mismatch(machine, current_block.id));
            }
        }
        return Ok(());
    }

    // Each relocated node must land in its own component's unique-entry
    // preheader ahead of the terminator that owns the entry edge, and it must
    // retain every source-owned field.
    for relocation in &moved {
        let component = relocation.home;
        let [entry] = component.entries.as_slice() else {
            return Err(mismatch(machine, relocation.expected_block));
        };
        if component.members.contains(&entry.source) || relocation.current_block != entry.source {
            return Err(mismatch(machine, relocation.expected_block));
        }
        let preheader =
            block(current, entry.source).ok_or_else(|| mismatch(machine, entry.source))?;
        let Some(terminator) = preheader.nodes.last() else {
            return Err(mismatch(machine, entry.source));
        };
        if !terminator
            .successors
            .iter()
            .any(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
        {
            return Err(mismatch(machine, entry.source));
        }
        // Leaf relocations move byte-exact; invariant computations rebind
        // entry-target parameters to the representatives this validator
        // re-derives from the expected seed, so a forged operand rewrite
        // cannot carry different authority than the loop's own edges prove.
        let substitution =
            if crate::validation::admissible_scalar_leaf_relocation(relocation.expected) {
                BTreeMap::new()
            } else {
                match crate::validation::invariant_scalar_operand_substitution(
                    expected,
                    component,
                    relocation.expected,
                ) {
                    Some(substitution) => substitution,
                    None => return Err(mismatch(machine, relocation.expected_block)),
                }
            };
        if !same_relocated_node(relocation.expected, relocation.current, &substitution) {
            return Err(mismatch(machine, relocation.expected_block));
        }
    }

    // Relocated nodes sharing one preheader form the contiguous run
    // immediately ahead of that block's terminator.
    let mut destinations = BTreeMap::<BlockId, Vec<usize>>::new();
    for relocation in &moved {
        destinations
            .entry(relocation.current_block)
            .or_default()
            .push(relocation.current_index);
    }
    for (destination, mut indices) in destinations {
        indices.sort_unstable();
        let terminator = block(current, destination)
            .and_then(|block| block.nodes.len().checked_sub(1))
            .ok_or_else(|| mismatch(machine, destination))?;
        if indices.len() > terminator
            || indices
                .iter()
                .enumerate()
                .any(|(offset, index)| *index != terminator - indices.len() + offset)
        {
            return Err(mismatch(machine, destination));
        }
    }

    let relocated_operations = moved
        .iter()
        .filter_map(|relocation| match relocation.expected.provenance.first() {
            Some(PsiProvenance::Operation(operation)) => Some(*operation),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for expected_block in &expected.blocks {
        let current_block = block(current, expected_block.id)
            .ok_or_else(|| mismatch(machine, expected_block.id))?;
        if expected_block.parameters != current_block.parameters
            || expected_block.structural_parameters != current_block.structural_parameters
        {
            return Err(mismatch(machine, expected_block.id));
        }
        let expected_nodes = retained_nodes(expected_block, &relocated_operations);
        let current_nodes = retained_nodes(current_block, &relocated_operations);
        if expected_nodes.len() != current_nodes.len()
            || expected_nodes
                .iter()
                .zip(current_nodes)
                .any(|(expected, current)| !same_position_normalized_node(expected, current))
        {
            return Err(mismatch(machine, expected_block.id));
        }
    }
    for current_block in &current.blocks {
        if block(expected, current_block.id).is_none() {
            return Err(mismatch(machine, current_block.id));
        }
    }
    Ok(())
}

fn retained_nodes<'block>(
    block: &'block OptimizationBlock,
    relocated_operations: &BTreeSet<OperationId>,
) -> Vec<&'block OptimizationNode> {
    block
        .nodes
        .iter()
        .filter(|node| {
            !matches!(
                node.provenance.first(),
                Some(PsiProvenance::Operation(operation))
                    if relocated_operations.contains(operation)
            )
        })
        .collect()
}

fn same_relocated_node(
    expected: &OptimizationNode,
    current: &OptimizationNode,
    substitution: &BTreeMap<ValueId, ValueId>,
) -> bool {
    let mut operation = expected.operation.clone();
    crate::validation::substitute_invariant_scalar_operands(&mut operation, substitution);
    operation == current.operation
        && expected.provenance == current.provenance
        && expected.fuel == current.fuel
        && position_normalized_definitions(expected) == position_normalized_definitions(current)
        && expected
            .uses
            .iter()
            .map(|value_use| {
                substitution
                    .get(&value_use.value)
                    .copied()
                    .unwrap_or(value_use.value)
            })
            .eq(current.uses.iter().map(|value_use| value_use.value))
        && expected.successors == current.successors
        && expected.ownership == current.ownership
}

fn same_position_normalized_node(expected: &OptimizationNode, current: &OptimizationNode) -> bool {
    // Relocation rebases definition/use coordinates and the function-wide
    // effect sequence. Core unit validation immediately reconstructs those
    // derived fields; this freeze retains every source-owned field instead.
    expected.operation == current.operation
        && expected.provenance == current.provenance
        && expected.fuel == current.fuel
        && position_normalized_definitions(expected) == position_normalized_definitions(current)
        && expected
            .uses
            .iter()
            .map(|value_use| value_use.value)
            .eq(current.uses.iter().map(|value_use| value_use.value))
        && expected.successors == current.successors
        && expected.ownership == current.ownership
}

fn position_normalized_definitions(node: &OptimizationNode) -> Vec<(ValueId, ScalarType)> {
    node.definitions
        .iter()
        .map(|definition| (definition.value, definition.scalar_type))
        .collect()
}

fn occurrences(
    function: &PsiOptimizationFunction,
    operation: OperationId,
) -> Vec<(BlockId, usize, &OptimizationNode)> {
    function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter(move |(_, node)| {
                    node.provenance.first() == Some(&PsiProvenance::Operation(operation))
                })
                .map(move |(node, value)| (block.id, node, value))
        })
        .collect()
}

fn block(function: &PsiOptimizationFunction, id: BlockId) -> Option<&OptimizationBlock> {
    function.blocks.iter().find(|block| block.id == id)
}

fn mismatch(machine: MachineId, block: BlockId) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch { machine, block }
}

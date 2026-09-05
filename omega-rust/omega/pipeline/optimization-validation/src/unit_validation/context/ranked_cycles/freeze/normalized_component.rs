//! Optimizer module role: validation leaf. Exact ranked-block relocation normalization.

use super::*;

pub(super) fn validate(
    expected: &PsiOptimizationFunction,
    current: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
) -> Result<(), OptimizationUnitValidationError> {
    let roles = [
        (certificate.header, certificate.guard.zero_operation),
        (
            certificate.descent.backedge.source,
            certificate.descent.one_operation,
        ),
    ];
    let [entry] = component.entries.as_slice() else {
        return Err(mismatch(component, certificate.header));
    };
    let preheader = block(current, entry.source)
        .filter(|_| !component.members.contains(&entry.source))
        .ok_or_else(|| mismatch(component, certificate.header))?;
    let jump_index = preheader
        .nodes
        .len()
        .checked_sub(1)
        .ok_or_else(|| mismatch(component, certificate.header))?;
    let O::Jump {
        psi_edge, target, ..
    } = preheader.nodes[jump_index].operation
    else {
        return Err(mismatch(component, certificate.header));
    };
    if psi_edge != entry.edge || target != certificate.header || entry.target != target {
        return Err(mismatch(component, certificate.header));
    }

    let mut moved = Vec::new();
    for (original_block, operation) in roles {
        let expected_occurrence = unique_operation(expected, operation)
            .filter(|(block, _, _)| *block == original_block)
            .ok_or_else(|| mismatch(component, original_block))?;
        let current_occurrence = unique_operation(current, operation)
            .ok_or_else(|| mismatch(component, original_block))?;
        if current_occurrence.0 == original_block {
            if current_occurrence.1 != expected_occurrence.1 {
                return Err(mismatch(component, original_block));
            }
        } else if current_occurrence.0 == preheader.id {
            moved.push((current_occurrence.1, original_block));
        } else {
            return Err(mismatch(component, original_block));
        }
        if !same_preserved_node(expected_occurrence.2, current_occurrence.2) {
            return Err(mismatch(component, original_block));
        }
    }

    if moved.is_empty() {
        return validate_exact_blocks(expected, current, component);
    }
    let suffix_start = jump_index
        .checked_sub(moved.len())
        .ok_or_else(|| mismatch(component, moved[0].1))?;
    if moved
        .iter()
        .enumerate()
        .any(|(offset, (node, _))| *node != suffix_start + offset)
    {
        return Err(mismatch(component, moved[0].1));
    }
    validate_normalized_blocks(
        expected,
        current,
        component,
        &roles.map(|(_, operation)| operation),
    )
}

fn validate_exact_blocks(
    expected: &PsiOptimizationFunction,
    current: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> Result<(), OptimizationUnitValidationError> {
    for id in &component.members {
        if block(expected, *id).is_none() || block(expected, *id) != block(current, *id) {
            return Err(mismatch(component, *id));
        }
    }
    Ok(())
}

fn validate_normalized_blocks(
    expected: &PsiOptimizationFunction,
    current: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    relocated_operations: &[OperationId; 2],
) -> Result<(), OptimizationUnitValidationError> {
    for id in &component.members {
        let expected = block(expected, *id).ok_or_else(|| mismatch(component, *id))?;
        let current = block(current, *id).ok_or_else(|| mismatch(component, *id))?;
        if expected.parameters != current.parameters {
            return Err(mismatch(component, *id));
        }
        let expected_nodes = retained_nodes(expected, relocated_operations);
        let current_nodes = retained_nodes(current, relocated_operations);
        if expected_nodes.len() != current_nodes.len()
            || expected_nodes
                .iter()
                .zip(current_nodes)
                .any(|(expected, current)| !same_position_normalized_node(expected, current))
        {
            return Err(mismatch(component, *id));
        }
    }
    Ok(())
}

fn retained_nodes<'block>(
    block: &'block optimization_unit::OptimizationBlock,
    relocated_operations: &[OperationId; 2],
) -> Vec<&'block optimization_unit::OptimizationNode> {
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

fn same_preserved_node(
    expected: &optimization_unit::OptimizationNode,
    current: &optimization_unit::OptimizationNode,
) -> bool {
    expected.operation == current.operation
        && expected.provenance == current.provenance
        && expected.fuel == current.fuel
        && position_normalized_definitions(expected) == position_normalized_definitions(current)
        && expected.uses.is_empty()
        && current.uses.is_empty()
        && expected.successors == current.successors
        && expected.ownership == current.ownership
}

fn same_position_normalized_node(
    expected: &optimization_unit::OptimizationNode,
    current: &optimization_unit::OptimizationNode,
) -> bool {
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

fn position_normalized_definitions(
    node: &optimization_unit::OptimizationNode,
) -> Vec<(ValueId, ScalarType)> {
    node.definitions
        .iter()
        .map(|definition| (definition.value, definition.scalar_type))
        .collect()
}

fn unique_operation(
    function: &PsiOptimizationFunction,
    operation: OperationId,
) -> Option<(BlockId, usize, &optimization_unit::OptimizationNode)> {
    let mut occurrences = function.blocks.iter().flat_map(|block| {
        block
            .nodes
            .iter()
            .enumerate()
            .filter(move |(_, node)| {
                node.provenance.first() == Some(&PsiProvenance::Operation(operation))
            })
            .map(move |(node, value)| (block.id, node, value))
    });
    let occurrence = occurrences.next()?;
    occurrences.next().is_none().then_some(occurrence)
}

fn block(
    function: &PsiOptimizationFunction,
    id: BlockId,
) -> Option<&optimization_unit::OptimizationBlock> {
    function.blocks.iter().find(|block| block.id == id)
}

fn mismatch(
    component: &OptimizerCycleComponent,
    block: BlockId,
) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
        machine: component.id.machine,
        block,
    }
}

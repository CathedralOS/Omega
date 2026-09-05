//! Optimizer module role: reconstruction leaf. Location-independent countdown constants.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct ResolvedInvariantConstant {
    pub(super) result: ValueId,
    pub(super) operation: OperationId,
    pub(super) location: NodeLocation,
}

pub(super) fn resolve(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    original_block: BlockId,
    result: ValueId,
    rank_type: IntegerType,
    expected_value: IntegerValue,
) -> Result<ResolvedInvariantConstant, ()> {
    let mut definitions = function.blocks.iter().flat_map(|block| {
        block
            .nodes
            .iter()
            .enumerate()
            .filter(move |(_, node)| {
                node.definitions
                    .iter()
                    .any(|definition| definition.value == result)
            })
            .map(move |(node, value)| (block.id, node, value))
    });
    let (block, node_index, node) = definitions.next().ok_or(())?;
    if definitions.next().is_some() {
        return Err(());
    }
    let node_index = u32::try_from(node_index).map_err(|_| ())?;
    let preheader = sole_preheader(function, component).map(|(block, _)| block.id);
    if block != original_block && Some(block) != preheader {
        return Err(());
    }
    let O::IntegerConstant {
        psi_operation,
        result: actual_result,
        scalar_type,
        value,
    } = node.operation
    else {
        return Err(());
    };
    let [definition] = node.definitions.as_slice() else {
        return Err(());
    };
    if actual_result != result
        || scalar_type != ScalarType::Integer(rank_type)
        || value != expected_value
        || node.provenance.first() != Some(&PsiProvenance::Operation(psi_operation))
        || definition.value != result
        || definition.scalar_type != ScalarType::Integer(rank_type)
        || definition.site
            != (ValueDefinitionSite::Node {
                block,
                node: node_index,
            })
        || !node.uses.is_empty()
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
    {
        return Err(());
    }
    Ok(ResolvedInvariantConstant {
        result,
        operation: psi_operation,
        location: NodeLocation {
            machine: function.machine,
            block,
            node: node_index,
        },
    })
}

pub(super) fn validate_canonical_preheader_suffix(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    header: BlockId,
    original_blocks_in_role_order: [BlockId; 2],
    locations_in_role_order: [NodeLocation; 2],
) -> Result<(), ()> {
    if locations_in_role_order
        .iter()
        .zip(original_blocks_in_role_order)
        .all(|(location, original)| location.block == original)
    {
        return Ok(());
    }
    let (preheader, jump_index) = sole_preheader(function, component).ok_or(())?;
    let moved = locations_in_role_order
        .into_iter()
        .zip(original_blocks_in_role_order)
        .filter_map(|(location, original)| (location.block != original).then_some(location))
        .collect::<Vec<_>>();
    if moved.iter().any(|location| location.block != preheader.id) {
        return Err(());
    }
    let O::Jump {
        psi_edge, target, ..
    } = preheader.nodes[jump_index].operation
    else {
        return Err(());
    };
    let [entry] = component.entries.as_slice() else {
        return Err(());
    };
    if psi_edge != entry.edge || target != header || entry.target != header {
        return Err(());
    }
    let suffix_start = jump_index.checked_sub(moved.len()).ok_or(())?;
    if moved.iter().enumerate().all(|(offset, location)| {
        location.machine == function.machine
            && usize::try_from(location.node).ok() == Some(suffix_start + offset)
    }) {
        Ok(())
    } else {
        Err(())
    }
}

fn sole_preheader<'function>(
    function: &'function PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> Option<(&'function optimization_unit::OptimizationBlock, usize)> {
    let [entry] = component.entries.as_slice() else {
        return None;
    };
    if component.members.contains(&entry.source) {
        return None;
    }
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == entry.source)?;
    preheader
        .nodes
        .iter()
        .enumerate()
        .next_back()
        .map(|(jump, _)| (preheader, jump))
}

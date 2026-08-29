//! Current-operation applicability from scope and value availability.

use super::*;

pub(super) fn validate_current_value_range_fact_at(
    unit: &PsiOptimizationUnit,
    fact: &ValueRangeFact,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> Result<(), OptimizationUnitValidationError> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable {
                machine,
                block,
                node,
            },
        )?;
    let query_block = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable {
                machine,
                block,
                node,
            },
        )?;
    if usize::try_from(node)
        .ok()
        .is_none_or(|node| node >= query_block.nodes.len())
        || fact.valid_in.machine != machine
        || !value_available_at(function, fact.value, block, node)
        || !scope_applies_at(
            fact.valid_in.scope,
            &fact.valid_in.dominated_blocks,
            block,
            node,
        )
    {
        return Err(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable {
                machine,
                block,
                node,
            },
        );
    }
    Ok(())
}

pub(super) fn scope_applies_at(
    scope: ValueRangeScope,
    dominated_blocks: &[BlockId],
    block: BlockId,
    node: u32,
) -> bool {
    match scope {
        ValueRangeScope::EntireValue => true,
        ValueRangeScope::DominatedOperationEntry {
            block: anchor,
            node: anchor_node,
            ..
        } => {
            if block == anchor {
                node >= anchor_node
            } else {
                dominated_blocks.binary_search(&block).is_ok()
            }
        }
    }
}

pub(super) fn value_available_at(
    function: &PsiOptimizationFunction,
    value: ValueId,
    block: BlockId,
    node: u32,
) -> bool {
    let Some(definition) = scalar_value_definition(function, value) else {
        return false;
    };
    match definition.site {
        ValueDefinitionSite::FunctionParameter(_) => true,
        ValueDefinitionSite::BlockParameter {
            block: definition_block,
            ..
        } => {
            definition_block == block
                || independent_reachable_dominators(function)
                    .get(&block)
                    .is_some_and(|dominators| dominators.contains(&definition_block))
        }
        ValueDefinitionSite::Node {
            block: definition_block,
            node: definition_node,
        } => {
            if definition_block == block {
                definition_node < node
            } else {
                independent_reachable_dominators(function)
                    .get(&block)
                    .is_some_and(|dominators| dominators.contains(&definition_block))
            }
        }
    }
}

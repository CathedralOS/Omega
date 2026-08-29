//! Representation-only validation for retained ranked control components.
//!
//! This closes source-handle-free identity, graph custody, guard/decrement
//! reconstruction, and structural-frontier preservation without granting
//! execution authority. Interpreter, fuel, and native support remain separate
//! milestones.

use super::*;
use psi_core::{IntegerCarrier, IntegerSign};
use psi_terminal::{
    OperationKind, OperationResult, TerminalRankedGuard, TerminalRankedSuccessorArgument,
};

pub(super) fn validate_ranked_scc(
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<BTreeSet<EdgeId>, ModuleError> {
    let Some(component) = &machine.ranked_scc else {
        return Ok(BTreeSet::new());
    };
    let invalid = || ModuleError::InvalidRankedScc(machine.id);
    let entry = blocks.get(&machine.entry).copied().ok_or_else(invalid)?;
    let header = blocks.get(&component.header).copied().ok_or_else(invalid)?;
    if machine.entry == component.header
        || !entry.parameters.is_empty()
        || component.rank_type.carrier() != IntegerCarrier::Fixed
        || component.rank_type.sign() != IntegerSign::Unsigned
        || component.lower_bound != component.rank_type.minimum_value()
        || component.upper_bound != component.rank_type.maximum_value()
        || value_types.get(&component.rank_parameter)
            != Some(&ScalarType::Integer(component.rank_type))
        || !header.parameters.iter().any(|parameter| {
            parameter.id == component.rank_parameter
                && parameter.scalar_type == ScalarType::Integer(component.rank_type)
        })
    {
        return Err(invalid());
    }

    let Terminator::Jump {
        target: preheader_target,
        arguments: preheader_arguments,
        ..
    } = &entry.terminator
    else {
        return Err(invalid());
    };
    let rank_index = header
        .parameters
        .iter()
        .position(|parameter| parameter.id == component.rank_parameter)
        .ok_or_else(invalid)?;
    if *preheader_target != component.header
        || preheader_arguments.len() != header.parameters.len()
        || preheader_arguments
            .get(rank_index)
            .and_then(|argument| value_types.get(argument))
            != Some(&ScalarType::Integer(component.rank_type))
    {
        return Err(invalid());
    }

    let rows = &component.covered_cyclic_edges;
    if rows.len() != 1
        || rows.windows(2).any(|pair| pair[0].edge >= pair[1].edge)
        || rows[0].source == component.header
        || rows[0].target != component.header
    {
        return Err(invalid());
    }
    let row = &rows[0];
    let source = blocks.get(&row.source).copied().ok_or_else(invalid)?;
    let Terminator::Jump {
        edge,
        target,
        arguments,
        trivial_affine_discards,
    } = &source.terminator
    else {
        return Err(invalid());
    };
    if *edge != row.edge || *target != row.target || !trivial_affine_discards.is_empty() {
        return Err(invalid());
    }
    match row.guard {
        TerminalRankedGuard::UnsignedParameterPositive {
            block: guard_block,
            edge: guard_edge,
            condition: retained_condition,
            parameter,
        } => {
            let guard_block = blocks.get(&guard_block).copied().ok_or_else(invalid)?;
            let Terminator::Conditional {
                condition,
                when_true,
                ..
            } = &guard_block.terminator
            else {
                return Err(invalid());
            };
            if guard_edge != when_true.edge
                || when_true.target != row.source
                || retained_condition != *condition
                || parameter != component.rank_parameter
                || value_types.get(condition) != Some(&ScalarType::Boolean)
                || incoming_edge_count(blocks, row.source) != 1
                || !positive_guard_operation(
                    guard_block,
                    retained_condition,
                    component.rank_parameter,
                    component.rank_type,
                )
            {
                return Err(invalid());
            }
        }
    }
    match row.successor_argument {
        TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
            argument_index,
            argument,
            source_parameter,
            target_parameter,
        } => {
            let argument_index = usize::try_from(argument_index).map_err(|_| invalid())?;
            if source_parameter != component.rank_parameter
                || target_parameter != component.rank_parameter
                || header
                    .parameters
                    .get(argument_index)
                    .map(|parameter| parameter.id)
                    != Some(target_parameter)
                || arguments.get(argument_index).copied() != Some(argument)
                || value_types.get(&argument) != Some(&ScalarType::Integer(component.rank_type))
                || !decrement_operation(source, argument, source_parameter, component.rank_type)
            {
                return Err(invalid());
            }
        }
    }
    Ok(BTreeSet::from([row.edge]))
}

fn incoming_edge_count(blocks: &BTreeMap<BlockId, &psi_terminal::Block>, target: BlockId) -> usize {
    blocks
        .values()
        .flat_map(|block| match &block.terminator {
            Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (when_true.edge, when_true.target),
                (when_false.edge, when_false.target),
            ],
            _ => Vec::new(),
        })
        .filter(|(_, candidate)| *candidate == target)
        .count()
}

fn positive_guard_operation(
    block: &psi_terminal::Block,
    condition: ValueId,
    rank: ValueId,
    rank_type: psi_core::IntegerType,
) -> bool {
    let Some((condition_index, condition_operation)) =
        block.operations.iter().enumerate().find(|(_, operation)| {
            operation.result.scalar().map(|result| result.id) == Some(condition)
        })
    else {
        return false;
    };
    let OperationKind::IntegerLessThan { left, right } = condition_operation.kind else {
        return false;
    };
    if right != rank {
        return false;
    }
    block.operations[..condition_index].iter().any(|operation| {
        operation.result
            == OperationResult::Scalar(psi_terminal::ValueDeclaration {
                id: left,
                scalar_type: ScalarType::Integer(rank_type),
            })
            && operation.kind
                == OperationKind::IntegerConstant {
                    value: psi_core::IntegerValue::Unsigned(0),
                }
    })
}

fn decrement_operation(
    block: &psi_terminal::Block,
    result: ValueId,
    rank: ValueId,
    rank_type: psi_core::IntegerType,
) -> bool {
    let Some((subtract_index, subtract)) = block
        .operations
        .iter()
        .enumerate()
        .find(|(_, operation)| operation.result.scalar().map(|value| value.id) == Some(result))
    else {
        return false;
    };
    let OperationKind::ExactIntegerSubtract { left, right, .. } = subtract.kind else {
        return false;
    };
    if left != rank {
        return false;
    }
    block.operations[..subtract_index].iter().any(|operation| {
        operation.result
            == OperationResult::Scalar(psi_terminal::ValueDeclaration {
                id: right,
                scalar_type: ScalarType::Integer(rank_type),
            })
            && operation.kind
                == OperationKind::IntegerConstant {
                    value: psi_core::IntegerValue::Unsigned(1),
                }
    })
}

//! Producer derivation of exact literal-fold actions.

use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{SelectedFunction, SelectedInstructionKind};
use psi_core::IntegerValue;

use crate::{LiteralFoldAction, LiteralFoldError, RecoveryClassification, RecoveryVictimRole};

use super::constraints::ImmediateRows;

pub(super) fn derive_action(
    function_index: usize,
    function: &SelectedFunction,
    candidate: &crate::PressureRecoveryClassification,
    rows: &ImmediateRows<'_>,
) -> Result<LiteralFoldAction, LiteralFoldError> {
    if candidate.role != RecoveryVictimRole::Incoming {
        return Err(LiteralFoldError::UnsupportedVictimRole {
            function: function_index,
        });
    }
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        value: IntegerValue::Unsigned(value),
        provenance,
        future_uses,
        ..
    } = &candidate.classification
    else {
        return Err(LiteralFoldError::ClassificationNotAdmitted {
            function: function_index,
        });
    };
    let immediate = u64::try_from(*value)
        .ok()
        .filter(|value| *value <= 4095)
        .ok_or(LiteralFoldError::UnsupportedImmediate {
            function: function_index,
        })?;
    let [future_use] = future_uses.as_slice() else {
        return Err(LiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    };
    if future_use.operand != 1 || future_use.block != candidate.block {
        return Err(LiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    }

    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(LiteralFoldError::LiteralMismatch {
            function: function_index,
        })?;
    let literal_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == *defining_instruction)
        .ok_or(LiteralFoldError::LiteralMismatch {
            function: function_index,
        })?;
    let literal = &block.instructions[literal_index];
    let consumer = block
        .instructions
        .get(literal_index + 1)
        .filter(|instruction| instruction.id == future_use.instruction)
        .ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;
    if literal.kind
        != (SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(*value),
        })
        || literal.provenance != *provenance
        || literal.operands.len() != 1
        || literal.operands[0].virtual_register != candidate.victim
        || literal.operands[0].access != RegisterOperandAccess::Def
    {
        return Err(LiteralFoldError::LiteralMismatch {
            function: function_index,
        });
    }

    let row = match consumer.kind {
        SelectedInstructionKind::ExactAddI64 { .. } => rows.add,
        SelectedInstructionKind::ExactSubtractI64 { .. } => rows.subtract,
        _ => None,
    }
    .ok_or(LiteralFoldError::ConsumerMismatch {
        function: function_index,
    })?;
    let [left, right, result] = consumer.operands.as_slice() else {
        return Err(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    };
    if left.access != RegisterOperandAccess::Use
        || right.access != RegisterOperandAccess::Use
        || right.virtual_register != candidate.victim
        || result.access != RegisterOperandAccess::Def
        || left.class != row.operands[0].class
        || result.class != row.operands[1].class
    {
        return Err(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    }

    Ok(LiteralFoldAction {
        block: candidate.block,
        pressure_point: candidate.point,
        literal_instruction: *defining_instruction,
        victim: candidate.victim,
        consumer_instruction: consumer.id,
        left: left.virtual_register,
        result: result.virtual_register,
        immediate,
        immediate_constraint: row.key,
    })
}

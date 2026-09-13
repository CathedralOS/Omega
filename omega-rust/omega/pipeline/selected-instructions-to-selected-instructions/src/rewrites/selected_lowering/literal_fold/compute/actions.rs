//! Producer derivation of exact literal-fold actions.

use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedFunction, SelectedInstructionKind};
use semantic_vocabulary::IntegerValue;

use crate::{LiteralFoldAction, LiteralFoldError, RecoveryClassification, RecoveryVictimRole};

use super::constraints::AdmittedPairs;

pub(super) fn derive_action(
    function_index: usize,
    function: &SelectedFunction,
    candidate: &crate::PressureRecoveryClassification,
    rows: &AdmittedPairs<'_>,
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
    let immediate = u64::try_from(*value).map_err(|_| LiteralFoldError::UnsupportedImmediate {
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
    let pair = rows
        .for_consumer(consumer.kind)
        .ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;
    if !pair.rule.admits_immediate(immediate) {
        return Err(LiteralFoldError::UnsupportedImmediate {
            function: function_index,
        });
    }
    if literal.kind
        != (SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(*value),
        })
        || !pair.rule.matches_producer(literal.kind)
        || literal.provenance != *provenance
        || literal.operands.len() != 1
        || literal.operands[0].virtual_register != candidate.victim
        || literal.operands[0].access != RegisterOperandAccess::Def
    {
        return Err(LiteralFoldError::LiteralMismatch {
            function: function_index,
        });
    }

    let row = pair.row;
    let result = match consumer.operands.as_slice() {
        [left, right, result] => {
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
            Some(result.virtual_register)
        }
        // Flag-defining consumers carry `[left, right]` uses and no `Def`.
        [left, right] => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || row.operands.len() != 1
                || left.class != row.operands[0].class
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            None
        }
        _ => {
            return Err(LiteralFoldError::ConsumerMismatch {
                function: function_index,
            });
        }
    };
    let left = consumer.operands[0].virtual_register;

    Ok(LiteralFoldAction {
        block: candidate.block,
        pressure_point: candidate.point,
        literal_instruction: *defining_instruction,
        victim: candidate.victim,
        consumer_instruction: consumer.id,
        left,
        result,
        immediate,
        immediate_constraint: row.key,
    })
}

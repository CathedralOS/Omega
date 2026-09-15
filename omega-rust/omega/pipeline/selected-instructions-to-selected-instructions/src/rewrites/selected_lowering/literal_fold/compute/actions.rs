//! Producer derivation of exact literal-fold actions.

use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedFunction, SelectedInstructionKind};
use semantic_vocabulary::IntegerValue;

use crate::{
    LiteralFoldAction, LiteralFoldError, PairOperandShape, PairResultDisposition,
    RecoveryClassification, RecoveryVictimRole,
};

use super::constraints::{AdmittedPairs, effect_declaration};

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
    let literal_u64 =
        u64::try_from(*value).map_err(|_| LiteralFoldError::UnsupportedImmediate {
            function: function_index,
        })?;
    let [future_use] = future_uses.as_slice() else {
        return Err(LiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    };
    if future_use.block != candidate.block {
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
    // The consumer kind and the folded literal's operand position together
    // select the grammar: a family may admit the same kind under disjoint
    // operand shapes. An unadmitted kind is a consumer mismatch; an admitted
    // kind whose literal sits at a position no enabled grammar covers is a
    // future-use mismatch.
    let pair = rows
        .for_consumer(consumer.kind, future_use.operand)
        .ok_or_else(|| {
            if rows.admits_consumer_kind(consumer.kind) {
                LiteralFoldError::FutureUseMismatch {
                    function: function_index,
                }
            } else {
                LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                }
            }
        })?;
    if !pair.rule.admits_immediate(literal_u64) {
        return Err(LiteralFoldError::UnsupportedImmediate {
            function: function_index,
        });
    }
    let immediate =
        pair.rule
            .fold_immediate(literal_u64)
            .ok_or(LiteralFoldError::UnsupportedImmediate {
                function: function_index,
            })?;
    if literal.kind
        != (SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(*value),
        })
        || !pair.rule.matches_producer(literal.kind)
        || literal.provenance != *provenance
        || literal.operands.len() != 1
        || literal.operands[0].virtual_register != candidate.victim
        || literal.operands[0].access != RegisterOperandAccess::Def
        // The eliminated instruction's record must carry no unit traffic at
        // all: removing it would silently drop any implicit use, definition,
        // clobber, or operand binding it declared.
        || !pair.rule.unit_effects().admits_producer(literal)
    {
        return Err(LiteralFoldError::LiteralMismatch {
            function: function_index,
        });
    }

    let row = pair.row;
    let result = match (
        pair.rule.operand_shape(),
        pair.rule.result(),
        consumer.operands.as_slice(),
    ) {
        (
            PairOperandShape::BinaryRightLiteral,
            PairResultDisposition::ScalarRegister,
            [left, right, result],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        // Commutative binary consumers also admit the literal as the left
        // operand: `[victim, right, result]` folds the operand-0 `Use` and
        // binds the operand-1 survivor into the rewritten row.
        (
            PairOperandShape::BinaryLeftLiteral,
            PairResultDisposition::ScalarRegister,
            [victim, right, result],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != candidate.victim
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || right.class != row.operands[0].class
                || result.class != row.operands[1].class
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        // Flag-defining consumers carry `[left, right]` uses and no `Def`;
        // their result is the rewritten row's implicit unit definitions.
        (
            PairOperandShape::BinaryRightLiteral,
            PairResultDisposition::ImplicitUnits,
            [left, right],
        ) => {
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
        // Unary consumers carry `[input, result]`; the folded literal is the
        // sole `Use` operand and the rewritten row carries only its `Def`.
        (
            PairOperandShape::UnaryLiteral,
            PairResultDisposition::ScalarRegister,
            [input, result],
        ) => {
            if input.access != RegisterOperandAccess::Use
                || input.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        _ => {
            return Err(LiteralFoldError::ConsumerMismatch {
                function: function_index,
            });
        }
    };
    // The rewrite rebuilds the consumer's operands from the rewritten
    // constraint row; an operand carrying a unit binding would silently
    // lose it, so the declared unit-effect surface admits only undecorated
    // consumer operands.
    if !pair.rule.unit_effects().admits_consumer(consumer) {
        return Err(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    }
    // The pair's declared machine-effect surface must hold in the bound
    // catalog for both instructions the rewrite touches: the eliminated
    // literal must be fully effect-isolated so its removal drops nothing
    // machine-visible, and the consumer must be isolated outside the unit
    // surface the rewrite replaces wholesale. The rewritten declaration was
    // already admitted against the same surface when the pair was bound.
    let producer_declaration =
        effect_declaration(rows.catalog, pair.rule.producer(), literal.constraint).ok_or(
            LiteralFoldError::EffectSurfaceMismatch {
                function: function_index,
            },
        )?;
    let consumer_declaration =
        effect_declaration(rows.catalog, pair.rule.consumer(), consumer.constraint).ok_or(
            LiteralFoldError::EffectSurfaceMismatch {
                function: function_index,
            },
        )?;
    let effects = pair.rule.machine_effects();
    if !effects.admits_producer(producer_declaration)
        || !effects.admits_consumer(consumer_declaration, pair.declaration)
    {
        return Err(LiteralFoldError::EffectSurfaceMismatch {
            function: function_index,
        });
    }
    // The action records the register every `Use` position of the rewritten
    // row binds — the source operand that survives the fold. A right-literal
    // grammar leaves operand 0, a left-literal grammar leaves operand 1, and
    // the `Use`-free unary fold records its folded input.
    let surviving = match pair.rule.operand_shape() {
        PairOperandShape::BinaryLeftLiteral => consumer.operands[1].virtual_register,
        PairOperandShape::BinaryRightLiteral | PairOperandShape::UnaryLiteral => {
            consumer.operands[0].virtual_register
        }
    };

    Ok(LiteralFoldAction {
        block: candidate.block,
        pressure_point: candidate.point,
        literal_instruction: *defining_instruction,
        victim: candidate.victim,
        consumer_instruction: consumer.id,
        surviving,
        result,
        immediate,
        immediate_constraint: row.key,
    })
}

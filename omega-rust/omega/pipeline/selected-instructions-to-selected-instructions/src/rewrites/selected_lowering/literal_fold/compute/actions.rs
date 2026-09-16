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
    // The consumer kind, the folded literal's operand position, and the
    // literal's value together select the grammar: a family may admit the
    // same kind under disjoint operand shapes, and disjoint families may
    // admit the same kind at the same position under disjoint literal
    // bounds — the bitwise-and annihilator and identity selections both
    // fold `BitwiseAndI64` at either `Use` position. An unadmitted kind is
    // a consumer mismatch; an admitted kind whose literal sits at a
    // position no enabled grammar covers is a future-use mismatch; an
    // admitted position whose literal lies outside every enabled bound is
    // an unsupported immediate.
    let pair = rows
        .for_consumer(consumer.kind, future_use.operand, literal_u64)
        .ok_or_else(|| {
            if !rows.admits_consumer_kind(consumer.kind) {
                LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                }
            } else if rows
                .for_position(consumer.kind, future_use.operand)
                .is_none()
            {
                LiteralFoldError::FutureUseMismatch {
                    function: function_index,
                }
            } else {
                LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                }
            }
        })?;
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
        // A binary right-literal consumer whose operand list continues past
        // its `Def` result: `[left, victim, result, aux...]` folds the
        // operand-1 `Use` and drops every trailing `Use` operand, which the
        // declared grammar admits only when each dropped register is defined
        // in this function solely by zero materializations — the zeroed
        // high-half input an x86-64 `div` realization reads, left dead by a
        // divide by one. A dropped operand defined any other way would
        // silently discard a value the consumer observed.
        (
            PairOperandShape::BinaryRightLiteralAuxiliaryUses,
            PairResultDisposition::ScalarRegister,
            [left, right, result, auxiliary @ ..],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
                || !auxiliary.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && auxiliary_zero_defined(function, operand.virtual_register)
                })
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        // A binary right-literal consumer whose folded result is a constant
        // of the literal alone: `[left, victim, result, scratch...]` folds
        // the operand-1 `Use`, drops the operand-0 `Use` — the constant
        // result never reads it — and drops every `Def` operand past the
        // result, which the declared grammar admits only when each dropped
        // register occurs nowhere else in the function — the dead quotient
        // scratch an x86-64 `idiv` realization writes. A dropped `Def`
        // another instruction read or defined would silently leave a use
        // of a register the rewrite stopped defining.
        (
            PairOperandShape::BinaryRightLiteralConstantResult,
            PairResultDisposition::ScalarRegister,
            [left, right, result, scratch @ ..],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
                || !scratch.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Def
                        && dropped_def_is_dead(function, operand.virtual_register)
                })
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        // A binary left-literal consumer whose folded result is a constant
        // of the literal alone: `[victim, right, result, scratch...]`
        // folds the operand-0 `Use`, drops the operand-1 `Use` — the
        // constant result never reads it — and drops every `Def` operand
        // past the result under the same occurrence-free custody the
        // right grammar requires. Declaring this shape attests the
        // operand-0 literal alone fixes the result — `0 & x` is zero for
        // every `x` — so no commutation of the surviving `Use` is
        // implied.
        (
            PairOperandShape::BinaryLeftLiteralConstantResult,
            PairResultDisposition::ScalarRegister,
            [victim, right, result, scratch @ ..],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != candidate.victim
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
                || !scratch.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Def
                        && dropped_def_is_dead(function, operand.virtual_register)
                })
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
    // The obligation-discharged surface also needs the obligation's
    // instruction-level custody: under `FaultDischargedByObligation` the
    // folded literal does not itself make the encoded fault unreachable,
    // so the fold is admitted only when the obligation the consumer kind
    // names is retained in the instruction's recorded proof custody.
    if !pair
        .rule
        .machine_effects()
        .admits_consumer_obligation(consumer)
    {
        return Err(LiteralFoldError::EffectSurfaceMismatch {
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
    // grammar — including the auxiliary-`Use` divide grammar — leaves
    // operand 0, a left-literal grammar leaves operand 1, and the `Use`-free
    // unary fold records its folded input. The constant-result grammars
    // bind no `Use` position; they record the dropped non-victim `Use` for
    // custody — the operand-0 dividend under the right grammar, the
    // operand-1 `Use` under the left annihilator grammar.
    let surviving = match pair.rule.operand_shape() {
        PairOperandShape::BinaryLeftLiteral | PairOperandShape::BinaryLeftLiteralConstantResult => {
            consumer.operands[1].virtual_register
        }
        PairOperandShape::BinaryRightLiteral
        | PairOperandShape::BinaryRightLiteralAuxiliaryUses
        | PairOperandShape::BinaryRightLiteralConstantResult
        | PairOperandShape::UnaryLiteral => consumer.operands[0].virtual_register,
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

/// Whether `register` is defined in `function` only by `MaterializeI64`
/// instructions producing `Unsigned(0)` — the provenance the
/// auxiliary-`Use` operand grammar requires of every operand it drops. A
/// register with no definition, or any definition that is not a zero
/// materialization, fails: the dropped operand would carry a value the
/// folded form silently stopped observing.
fn auxiliary_zero_defined(
    function: &SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    let mut definitions = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access == RegisterOperandAccess::Def && operand.virtual_register == register
            })
        });
    let mut saw_definition = false;
    definitions.all(|instruction| {
        saw_definition = true;
        matches!(
            instruction.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0)
            }
        )
    }) && saw_definition
}

/// Whether `register`'s only occurrence in `function` is one `Def` operand —
/// the custody the constant-result grammar requires of every scratch `Def`
/// it drops. The operand itself is that one occurrence: any other operand
/// position, terminator operand, or successor transport naming the register
/// would leave the fold removing a definition that a surviving read or a
/// second definition still observes. Uses are counted across instruction
/// and terminator operand lists and every successor binding transport, the
/// same sites the rewrite's densification walks.
fn dropped_def_is_dead(
    function: &SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    let mut occurrences = 0_usize;
    for block in &function.blocks {
        for instruction in block.instructions.iter().chain(match &block.terminator {
            selected_instructions::SelectedTerminator::ConditionalBranch {
                instruction, ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
                instruction,
                ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
                instruction,
                ..
            }
            | selected_instructions::SelectedTerminator::Jump { instruction, .. }
            | selected_instructions::SelectedTerminator::Return { instruction, .. }
            | selected_instructions::SelectedTerminator::HostedExitProcess {
                instruction, ..
            } => std::iter::once(instruction),
        }) {
            occurrences += instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
                .count();
        }
        let successors = match &block.terminator {
            selected_instructions::SelectedTerminator::Jump { successor, .. } => {
                vec![successor]
            }
            selected_instructions::SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            selected_instructions::SelectedTerminator::Return { .. }
            | selected_instructions::SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
        };
        for successor in successors {
            occurrences += successor
                .structural_bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.transport,
                        selected_instructions::SelectedStructuralTransport::WholeValue {
                            argument,
                            ..
                        }
                        | selected_instructions::SelectedStructuralTransport::Descriptor {
                            argument,
                            ..
                        } if argument == register
                    )
                })
                .count();
            if let Some(case) = &successor.structural_case {
                occurrences += case
                    .payloads
                    .iter()
                    .filter(|payload| match &payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter,
                        } => *parameter == register,
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => *argument == register || *parameter == register,
                    })
                    .count();
            }
            occurrences += successor
                .bindings
                .iter()
                .filter(|binding| match &binding.transport {
                    selected_instructions::SelectedValueTransport::Unused => false,
                    selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == register || *parameter == register,
                })
                .count();
        }
    }
    occurrences == 1
}

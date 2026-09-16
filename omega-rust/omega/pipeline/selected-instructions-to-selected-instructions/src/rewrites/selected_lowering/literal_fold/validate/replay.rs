use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineSemanticKind, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionProvenance,
    SelectedOperand, SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

use crate::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, RecoveryClassification,
    RecoveryVictimRole, ValidatedRecoveryClassifications, ValidatedSelectedAnalysis,
    machine_semantic_kind,
};

use super::constraints::{
    ValidationImmediateRows, effect_declaration, fault_discharged_fold_admission,
    indexed_read_fold_admission, isolated_effect_alternative, isolated_effect_declaration,
    isolated_rewritten_declaration,
};

pub(super) fn reconstruct_literal_fold(
    selected: &impl ValidatedSelectedAnalysis,
    recovery: &ValidatedRecoveryClassifications,
    rows: &ValidationImmediateRows<'_>,
) -> Result<(Vec<FunctionLiteralFold>, SelectedInstructionPlan), LiteralFoldError> {
    let source_plan = selected.selected_plan();
    let mut transformed = source_plan.clone();
    let mut expected_functions = Vec::with_capacity(source_plan.functions.len());

    for (function_index, source) in source_plan.functions.iter().enumerate() {
        let recovery_function = recovery.plan().functions.get(function_index).ok_or(
            LiteralFoldError::FunctionMismatch {
                function: function_index,
            },
        )?;
        if source.machine != recovery_function.machine {
            return Err(LiteralFoldError::FunctionMismatch {
                function: function_index,
            });
        }
        validate_dense_identifiers(function_index, source)?;

        let action = recovery_function
            .classification
            .as_ref()
            .map(|classification| reconstruct_action(function_index, source, classification, rows))
            .transpose()?;
        if let Some(action) = action {
            rebuild_function(
                function_index,
                &mut transformed.functions[function_index],
                action,
                rows,
            )?;
        }
        expected_functions.push(FunctionLiteralFold {
            machine: source.machine,
            action,
        });
    }

    Ok((expected_functions, transformed))
}

fn reconstruct_action(
    function_index: usize,
    function: &SelectedFunction,
    candidate: &crate::PressureRecoveryClassification,
    rows: &ValidationImmediateRows<'_>,
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

    if literal.kind
        != (SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(*value),
        })
        || literal.provenance != *provenance
        || literal.operands.len() != 1
        || literal.operands[0].virtual_register != candidate.victim
        || literal.operands[0].access != RegisterOperandAccess::Def
        // The eliminated instruction's record must carry no unit traffic at
        // all: removing it would silently drop any implicit use, definition,
        // clobber, or operand binding it declared.
        || literal.operands[0].fixed_view.is_some()
        || literal.operands[0].tied_to.is_some()
        || literal.operands[0].early_clobber
        || !literal.implicit_uses.is_empty()
        || !literal.implicit_defs.is_empty()
        || !literal.clobbers.is_empty()
    {
        return Err(LiteralFoldError::LiteralMismatch {
            function: function_index,
        });
    }

    // The validator re-derives the source grammar from the consumer kind and
    // the folded literal's operand position alone — never from the producer's
    // descriptor. Binary consumers fold the literal into an immediate form
    // whose row the kind owns; exact addition is commutative, so an operand-0
    // literal derives the left-fold grammar while operand 1 derives the right
    // fold. Unary extension consumers fold into a direct `MaterializeI64` of
    // the extension's exact output bits, and a unary copy consumer folds into
    // a direct `MaterializeI64` of the literal itself.
    let (shape, row, rewritten) = match consumer.kind {
        SelectedInstructionKind::ExactAddI64 { .. } => (
            if future_use.operand == 0 {
                SourceShape::BinaryLeftImmediate
            } else {
                SourceShape::BinaryImmediate
            },
            rows.add,
            MachineSemanticKind::ExactAddI64Immediate,
        ),
        SelectedInstructionKind::ExactSubtractI64 { .. } => (
            SourceShape::BinaryImmediate,
            rows.subtract,
            MachineSemanticKind::ExactSubtractI64Immediate,
        ),
        SelectedInstructionKind::CompareI64 => (
            SourceShape::BinaryImmediate,
            rows.compare,
            MachineSemanticKind::CompareI64Immediate,
        ),
        SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::ZeroExtendU32
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32 => (
            SourceShape::UnaryExtension,
            rows.materialize,
            MachineSemanticKind::MaterializeI64,
        ),
        // The copy materializes its literal input directly: the rewritten
        // form is `MaterializeI64`, bound through the copy policy's own row
        // gate so an unselected copy fold cannot replay under the extension
        // policy's binding.
        SelectedInstructionKind::CopyI64 => (
            SourceShape::UnaryCopy,
            rows.copy,
            MachineSemanticKind::MaterializeI64,
        ),
        // The indexed byte load folds its operand-1 index literal into the
        // direct-offset `Load8` form, bound to the `Load8` row.
        SelectedInstructionKind::Load8Indexed => (
            SourceShape::BinaryImmediate,
            rows.load8,
            MachineSemanticKind::Load8,
        ),
        // The byte-view address projection folds its operand-1 offset
        // literal into the constant-offset `AddressOffset` form, bound to
        // the `AddressOffset` row: the surviving operand-0 `Use` is the
        // base and the operand-2 `Def` is the result.
        SelectedInstructionKind::ByteViewAddress => (
            SourceShape::BinaryImmediate,
            rows.address_offset,
            MachineSemanticKind::AddressOffset,
        ),
        // The unsigned divide folds an operand-1 divisor literal of exactly
        // one into a `CopyI64` of the operand-0 dividend, bound to the
        // `CopyI64` row. Operands past the operand-2 `Def` result are `Use`
        // positions the fold drops — the zeroed high-half input an x86-64
        // `div` realization reads — each independently required to be
        // defined only by zero materializations.
        SelectedInstructionKind::ExactDivideU64 { .. } => (
            SourceShape::DivideIdentity,
            rows.divide,
            MachineSemanticKind::CopyI64,
        ),
        // The wrapping remainder folds an operand-1 divisor literal of
        // exactly one into a `MaterializeI64` of zero — a remainder by one
        // is always zero — bound to the `MaterializeI64` row the
        // remainder policy's own gate selected. Operands past the
        // operand-2 `Def` result are `Def` scratch outputs the fold drops,
        // each independently required to occur nowhere else in the
        // function, and the operand-0 `Use` is dropped because the
        // constant result never reads it.
        SelectedInstructionKind::WrappingRemainderI64 { .. } => (
            SourceShape::RemainderIdentity,
            rows.remainder,
            MachineSemanticKind::MaterializeI64,
        ),
        // The bitwise-and annihilator fold: a literal of exactly zero at
        // either `Use` folds `BitwiseAndI64` into a `MaterializeI64` of
        // zero — `0 & x` and `x & 0` are both zero — bound to the
        // `MaterializeI64` row the and-zero policy's own gate selected.
        // The other `Use` drops because the constant result never reads
        // it, and every `Def` operand past the operand-2 result drops
        // under the same occurrence-free custody the remainder grammar
        // derives. The recorded operand position picks the grammar.
        SelectedInstructionKind::BitwiseAndI64 => (
            if future_use.operand == 0 {
                SourceShape::AndZeroLeft
            } else {
                SourceShape::AndZero
            },
            rows.and_zero,
            MachineSemanticKind::MaterializeI64,
        ),
        _ => (
            SourceShape::BinaryImmediate,
            None,
            MachineSemanticKind::MaterializeI64,
        ),
    };
    let row = row.ok_or(LiteralFoldError::ConsumerMismatch {
        function: function_index,
    })?;
    if future_use.operand != shape.victim_operand() {
        return Err(LiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    }
    let immediate = match shape {
        SourceShape::BinaryImmediate | SourceShape::BinaryLeftImmediate => {
            if literal_u64 > 4095 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
        }
        SourceShape::UnaryExtension => extension_bits(consumer.kind, literal_u64).ok_or(
            LiteralFoldError::ConsumerMismatch {
                function: function_index,
            },
        )?,
        // The copy's constant output is the literal itself; the
        // `MaterializeI64` row it rewrites into bounds the payload only by
        // what the result register's scalar type admits, checked at rebuild.
        SourceShape::UnaryCopy => literal_u64,
        // The divide fold is the identity only when the divisor literal is
        // exactly one; any other divisor is a different computation the
        // replay must not admit.
        SourceShape::DivideIdentity => {
            if literal_u64 != 1 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
        }
        // The remainder fold is the constant zero only when the divisor
        // literal is exactly one; any other divisor is a different
        // computation the replay must not admit. The recorded immediate is
        // the constant the rewritten `MaterializeI64` embeds — zero.
        SourceShape::RemainderIdentity => {
            if literal_u64 != 1 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            0
        }
        // The and-zero fold is the constant zero only when the folded
        // literal is exactly zero — zero is the bitwise-and annihilator at
        // either `Use` position; any other literal is a different
        // computation the replay must not admit. The recorded immediate
        // is the constant the rewritten `MaterializeI64` embeds — zero.
        SourceShape::AndZero | SourceShape::AndZeroLeft => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            0
        }
    };
    let result = match (shape, consumer.operands.as_slice()) {
        (SourceShape::BinaryImmediate, [left, right, result]) => {
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
        // The commutative left fold: operand 0 is the folded victim and the
        // operand-1 `Use` is the register the rewritten row binds.
        (SourceShape::BinaryLeftImmediate, [victim, right, result]) => {
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
        (SourceShape::BinaryImmediate, [left, right]) => {
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
        // The divide-identity grammar: `[dividend, divisor, result, aux...]`
        // folds the operand-1 `Use` and drops every `Use` operand past the
        // operand-2 `Def` result. The validator independently re-derives
        // the dropped-operand custody: each dropped register must be
        // defined in this function only by `MaterializeI64` instructions
        // producing `Unsigned(0)` — the zeroed high-half input an x86-64
        // `div` realization reads — because the fold discards whatever the
        // operand carried.
        (SourceShape::DivideIdentity, [left, right, result, auxiliary @ ..]) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
                || !auxiliary.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && dropped_use_defined_zero(function, operand.virtual_register)
                })
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        // The constant-result grammar: `[dividend, divisor, result,
        // scratch...]` folds the operand-1 `Use`, drops the operand-0
        // `Use` — the constant result never reads it — and drops every
        // `Def` operand past the operand-2 `Def` result. The validator
        // independently re-derives the dropped-operand custody: each
        // dropped `Def` register must occur nowhere else in the function —
        // the dead quotient scratch an x86-64 `idiv` realization writes —
        // because the fold discards a definition a surviving read or
        // second definition would still observe.
        (SourceShape::RemainderIdentity, [left, right, result, scratch @ ..]) => {
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
        // The and-zero grammar: `[left, victim, result, scratch...]`
        // folds the operand-1 `Use`, drops the operand-0 `Use` — the
        // constant result never reads it — and drops every `Def` operand
        // past the operand-2 `Def` result. The validator independently
        // re-derives the dropped-operand custody: each dropped `Def`
        // register must occur nowhere else in the function, because the
        // fold discards a definition a surviving read or second
        // definition would still observe.
        (SourceShape::AndZero, [left, right, result, scratch @ ..]) => {
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
        // The left annihilator grammar: `[victim, right, result,
        // scratch...]` folds the operand-0 `Use`, drops the operand-1
        // `Use`, and drops every `Def` operand past the result under the
        // same occurrence-free custody. No commutation is attested — the
        // operand-0 literal alone fixes the result, so the fold never
        // repositions the dropped operand.
        (SourceShape::AndZeroLeft, [victim, right, result, scratch @ ..]) => {
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
        (SourceShape::UnaryExtension | SourceShape::UnaryCopy, [input, result]) => {
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
    // The rebuild replaces the consumer's operands wholesale; an operand
    // carrying a unit binding would silently lose it. The validator
    // re-derives this requirement itself rather than reading the producer's
    // declared unit-effect surface. Under the divide-identity grammar a
    // `fixed_view` pin is deliberately dropped with the pinned operand
    // form — a surviving operand keeps its other uses' own constraints —
    // while `tied_to` has no carried meaning once the operand list is
    // rebuilt and still rejects under every grammar. Under the
    // constant-result grammar an `early_clobber` mark drops with its
    // operand for the same reason: the write-before-read hazard it names
    // exists only inside the folded operand list.
    let drops_fixed_views = matches!(
        shape,
        SourceShape::DivideIdentity | SourceShape::RemainderIdentity
    );
    let drops_early_clobbers = shape == SourceShape::RemainderIdentity;
    if consumer.operands.iter().any(|operand| {
        (operand.fixed_view.is_some() && !drops_fixed_views)
            || operand.tied_to.is_some()
            || (operand.early_clobber && !drops_early_clobbers)
    }) {
        return Err(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    }

    // The validator re-derives the effect-surface admission from the bound
    // catalog itself: the eliminated literal's declaration must be isolated
    // including every implicit unit it could have written, the consumer's
    // declaration must be isolated outside a unit surface the rewrite may
    // replace — no implicit uses, and every implicit definition must stay
    // defined by every alternative the rewritten form could select — and the
    // rewritten declaration must be isolated with no implicit uses or
    // clobbers beyond its declared result channel.
    let producer_declaration = effect_declaration(
        rows.catalog,
        MachineSemanticKind::MaterializeI64,
        literal.constraint,
    )
    .ok_or(LiteralFoldError::EffectSurfaceMismatch {
        function: function_index,
    })?;
    let consumer_declaration = effect_declaration(
        rows.catalog,
        machine_semantic_kind(consumer.kind),
        consumer.constraint,
    )
    .ok_or(LiteralFoldError::EffectSurfaceMismatch {
        function: function_index,
    })?;
    let rewritten_declaration = effect_declaration(rows.catalog, rewritten, row.key).ok_or(
        LiteralFoldError::EffectSurfaceMismatch {
            function: function_index,
        },
    )?;
    // The consumer/rewritten relationship differs per fold family. The
    // isolated folds replace an isolated surface wholesale; the indexed
    // byte-load fold is memory-carrying, so its admission binds the two
    // declarations' pointer-read surface pairwise — the read survives the
    // rewrite — instead of requiring isolation.
    let fold_surface_admitted = match consumer.kind {
        SelectedInstructionKind::Load8Indexed => indexed_read_fold_admission(
            consumer_declaration,
            rewritten_declaration,
            shape.victim_operand(),
        ),
        // The divide's encoded alternatives may architecturally fault; the
        // folded divisor of one discharges that surface, so the validator
        // requires the fault-discharging relationship rather than strict
        // isolation on the consumer side.
        SelectedInstructionKind::ExactDivideU64 { .. }
        | SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            fault_discharged_fold_admission(consumer_declaration, rewritten_declaration)
        }
        _ => {
            isolated_effect_declaration(consumer_declaration)
                && consumer_declaration.alternatives.iter().all(|alternative| {
                    isolated_effect_alternative(alternative)
                        && alternative.encoded.implicit_unit_uses.is_empty()
                        && alternative.encoded.implicit_unit_defs.iter().all(|unit| {
                            rewritten_declaration.alternatives.iter().all(|rewritten| {
                                rewritten.encoded.implicit_unit_defs.contains(unit)
                            })
                        })
                })
                && isolated_rewritten_declaration(rewritten_declaration)
        }
    };
    if !isolated_effect_declaration(producer_declaration)
        || !producer_declaration.alternatives.iter().all(|alternative| {
            isolated_effect_alternative(alternative)
                && alternative.encoded.implicit_unit_uses.is_empty()
                && alternative.encoded.implicit_unit_defs.is_empty()
                && alternative.encoded.implicit_unit_clobbers.is_empty()
        })
        || !fold_surface_admitted
    {
        return Err(LiteralFoldError::EffectSurfaceMismatch {
            function: function_index,
        });
    }

    // The surviving operand is the register each `Use` position of the
    // rewritten row binds: the operand-0 `Use` under a right-literal grammar,
    // the operand-1 `Use` under a left-literal one. The `Use`-free unary
    // grammar records its folded input, and the constant-result grammars —
    // whose rewritten row binds no `Use` at all — record the dropped
    // non-victim `Use` for custody: operand 0 under the right grammars,
    // operand 1 under the left annihilator grammar.
    let surviving = match shape {
        SourceShape::BinaryLeftImmediate | SourceShape::AndZeroLeft => {
            consumer.operands[1].virtual_register
        }
        SourceShape::BinaryImmediate
        | SourceShape::UnaryExtension
        | SourceShape::UnaryCopy
        | SourceShape::DivideIdentity
        | SourceShape::RemainderIdentity
        | SourceShape::AndZero => consumer.operands[0].virtual_register,
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

/// The consumer source grammar the validator admits: the binary immediate
/// forms whose literal is the right `Use` operand — including the
/// three-operand `Load8Indexed` and `ByteViewAddress` projections whose
/// operand-1 `Use` is the folded index or offset and whose operand-2 `Def`
/// is the result — the commutative binary immediate form whose literal is
/// the left `Use` operand, the unary extension and copy forms whose
/// literal is the sole operand, the divide-identity form whose
/// operand-1 divisor literal of one folds into a copy of the dividend and
/// drops every `Use` operand past the operand-2 `Def` result, or the
/// remainder-identity form whose operand-1 divisor literal of one folds
/// into a materialized zero and drops the operand-0 `Use` and every `Def`
/// operand past the operand-2 `Def` result, or the bitwise-and
/// annihilator forms whose zero literal folds `BitwiseAndI64` into a
/// materialized zero — at the operand-1 `Use`, or at the operand-0 `Use`
/// under the left grammar that drops the operand-1 `Use` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceShape {
    BinaryImmediate,
    BinaryLeftImmediate,
    UnaryExtension,
    UnaryCopy,
    DivideIdentity,
    RemainderIdentity,
    AndZero,
    AndZeroLeft,
}

impl SourceShape {
    const fn victim_operand(self) -> u16 {
        match self {
            Self::BinaryImmediate
            | Self::DivideIdentity
            | Self::RemainderIdentity
            | Self::AndZero => 1,
            Self::BinaryLeftImmediate
            | Self::UnaryExtension
            | Self::UnaryCopy
            | Self::AndZeroLeft => 0,
        }
    }
}

/// Whether `register` is defined in `function` only by `MaterializeI64`
/// instructions producing `Unsigned(0)` — the validator's independent
/// re-derivation of the custody the divide-identity grammar requires of
/// every operand it drops. A register with no definition, or any
/// definition that is not a zero materialization, fails: the dropped
/// operand would carry a value the folded form silently stopped
/// observing.
fn dropped_use_defined_zero(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
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

/// Whether `register`'s only occurrence in `function` is one `Def` operand
/// — the validator's independent re-derivation of the custody the
/// constant-result grammar requires of every scratch `Def` it drops. The
/// operand itself is that one occurrence: any other operand position,
/// terminator operand, or successor transport naming the register would
/// leave the fold removing a definition that a surviving read or a second
/// definition still observes. Uses are counted across instruction and
/// terminator operand lists and every successor binding transport, the
/// same sites the rebuild's densification walks.
fn dropped_def_is_dead(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
    let mut occurrences = 0_usize;
    for block in &function.blocks {
        for instruction in block.instructions.iter().chain(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => {
                std::iter::once(instruction)
            }
        }) {
            occurrences += instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
                .count();
        }
        let successors = match &block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
                Vec::new()
            }
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

/// The exact 64-bit output an extension consumer computes on `literal`. The
/// validator recomputes the fold from the concrete consumer kind rather than
/// trusting the producer's recorded action.
fn extension_bits(kind: SelectedInstructionKind, literal: u64) -> Option<u64> {
    match kind {
        SelectedInstructionKind::ZeroExtendU8 => Some(literal & 0xFF),
        SelectedInstructionKind::ZeroExtendU16 => Some(literal & 0xFFFF),
        SelectedInstructionKind::ZeroExtendU32 => Some(literal & 0xFFFF_FFFF),
        SelectedInstructionKind::SignExtendI8 => Some((literal & 0xFF) as u8 as i8 as i64 as u64),
        SelectedInstructionKind::SignExtendI16 => {
            Some((literal & 0xFFFF) as u16 as i16 as i64 as u64)
        }
        SelectedInstructionKind::SignExtendI32 => {
            Some((literal & 0xFFFF_FFFF) as u32 as i32 as i64 as u64)
        }
        _ => None,
    }
}

/// The `IntegerValue` a folded `MaterializeI64` declares for `bits` under the
/// result register's scalar type: `Unsigned` for unsigned integer results,
/// the two's-complement interpretation for signed ones. Non-integer,
/// address-carrier, wider-than-64, or non-admitting result types reject.
fn materialize_value(bits: u64, scalar: ScalarType) -> Option<IntegerValue> {
    let ScalarType::Integer(integer) = scalar else {
        return None;
    };
    if integer.is_address() || integer.bits() > 64 {
        return None;
    }
    let value = match integer.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(bits)),
        IntegerSign::Signed => IntegerValue::Signed(i128::from(bits as i64)),
    };
    integer.admits(value).then_some(value)
}

fn validate_dense_identifiers(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), LiteralFoldError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(LiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain(std::iter::once(match &block.terminator {
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                    | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                    | SelectedTerminator::Jump { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. }
                    | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let count = u32::try_from(ids.len()).map_err(|_| LiteralFoldError::WorkOverflow)?;
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(LiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn rebuild_function(
    function_index: usize,
    function: &mut SelectedFunction,
    action: LiteralFoldAction,
    rows: &ValidationImmediateRows<'_>,
) -> Result<(), LiteralFoldError> {
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let literal_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == action.literal_instruction)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let removed_position =
        u32::try_from(literal_index).map_err(|_| LiteralFoldError::WorkOverflow)?;
    let literal = block.instructions.remove(literal_index);
    for settlement in &mut function.boundary_settlements {
        if settlement.block == action.block && settlement.instruction_index > removed_position {
            settlement.instruction_index -= 1;
        }
    }
    let consumer = block
        .instructions
        .get_mut(literal_index)
        .filter(|instruction| instruction.id == action.consumer_instruction)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;

    let (row, rewritten_kind) = match consumer.kind {
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => (
            rows.add,
            SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(action.immediate)),
                obligation,
                accepted_fact,
            },
        ),
        SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => (
            rows.subtract,
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(action.immediate)),
                obligation,
                accepted_fact,
            },
        ),
        SelectedInstructionKind::CompareI64 => (
            rows.compare,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(action.immediate)),
            },
        ),
        SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::ZeroExtendU32
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::WrappingRemainderI64 { .. }
        | SelectedInstructionKind::BitwiseAndI64 => {
            // The folded materialization must declare the exact constant the
            // result register's scalar type admits; the validator recomputes
            // it from the action payload and the surviving result register.
            let result = action.result.ok_or(LiteralFoldError::ConsumerMismatch {
                function: function_index,
            })?;
            let scalar = function
                .virtual_registers
                .iter()
                .find(|register| register.id == result)
                .map(|register| register.scalar_type)
                .ok_or(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                })?;
            let value = materialize_value(action.immediate, scalar).ok_or(
                LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                },
            )?;
            // The copy fold binds its own policy-gated row, the remainder
            // fold binds the materialize row under its own policy bit, the
            // and-zero fold binds the materialize row under its own policy
            // bit, and the extension consumers bind theirs.
            let row = if consumer.kind == SelectedInstructionKind::CopyI64 {
                rows.copy
            } else if matches!(
                consumer.kind,
                SelectedInstructionKind::WrappingRemainderI64 { .. }
            ) {
                rows.remainder
            } else if consumer.kind == SelectedInstructionKind::BitwiseAndI64 {
                rows.and_zero
            } else {
                rows.materialize
            };
            (row, SelectedInstructionKind::MaterializeI64 { value })
        }
        SelectedInstructionKind::Load8Indexed => (
            rows.load8,
            SelectedInstructionKind::Load8 {
                byte_offset: u32::try_from(action.immediate).map_err(|_| {
                    LiteralFoldError::UnsupportedImmediate {
                        function: function_index,
                    }
                })?,
            },
        ),
        SelectedInstructionKind::ByteViewAddress => (
            rows.address_offset,
            SelectedInstructionKind::AddressOffset {
                byte_offset: u32::try_from(action.immediate).map_err(|_| {
                    LiteralFoldError::UnsupportedImmediate {
                        function: function_index,
                    }
                })?,
            },
        ),
        // A divide by one is the dividend: the validator rebuilds the
        // consumer as a `CopyI64` of the surviving operand, bound to the
        // `CopyI64` row the divide policy gate selected.
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            (rows.divide, SelectedInstructionKind::CopyI64)
        }
        _ => (None, consumer.kind),
    };
    let row = row
        .filter(|row| row.key == action.immediate_constraint)
        .ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;

    let consumer_provenance = consumer.provenance.clone();
    let mut operations = literal.provenance.operations;
    operations.extend(consumer_provenance.operations);
    let mut fuel = literal.provenance.fuel;
    fuel.extend(consumer_provenance.fuel);
    // Bind each rewritten row operand to its recorded register: `Use`
    // positions take the surviving source operand and `Def` positions take
    // the scalar result, so unary constant folds bind only their result.
    let mut registers = Vec::with_capacity(row.operands.len());
    for constraint in &row.operands {
        let register = match constraint.access {
            RegisterOperandAccess::Use => Some(action.surviving),
            RegisterOperandAccess::Def => action.result,
            _ => None,
        };
        registers.push(register.ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?);
    }
    // The divide-identity grammar deliberately drops `fixed_view` pins with
    // the pinned operand form, and the remainder-identity grammar drops the
    // pins and `early_clobber` marks a pinned-scratch realization carries;
    // every other grammar still requires undecorated operands.
    let drops_fixed_views = matches!(
        consumer.kind,
        SelectedInstructionKind::ExactDivideU64 { .. }
            | SelectedInstructionKind::WrappingRemainderI64 { .. }
    );
    let drops_early_clobbers = matches!(
        consumer.kind,
        SelectedInstructionKind::WrappingRemainderI64 { .. }
    );
    if consumer.operands.iter().any(|operand| {
        (operand.fixed_view.is_some() && !drops_fixed_views)
            || operand.tied_to.is_some()
            || (operand.early_clobber && !drops_early_clobbers)
    }) {
        return Err(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    }
    consumer.kind = rewritten_kind;
    consumer.constraint = action.immediate_constraint;
    consumer.operands = row
        .operands
        .iter()
        .zip(registers.iter())
        .map(|(constraint, register)| selected_operand(constraint, *register))
        .collect();
    consumer.implicit_uses = row.implicit_uses.clone();
    consumer.implicit_defs = row.implicit_defs.clone();
    consumer.clobbers = row.clobbers.clone();
    consumer.provenance = SelectedInstructionProvenance {
        operations,
        values: consumer_provenance.values,
        edges: consumer_provenance.edges,
        obligations: consumer_provenance.obligations,
        fuel,
    };

    let victim_index =
        usize::try_from(action.victim.0).map_err(|_| LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        })?;
    if function
        .virtual_registers
        .get(victim_index)
        .map(|register| register.id)
        != Some(action.victim)
    {
        return Err(LiteralFoldError::DecisionMismatch {
            function: function_index,
        });
    }
    function.virtual_registers.remove(victim_index);
    redensify(
        function_index,
        function,
        action.literal_instruction,
        action.victim,
    )
}

fn redensify(
    function_index: usize,
    function: &mut SelectedFunction,
    removed_instruction: SelectedInstructionId,
    removed_register: VirtualRegisterId,
) -> Result<(), LiteralFoldError> {
    for call in &mut function.calls {
        call.instruction =
            lower_instruction(function_index, call.instruction, removed_instruction)?;
    }
    for access in &mut function.memory_accesses {
        access.instruction =
            lower_instruction(function_index, access.instruction, removed_instruction)?;
    }
    for register in &mut function.virtual_registers {
        register.id = lower_register(function_index, register.id, removed_register)?;
        match &mut register.origin {
            VirtualRegisterOrigin::InstructionResult { instruction, .. }
            | VirtualRegisterOrigin::SpillAddress { instruction, .. }
            | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
            | VirtualRegisterOrigin::InstructionScratch { instruction, .. }
            | VirtualRegisterOrigin::AbiTransport { instruction, .. } => {
                *instruction =
                    lower_instruction(function_index, *instruction, removed_instruction)?;
            }
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. }
            | VirtualRegisterOrigin::BlockParameter { .. } => {}
        }
    }
    for block in &mut function.blocks {
        for instruction in &mut block.instructions {
            lower_selected_instruction(
                function_index,
                instruction,
                removed_instruction,
                removed_register,
            )?;
        }
        let successors = match &mut block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
                Vec::new()
            }
        };
        for successor in successors {
            for binding in &mut successor.structural_bindings {
                if let selected_instructions::SelectedStructuralTransport::Descriptor {
                    argument,
                    ..
                }
                | selected_instructions::SelectedStructuralTransport::WholeValue {
                    argument,
                    ..
                } = &mut binding.transport
                {
                    *argument = lower_register(function_index, *argument, removed_register)?;
                }
            }
            if let Some(case) = &mut successor.structural_case {
                for payload in &mut case.payloads {
                    match &mut payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => {}
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter,
                        } => {
                            *parameter =
                                lower_register(function_index, *parameter, removed_register)?;
                        }
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            *argument =
                                lower_register(function_index, *argument, removed_register)?;
                            *parameter =
                                lower_register(function_index, *parameter, removed_register)?;
                        }
                    }
                }
            }
            for binding in &mut successor.bindings {
                if let selected_instructions::SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = &mut binding.transport
                {
                    *argument = lower_register(function_index, *argument, removed_register)?;
                    *parameter = lower_register(function_index, *parameter, removed_register)?;
                }
            }
        }
        match &mut block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => {
                lower_selected_instruction(
                    function_index,
                    instruction,
                    removed_instruction,
                    removed_register,
                )?;
            }
        }
    }
    Ok(())
}

fn lower_selected_instruction(
    function_index: usize,
    instruction: &mut SelectedInstruction,
    removed_instruction: SelectedInstructionId,
    removed_register: VirtualRegisterId,
) -> Result<(), LiteralFoldError> {
    instruction.id = lower_instruction(function_index, instruction.id, removed_instruction)?;
    for operand in &mut instruction.operands {
        operand.virtual_register =
            lower_register(function_index, operand.virtual_register, removed_register)?;
    }
    Ok(())
}

fn lower_instruction(
    function_index: usize,
    id: SelectedInstructionId,
    removed: SelectedInstructionId,
) -> Result<SelectedInstructionId, LiteralFoldError> {
    if id == removed {
        return Err(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(SelectedInstructionId(if id > removed {
        id.0.checked_sub(1)
            .ok_or(LiteralFoldError::IdentifierUnderflow {
                function: function_index,
            })?
    } else {
        id.0
    }))
}

fn lower_register(
    function_index: usize,
    id: VirtualRegisterId,
    removed: VirtualRegisterId,
) -> Result<VirtualRegisterId, LiteralFoldError> {
    if id == removed {
        return Err(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(VirtualRegisterId(if id > removed {
        id.0.checked_sub(1)
            .ok_or(LiteralFoldError::IdentifierUnderflow {
                function: function_index,
            })?
    } else {
        id.0
    }))
}

fn selected_operand(
    constraint: &register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

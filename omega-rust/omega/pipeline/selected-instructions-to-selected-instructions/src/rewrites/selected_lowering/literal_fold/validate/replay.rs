use register_model::{RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    MachineSemanticKind, SaturatingCarrier, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, RecoveryClassification,
    RecoveryVictimRole, ValidatedRecoveryClassifications, ValidatedSelectedAnalysis,
    machine_semantic_kind,
};

use super::constraints::{
    ValidationImmediateRows, dead_unit_defs_fold_admission, effect_declaration,
    fault_discharged_dead_unit_defs_fold_admission, fault_discharged_fold_admission,
    indexed_read_fold_admission, isolated_effect_alternative, isolated_effect_declaration,
    isolated_rewritten_declaration, obligation_discharged_dead_unit_defs_fold_admission,
    obligation_discharged_fold_admission,
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

    let (block_index, block) = function
        .blocks
        .iter()
        .enumerate()
        .find(|(_, block)| block.id == candidate.block)
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
        // The compare binds the same `CompareI64Immediate` row at either
        // `Use` position, but the grammars are not interchangeable: the
        // operand-1 subtrahend literal rewrites in place — `x - literal`
        // keeps the operand order — while the operand-0 minuend literal
        // rewrites the operand-swapped `x - literal` for `literal - x`,
        // which preserves the zero condition but inverts every ordering
        // predicate. The left grammar therefore carries the reader-flow
        // audit the right one does not need.
        SelectedInstructionKind::CompareI64 => (
            if future_use.operand == 0 {
                SourceShape::CompareLeftImmediate
            } else {
                SourceShape::BinaryImmediate
            },
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
        // The byte-view address projection is a commutative modular
        // address addition — `(backing + offset) modulo 2^64` — so a
        // literal at either `Use` position folds into the constant-offset
        // `AddressOffset` form, bound to the `AddressOffset` row: the
        // operand-1 offset literal derives the right-fold grammar whose
        // surviving operand-0 `Use` is the base, and the operand-0
        // backing literal derives the commuted left-fold grammar whose
        // surviving operand-1 `Use` binds the rewritten row's base
        // position. The operand-2 `Def` is the result under either
        // grammar.
        SelectedInstructionKind::ByteViewAddress => (
            if future_use.operand == 0 {
                SourceShape::BinaryLeftImmediate
            } else {
                SourceShape::BinaryImmediate
            },
            rows.address_offset,
            MachineSemanticKind::AddressOffset,
        ),
        // Two disjoint families fold `ExactDivideU64`; the folded literal's
        // operand position names the family a fold belongs to. The
        // divisor-one fold admits an operand-1 divisor literal of exactly
        // one — a divide by one is the dividend — bound to the `CopyI64`
        // row the divide policy's own gate selected; operands past the
        // operand-2 `Def` result are `Use` positions the fold drops — the
        // zeroed high-half input an x86-64 `div` realization reads — each
        // independently required to be defined only by zero
        // materializations. The zero-dividend fold admits an operand-0
        // dividend literal of exactly zero — `0 / x` is `0` for every `x`
        // the consumer's proven nonzero divisor admits — bound to the
        // `MaterializeI64` row the zero-dividend policy's own gate
        // selected, dropping the operand-1 divisor `Use` and the same
        // provably-zero auxiliary `Use` operands. Its fault surface
        // retires under the nonzero-divisor obligation the kind carries,
        // not under the folded literal.
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            if future_use.operand == 0 {
                (
                    SourceShape::DivideZeroDividend,
                    rows.divide_zero,
                    MachineSemanticKind::MaterializeI64,
                )
            } else {
                (
                    SourceShape::DivideIdentity,
                    rows.divide,
                    MachineSemanticKind::CopyI64,
                )
            }
        }
        // Three disjoint families fold `WrappingRemainderI64`. The
        // zero-dividend fold admits an operand-0 dividend literal of
        // exactly zero — `0 % x` is `0` for every `x` — bound to the
        // `MaterializeI64` row the zero-dividend policy's own gate
        // selected, dropping the operand-1 divisor `Use` and every dead
        // scratch `Def`; its fault surface retires under the
        // nonzero-divisor obligation the kind carries, not under the
        // folded literal. The two divisor folds share operand 1, and the
        // literal's value names the family a fold belongs to: the
        // divisor-one fold admits a literal of exactly one — a remainder
        // by one is always zero — bound to the `MaterializeI64` row the
        // remainder policy's own gate selected, while the minus-one fold
        // admits the all-ones literal — the normalized-i64 divisor `-1`,
        // whose remainder is always zero including the `i64::MIN`
        // dividend the kind defines to produce zero rather than trap —
        // bound to the `MaterializeI64` row the minus-one policy's own
        // gate selected. Either divisor grammar drops the operand-0
        // dividend `Use` the constant result never reads and every `Def`
        // operand past the operand-2 `Def` result under occurrence-free
        // custody, and either retires the encoded fault surface under
        // the folded divisor literal itself. A literal neither divisor
        // family admits replays under the divisor-one shape so the
        // family's exact-literal check rejects it.
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            if future_use.operand == 0 {
                (
                    SourceShape::RemainderZeroDividend,
                    rows.remainder_zero,
                    MachineSemanticKind::MaterializeI64,
                )
            } else if literal_u64 == u64::MAX {
                (
                    SourceShape::RemainderMinusOne,
                    rows.remainder_minus_one,
                    MachineSemanticKind::MaterializeI64,
                )
            } else {
                (
                    SourceShape::RemainderIdentity,
                    rows.remainder,
                    MachineSemanticKind::MaterializeI64,
                )
            }
        }
        // Two disjoint families fold `BitwiseAndI64` at either `Use`
        // position; the literal's value names the family a fold belongs
        // to. The and-zero annihilator admits exactly the literal zero —
        // `0 & x` and `x & 0` are both zero — and folds into a
        // `MaterializeI64` of zero bound to the `MaterializeI64` row the
        // and-zero policy's own gate selected; the other `Use` drops
        // because the constant result never reads it, and every `Def`
        // operand past the operand-2 result drops under the same
        // occurrence-free custody the remainder grammar derives. The
        // and-ones identity admits exactly the all-ones literal —
        // `MAX & x` and `x & MAX` are both `x` — and folds into a
        // `CopyI64` of the surviving `Use` bound to the `CopyI64` row the
        // and-ones policy's own gate selected. A literal neither family
        // admits replays under the annihilator shapes so the family's
        // exact-literal check rejects it; which policy rows bound decides
        // whether that rejection reports the unadmitted literal or the
        // unadmitted kind. The recorded operand position picks the
        // grammar within each family.
        SelectedInstructionKind::BitwiseAndI64 => {
            let left = future_use.operand == 0;
            if literal_u64 == u64::MAX {
                (
                    if left {
                        SourceShape::AndOnesLeft
                    } else {
                        SourceShape::AndOnes
                    },
                    rows.and_ones,
                    MachineSemanticKind::CopyI64,
                )
            } else {
                (
                    if left {
                        SourceShape::AndZeroLeft
                    } else {
                        SourceShape::AndZero
                    },
                    rows.and_zero,
                    MachineSemanticKind::MaterializeI64,
                )
            }
        }
        // The bitwise-xor identity fold: a literal of exactly zero at
        // either `Use` folds `BitwiseXorI64` into a `CopyI64` of the other
        // `Use` — `x ^ 0` and `0 ^ x` are both `x` — bound to the
        // `CopyI64` row the xor-zero policy's own gate selected. The
        // surviving `Use` binds the rewritten row's operand-0 `Use`
        // position. The recorded operand position picks the grammar.
        SelectedInstructionKind::BitwiseXorI64 => (
            if future_use.operand == 0 {
                SourceShape::XorZeroLeft
            } else {
                SourceShape::XorZero
            },
            rows.xor_zero,
            MachineSemanticKind::CopyI64,
        ),
        // The wrapping-add identity fold: a literal of exactly zero at
        // either `Use` folds `WrappingAddI64` into a `CopyI64` of the other
        // `Use` — `x + 0` and `0 + x` are both `x` modulo 2^64 — bound to
        // the `CopyI64` row the wrapping-add-zero policy's own gate
        // selected. The surviving `Use` binds the rewritten row's
        // operand-0 `Use` position. The recorded operand position picks
        // the grammar.
        SelectedInstructionKind::WrappingAddI64 => (
            if future_use.operand == 0 {
                SourceShape::WrappingAddZeroLeft
            } else {
                SourceShape::WrappingAddZero
            },
            rows.wrapping_add_zero,
            MachineSemanticKind::CopyI64,
        ),
        // Two disjoint families fold `SaturatingAdd` at either `Use`
        // position; the literal's value names the family a fold belongs
        // to. The zero-identity fold admits a literal of exactly zero on
        // any carrier — `x +| 0` and `0 +| x` are both `x`, already inside
        // the carrier's bounds — bound to the `CopyI64` row the
        // saturating-add-zero policy's own gate selected. The upper-bound
        // fold admits a literal of exactly the carrier's maximum on an
        // unsigned carrier — `x +| MAX` and `MAX +| x` are both `MAX`,
        // because `x + MAX` reaches the carrier's upper bound and
        // saturates to it — bound to the `MaterializeI64` row the
        // upper-bound policy's own gate selected, dropping the other
        // `Use` the constant result never reads. A signed carrier's
        // maximum literal names no admitted grammar — `x +| MAX` there
        // is `x + MAX` unclamped for every negative `x`, not a constant —
        // and replays under the zero shapes so the family's exact-literal
        // check rejects it; which policy rows bound decides whether that
        // rejection reports the unadmitted literal or the unadmitted
        // kind. Either family's consumer implicitly defines the target
        // condition state on aarch64 — every carrier's realization is
        // flag-setting — and clobbers `rflags` on x86-64; the fold
        // retires both with the folded form under the per-kind admission
        // below. Under the identity family the carrier picks the operand
        // grammar: the u64 row is exactly `[left, victim, result]`; every
        // clamped carrier's row continues past the `Def` result with a
        // bound-scratch `Def` the fold drops under the occurrence-free
        // custody the scratch-defs grammars independently re-derive. The
        // constant-result grammars admit the same tail the same way.
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            let left = future_use.operand == 0;
            if !carrier.is_signed() && literal_u64 == carrier.maximum_bits() {
                (
                    if left {
                        SourceShape::SaturatingAddUpperBoundLeft
                    } else {
                        SourceShape::SaturatingAddUpperBound
                    },
                    rows.saturating_add_upper_bound,
                    MachineSemanticKind::MaterializeI64,
                )
            } else {
                (
                    match (carrier, left) {
                        (SaturatingCarrier::U64, true) => SourceShape::SaturatingAddZeroLeft,
                        (SaturatingCarrier::U64, false) => SourceShape::SaturatingAddZero,
                        (_, true) => SourceShape::SaturatingAddZeroLeftScratch,
                        (_, false) => SourceShape::SaturatingAddZeroScratch,
                    },
                    rows.saturating_add_zero,
                    MachineSemanticKind::CopyI64,
                )
            }
        }
        // Three disjoint families fold `SaturatingSubtract`; the folded
        // literal's operand position names the operand-0 family, and the
        // literal's value names which of the two operand-1 families a
        // fold belongs to. The right-zero identity fold admits an
        // operand-1 literal of exactly zero on any carrier — `x -| 0` is
        // `x`, already inside the carrier's bounds — bound to the
        // `CopyI64` row the saturating-subtract-zero policy's own gate
        // selected; its grammar is asymmetric — `0 -| x` is not `x` — so
        // the right-literal shapes are that family's only shapes. The
        // upper-bound subtrahend fold admits an operand-1 literal of
        // exactly the carrier's maximum on an unsigned carrier — `x -|
        // MAX` is `0` for every `x`, because `x - MAX` underflows the
        // carrier's lower bound and saturates to it — bound to the
        // `MaterializeI64` row the upper-bound policy's own gate
        // selected, dropping the operand-0 minuend `Use` the constant
        // result never reads. The zero-minuend fold admits an operand-0
        // literal of exactly zero on an unsigned carrier — `0 -| x` is
        // `0` for every `x`, because
        // `0 - x` underflows the carrier's lower bound and saturates to
        // it — bound to the `MaterializeI64` row the zero-minuend
        // policy's own gate selected, dropping the operand-1 subtrahend
        // `Use` the constant result never reads. A signed carrier's
        // operand-0 literal names no admitted grammar — `0 -| x` there is
        // `-x` clamped to the carrier's bounds, not a constant — and
        // neither does a signed carrier's maximum subtrahend literal —
        // `x -| MAX` there is `x - MAX` clamped to the carrier's lower
        // bound for every negative `x`, not a constant — so each
        // replays under the zero shapes so the family's exact-literal
        // check rejects it; which policy rows bound decides whether that
        // rejection reports the unadmitted literal or the unadmitted
        // kind. Either family's consumer implicitly
        // defines the target condition state on aarch64 — every carrier's
        // realization is flag-setting — and clobbers `rflags` on x86-64;
        // the fold retires both with the folded form under the per-kind
        // admission below. The carrier's signedness picks the identity
        // family's operand grammar: the unsigned row is exactly `[left,
        // victim, result]`; every signed carrier's clamped row continues
        // past the `Def` result with a bound-scratch `Def` the fold drops
        // under the occurrence-free custody the scratch-defs grammar
        // independently re-derives.
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            if future_use.operand == 0 && !carrier.is_signed() {
                (
                    SourceShape::SaturatingSubtractZeroMinuend,
                    rows.saturating_subtract_zero_minuend,
                    MachineSemanticKind::MaterializeI64,
                )
            } else if !carrier.is_signed() && literal_u64 == carrier.maximum_bits() {
                (
                    SourceShape::SaturatingSubtractUpperBoundSubtrahend,
                    rows.saturating_subtract_upper_bound,
                    MachineSemanticKind::MaterializeI64,
                )
            } else {
                (
                    if carrier.is_signed() {
                        SourceShape::SaturatingSubtractZeroScratch
                    } else {
                        SourceShape::SaturatingSubtractZero
                    },
                    rows.saturating_subtract_zero,
                    MachineSemanticKind::CopyI64,
                )
            }
        }
        // Two disjoint families fold `SaturatingDivide` on any carrier;
        // the folded literal's operand position names the family a fold
        // belongs to. The divisor-one fold admits an operand-1 literal of
        // exactly one — `x /| 1` is `x`, already inside the carrier's
        // bounds — bound to the `CopyI64` row the saturating-divide-one
        // policy's own gate selected; the literal of one is itself the
        // evidence the encoded fault surface cannot fire, and the
        // consumer's implicit unit definitions retire under the
        // whole-function deadness gate the saturating-add grammar
        // derives. The zero-dividend fold admits an operand-0 dividend
        // literal of exactly zero — `0 /| x` is `0` for every `x` the
        // consumer's proven nonzero divisor admits — bound to the
        // `MaterializeI64` row the zero-dividend policy's own gate
        // selected, dropping the operand-1 divisor `Use` and the same
        // provably-zero auxiliary `Use` operands. Its fault surface
        // retires under the nonzero-divisor obligation the kind carries,
        // not under the folded literal. Either family's operands past
        // the operand-2 `Def` result admit the mixed custody no other
        // grammar carries: a `Use` — the zeroed high-half input an x86-64
        // `div` realization reads — must be defined only by zero
        // materializations, and a `Def` — the bound scratch an aarch64
        // signed realization writes — must be occurrence-free.
        SelectedInstructionKind::SaturatingDivide { .. } => {
            if future_use.operand == 0 {
                (
                    SourceShape::SaturatingDivideZeroDividend,
                    rows.saturating_divide_zero,
                    MachineSemanticKind::MaterializeI64,
                )
            } else {
                (
                    SourceShape::SaturatingDivideOne,
                    rows.saturating_divide_one,
                    MachineSemanticKind::CopyI64,
                )
            }
        }
        _ => (
            SourceShape::BinaryImmediate,
            None,
            MachineSemanticKind::MaterializeI64,
        ),
    };
    // `BitwiseAndI64` and `SaturatingAdd` are the consumer kinds two
    // disjoint families admit at the same operand positions — the
    // literal's value names the family, so when that family's row was not
    // bound the position may still be admitted by the other family: an
    // admitted operand position whose literal no enabled family admits is
    // an unsupported immediate, a position outside both grammars a
    // future-use mismatch, and either kind with no enabled family a
    // consumer mismatch like any other unadmitted kind. The upper-bound
    // grammar binds only the unsigned carriers: a signed carrier's
    // operand positions name no admitted grammar even while the
    // upper-bound family is enabled, so the enabled-family check counts
    // only the rows a `SaturatingAdd` of this carrier can bind.
    // `WrappingRemainderI64` carries the same value-disjoint structure on
    // its divisor operand — the divisor-one and minus-one families share
    // operand 1 — while its operand-0 zero-dividend family is
    // position-disjoint like `ExactDivideU64`'s and `SaturatingDivide`'s
    // two-family splits: for those kinds the
    // position already picked the family, so its row being unbound means
    // no enabled grammar covers the position — a future-use mismatch
    // while either family of the kind is enabled, a consumer mismatch
    // when neither is. `SaturatingSubtract` mixes both structures: its
    // operand-0 zero-minuend family is position-disjoint while its
    // operand-1 subtrahend operand hosts two value-disjoint families —
    // the right-zero identity on every carrier and the upper-bound fold
    // on the unsigned ones.
    let row = row.ok_or_else(|| {
        let same_position_families = match consumer.kind {
            SelectedInstructionKind::BitwiseAndI64 => {
                rows.and_zero.is_some() || rows.and_ones.is_some()
            }
            SelectedInstructionKind::SaturatingAdd { carrier } => {
                rows.saturating_add_zero.is_some()
                    || (rows.saturating_add_upper_bound.is_some() && !carrier.is_signed())
            }
            _ => false,
        };
        if same_position_families {
            if future_use.operand == shape.victim_operand() {
                LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                }
            } else {
                LiteralFoldError::FutureUseMismatch {
                    function: function_index,
                }
            }
        } else if matches!(
            consumer.kind,
            SelectedInstructionKind::WrappingRemainderI64 { .. }
        ) && (rows.remainder.is_some()
            || rows.remainder_zero.is_some()
            || rows.remainder_minus_one.is_some())
        {
            // The divisor operand hosts two value-disjoint families — a
            // bound row missing there names a literal neither enabled
            // divisor family admits, while an unbound row at any other
            // position names a position no enabled grammar covers.
            if future_use.operand == 1
                && (rows.remainder.is_some() || rows.remainder_minus_one.is_some())
            {
                LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                }
            } else {
                LiteralFoldError::FutureUseMismatch {
                    function: function_index,
                }
            }
        } else if let SelectedInstructionKind::SaturatingSubtract { carrier } = consumer.kind {
            // The zero-minuend and upper-bound grammars bind only the
            // unsigned carriers: a signed carrier's operand positions
            // name no admitted grammar outside the right-zero family even
            // while an unsigned-only family is enabled, so the
            // enabled-family check counts only the rows a
            // `SaturatingSubtract` of this carrier can bind. Within the
            // admitted kind the subtrahend operand hosts two
            // value-disjoint families — a bound row missing there names a
            // literal neither enabled subtrahend family admits, while an
            // unbound row at any other position names a position no
            // enabled grammar covers.
            let kind_admitted = rows.saturating_subtract_zero.is_some()
                || (!carrier.is_signed()
                    && (rows.saturating_subtract_zero_minuend.is_some()
                        || rows.saturating_subtract_upper_bound.is_some()));
            if !kind_admitted {
                LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                }
            } else if (future_use.operand == 1
                && (rows.saturating_subtract_zero.is_some()
                    || (!carrier.is_signed() && rows.saturating_subtract_upper_bound.is_some())))
                || (future_use.operand == 0
                    && !carrier.is_signed()
                    && rows.saturating_subtract_zero_minuend.is_some())
            {
                LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                }
            } else {
                LiteralFoldError::FutureUseMismatch {
                    function: function_index,
                }
            }
        } else if (matches!(
            consumer.kind,
            SelectedInstructionKind::ExactDivideU64 { .. }
        ) && (rows.divide.is_some() || rows.divide_zero.is_some()))
            || (matches!(
                consumer.kind,
                SelectedInstructionKind::SaturatingDivide { .. }
            ) && (rows.saturating_divide_one.is_some()
                || rows.saturating_divide_zero.is_some()))
        {
            LiteralFoldError::FutureUseMismatch {
                function: function_index,
            }
        } else {
            LiteralFoldError::ConsumerMismatch {
                function: function_index,
            }
        }
    })?;
    if future_use.operand != shape.victim_operand() {
        return Err(LiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    }
    let immediate = match shape {
        // The operand-swapped compare grammar shares the same twelve-bit
        // bound the in-place compare grammar admits — the immediate field
        // the rewritten form encodes is identical.
        SourceShape::BinaryImmediate
        | SourceShape::BinaryLeftImmediate
        | SourceShape::CompareLeftImmediate => {
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
        // The divide zero-dividend fold is the constant zero only when the
        // dividend literal is exactly zero — `0 / x` is `0` for every `x`
        // the consumer's proven nonzero divisor admits; any other dividend
        // is a different computation the replay must not admit. The
        // recorded immediate is the constant the rewritten `MaterializeI64`
        // embeds — zero.
        SourceShape::DivideZeroDividend => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            0
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
        // The minus-one remainder fold is the constant zero only when the
        // divisor literal is `u64::MAX` — the normalized-i64 divisor `-1`:
        // `x % -1` is `0` for every `x`, including the `i64::MIN` dividend
        // the kind's semantics defines to produce zero rather than trap;
        // any other divisor is a different computation the replay must not
        // admit. The recorded immediate is the constant the rewritten
        // `MaterializeI64` embeds — zero.
        SourceShape::RemainderMinusOne => {
            if literal_u64 != u64::MAX {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            0
        }
        // The zero-dividend fold is the constant zero only when the
        // dividend literal is exactly zero — `0 % x` is `0` for every `x`
        // the consumer's proven nonzero divisor admits; any other dividend
        // is a different computation the replay must not admit. The
        // recorded immediate is the constant the rewritten `MaterializeI64`
        // embeds — zero.
        SourceShape::RemainderZeroDividend => {
            if literal_u64 != 0 {
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
        // The xor fold is the identity only when the folded literal is
        // exactly zero — zero is the bitwise-xor identity element at
        // either `Use` position; any other literal is a different
        // computation the replay must not admit. The recorded immediate
        // is the folded literal itself, unused by the `CopyI64` rebuild.
        SourceShape::XorZero | SourceShape::XorZeroLeft => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
        }
        // The wrapping-add fold is the identity only when the folded
        // literal is exactly zero — zero is the additive identity element
        // under modulo-2^64 wrap at either `Use` position; any other
        // literal is a different computation the replay must not admit.
        // The recorded immediate is the folded literal itself, unused by
        // the `CopyI64` rebuild.
        SourceShape::WrappingAddZero | SourceShape::WrappingAddZeroLeft => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
        }
        // The saturating-add fold is the identity only when the folded
        // literal is exactly zero — zero is the additive identity under
        // saturating addition on every carrier at either `Use` position;
        // any other literal is a different computation the replay must
        // not admit. The recorded immediate is the folded literal itself,
        // unused by the `CopyI64` rebuild.
        SourceShape::SaturatingAddZero
        | SourceShape::SaturatingAddZeroLeft
        | SourceShape::SaturatingAddZeroScratch
        | SourceShape::SaturatingAddZeroLeftScratch => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
        }
        // The saturating-add upper-bound fold is the carrier's maximum
        // only when the folded literal is exactly that maximum on an
        // unsigned carrier — `x +| MAX` and `MAX +| x` are both `MAX`
        // because `x + MAX` reaches the carrier's upper bound and
        // saturates to it; any other literal, and every signed carrier —
        // where `x +| MAX` is `x + MAX` unclamped for every negative
        // `x`, not a constant — is a different computation the replay
        // must not admit. The recorded immediate is the constant the
        // rewritten `MaterializeI64` embeds — the carrier maximum the
        // validator recomputes from the consumer kind itself.
        SourceShape::SaturatingAddUpperBound | SourceShape::SaturatingAddUpperBoundLeft => {
            let SelectedInstructionKind::SaturatingAdd { carrier } = consumer.kind else {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            };
            if carrier.is_signed() || literal_u64 != carrier.maximum_bits() {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            carrier.maximum_bits()
        }
        // The saturating-subtract fold is the identity only when the folded
        // literal is exactly zero — zero is the right identity under
        // saturating subtraction on every carrier; any other right literal
        // is a different computation the replay must not admit. The
        // recorded immediate is the folded literal itself, unused by the
        // `CopyI64` rebuild.
        SourceShape::SaturatingSubtractZero | SourceShape::SaturatingSubtractZeroScratch => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
        }
        // The saturating-subtract zero-minuend fold is the constant zero
        // only when the minuend literal is exactly zero — `0 -| x` is `0`
        // for every `x` an unsigned carrier admits, because `0 - x`
        // underflows the carrier's lower bound and saturates to it; any
        // other minuend is a different computation the replay must not
        // admit. The recorded immediate is the constant the rewritten
        // `MaterializeI64` embeds — zero.
        SourceShape::SaturatingSubtractZeroMinuend => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            0
        }
        // The saturating-subtract upper-bound fold is the constant zero
        // only when the subtrahend literal is exactly the carrier's
        // maximum on an unsigned carrier — `x -| MAX` is `0` for every
        // `x` an unsigned carrier admits, because `x - MAX` underflows
        // the carrier's lower bound and saturates to it for every
        // `x < MAX` and is exactly zero at `x == MAX`; any other
        // subtrahend, and every signed carrier — where `x -| MAX` is
        // `x - MAX` clamped to the carrier's lower bound for every
        // negative `x`, not a constant — is a different computation the
        // replay must not admit. The recorded immediate is the constant
        // the rewritten `MaterializeI64` embeds — zero.
        SourceShape::SaturatingSubtractUpperBoundSubtrahend => {
            let SelectedInstructionKind::SaturatingSubtract { carrier } = consumer.kind else {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            };
            if carrier.is_signed() || literal_u64 != carrier.maximum_bits() {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            0
        }
        // The saturating-divide fold is the identity only when the folded
        // literal is exactly one — one is the right divisor identity on
        // every carrier, and the literal itself is the evidence the
        // consumer's encoded fault surface cannot fire; any other right
        // literal is a different computation the replay must not admit.
        // The recorded immediate is the folded literal itself, unused by
        // the `CopyI64` rebuild.
        SourceShape::SaturatingDivideOne => {
            if literal_u64 != 1 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
        }
        // The saturating-divide zero-dividend fold is the constant zero
        // only when the dividend literal is exactly zero — `0 /| x` is
        // `0` inside every carrier's bounds for every `x` the consumer's
        // proven nonzero divisor admits; any other dividend is a
        // different computation the replay must not admit. The recorded
        // immediate is the constant the rewritten `MaterializeI64`
        // embeds — zero.
        SourceShape::SaturatingDivideZeroDividend => {
            if literal_u64 != 0 {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            0
        }
        // The and-ones fold is the identity only when the folded literal
        // is all ones — `u64::MAX` is the bitwise-and identity element at
        // either `Use` position; any other literal is a different
        // computation the replay must not admit. The recorded immediate
        // is the folded literal itself, unused by the `CopyI64` rebuild.
        SourceShape::AndOnes | SourceShape::AndOnesLeft => {
            if literal_u64 != u64::MAX {
                return Err(LiteralFoldError::UnsupportedImmediate {
                    function: function_index,
                });
            }
            literal_u64
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
        // The operand-swapped compare grammar: `[victim, subtrahend]`
        // folds the operand-0 `Use` — the literal minuend — and binds the
        // operand-1 subtrahend `Use` into the rewritten row's sole `Use`
        // position, computing `subtrahend - literal` for
        // `literal - subtrahend`. The validator re-derives the unit
        // surface itself: the record must publish exactly the implicit
        // uses, definitions, and clobbers the rewritten row carries —
        // the reader audit below covers the record's definitions, so
        // those are the units the rewrite must publish, and an implicit
        // use or clobber the row does not carry would silently stop
        // being observed.
        (SourceShape::CompareLeftImmediate, [victim, subtrahend]) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != candidate.victim
                || subtrahend.access != RegisterOperandAccess::Use
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Use
                || subtrahend.class != row.operands[0].class
                || consumer.implicit_uses != row.implicit_uses
                || consumer.implicit_defs != row.implicit_defs
                || consumer.clobbers != row.clobbers
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
        // The divide zero-dividend grammar: `[victim, divisor, result,
        // aux...]` folds the operand-0 `Use` — the zero dividend — drops
        // the operand-1 divisor `Use` because the constant result never
        // reads it, and drops every `Use` operand past the operand-2 `Def`
        // result. The validator independently re-derives the
        // dropped-operand custody: each dropped register must be defined
        // in this function only by `MaterializeI64` instructions producing
        // `Unsigned(0)` — the zeroed high-half input an x86-64 `div`
        // realization reads as the dividend's upper half — because the
        // fold discards whatever the operand carried, and a literal of
        // zero at operand 0 fixes only the low dividend half.
        (SourceShape::DivideZeroDividend, [victim, right, result, auxiliary @ ..]) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != candidate.victim
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
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
        // The divisor constant-result grammars: `[dividend, divisor,
        // result, scratch...]` folds the operand-1 `Use`, drops the
        // operand-0 `Use` — the constant result never reads it — and
        // drops every `Def` operand past the operand-2 `Def` result. The
        // divisor-one and divisor-minus-one folds share this grammar;
        // the literal check above already fixed which family the fold
        // belongs to. The validator independently re-derives the
        // dropped-operand custody: each dropped `Def` register must
        // occur nowhere else in the function — the dead quotient scratch
        // an x86-64 `idiv` realization writes — because the fold
        // discards a definition a surviving read or second definition
        // would still observe.
        (
            SourceShape::RemainderIdentity | SourceShape::RemainderMinusOne,
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
        // The zero-dividend grammar: `[victim, divisor, result,
        // scratch...]` folds the operand-0 `Use` — the zero dividend —
        // drops the operand-1 divisor `Use` because the constant result
        // never reads it, and drops every `Def` operand past the operand-2
        // `Def` result under the same occurrence-free custody the
        // divisor-one grammar derives: each dropped `Def` register must
        // occur nowhere else in the function — the dead quotient scratch
        // an x86-64 `idiv` realization writes.
        (SourceShape::RemainderZeroDividend, [victim, right, result, scratch @ ..]) => {
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
        // The xor-identity grammar: `[surviving, victim, result]` folds
        // the operand-1 `Use`; the operand-0 `Use` survives and binds the
        // `CopyI64` row's `Use` position. The consumer carries exactly
        // three operands — an operand past the `Def` result has no
        // droppable role under this grammar.
        (SourceShape::XorZero, [left, right, result]) => {
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
        // The commuted xor-identity grammar: `[victim, surviving, result]`
        // folds the operand-0 `Use`; the operand-1 `Use` survives into the
        // `CopyI64` row's `Use` position because bitwise xor commutes —
        // `0 ^ x` is `x ^ 0` is `x`.
        (SourceShape::XorZeroLeft, [victim, right, result]) => {
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
        // The wrapping-add identity grammar: `[surviving, victim, result]`
        // folds the operand-1 `Use`; the operand-0 `Use` survives and binds
        // the `CopyI64` row's `Use` position. The consumer carries exactly
        // three operands — an operand past the `Def` result has no
        // droppable role under this grammar.
        (SourceShape::WrappingAddZero, [left, right, result]) => {
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
        // The commuted wrapping-add identity grammar: `[victim, surviving,
        // result]` folds the operand-0 `Use`; the operand-1 `Use` survives
        // into the `CopyI64` row's `Use` position because wrapping
        // addition commutes — `0 + x` is `x + 0` is `x` modulo 2^64.
        (SourceShape::WrappingAddZeroLeft, [victim, right, result]) => {
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
        // The saturating-add identity grammar: `[surviving, victim,
        // result]` folds the operand-1 `Use`; the operand-0 `Use`
        // survives and binds the `CopyI64` row's `Use` position. The
        // u64-carrier consumer carries exactly three operands — an
        // operand past the `Def` result has no droppable role under this
        // grammar.
        (SourceShape::SaturatingAddZero, [left, right, result]) => {
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
        // The commuted saturating-add identity grammar: `[victim,
        // surviving, result]` folds the operand-0 `Use`; the operand-1
        // `Use` survives into the `CopyI64` row's `Use` position because
        // unsigned saturating addition commutes — `0 +| x` is `x +| 0`
        // is `x` inside the carrier's bounds. The u64-carrier consumer
        // carries exactly three operands.
        (SourceShape::SaturatingAddZeroLeft, [victim, right, result]) => {
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
        // The clamped-carrier saturating-add grammar: `[surviving, victim,
        // result, scratch...]` folds the operand-1 `Use`; the operand-0
        // `Use` survives and binds the `CopyI64` row's `Use` position, and
        // every operand past the operand-2 `Def` result is a scratch `Def`
        // — the bound output a clamped realization computes its
        // saturation bound through. The validator independently re-derives
        // the dropped-operand custody: each dropped `Def` register must
        // occur nowhere else in the function, because the fold discards a
        // definition a surviving read or second definition would still
        // observe.
        (SourceShape::SaturatingAddZeroScratch, [left, right, result, scratch @ ..]) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
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
        // The commuted clamped-carrier grammar: `[victim, surviving,
        // result, scratch...]` folds the operand-0 `Use`; the operand-1
        // `Use` survives into the `CopyI64` row's `Use` position because
        // saturating addition commutes under every carrier — `0 +| x` is
        // `x +| 0` is `x` inside the carrier's bounds — and drops the
        // scratch `Def` tail under the same occurrence-free custody.
        (SourceShape::SaturatingAddZeroLeftScratch, [victim, right, result, scratch @ ..]) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != candidate.victim
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || right.class != row.operands[0].class
                || result.class != row.operands[1].class
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
        // The saturating-add upper-bound grammar: `[dropped, victim,
        // result, scratch...]` folds the operand-1 `Use` — the
        // carrier-maximum literal — and drops the operand-0 `Use` because
        // the constant result never reads it: `x +| MAX` is `MAX` for
        // every `x` an unsigned carrier admits. The rewritten row is the
        // `MaterializeI64` constant row — a lone `Def` binding the
        // consumer's result register — and every operand past the
        // operand-2 `Def` result is a scratch `Def` the fold drops under
        // the occurrence-free custody the validator independently
        // re-derives: each dropped `Def` register must occur nowhere else
        // in the function, because the fold discards a definition a
        // surviving read or second definition would still observe. The
        // u64 carrier's row is exactly three long — the scratch slice is
        // empty for it — while every clamped carrier's row continues
        // with the bound scratch its realization writes.
        (SourceShape::SaturatingAddUpperBound, [left, right, result, scratch @ ..]) => {
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
        // The commuted saturating-add upper-bound grammar: `[victim,
        // dropped, result, scratch...]` folds the operand-0 `Use` — the
        // carrier-maximum literal — and drops the operand-1 `Use` under
        // the same constant-result custody. Saturating addition commutes,
        // so `MAX +| x` is `x +| MAX` is `MAX`, but the grammar needs
        // none of that: the operand-0 literal alone fixes the result. The
        // scratch `Def` tail drops under the same occurrence-free
        // custody.
        (SourceShape::SaturatingAddUpperBoundLeft, [victim, right, result, scratch @ ..]) => {
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
        // The saturating-subtract identity grammar: `[surviving, victim,
        // result]` folds the operand-1 `Use`; the operand-0 `Use`
        // survives and binds the `CopyI64` row's `Use` position. The
        // unsigned-carrier consumer carries exactly three operands — an
        // operand past the `Def` result has no droppable role under this
        // grammar — and no left-literal counterpart exists: `0 -| x` is
        // not `x`.
        (SourceShape::SaturatingSubtractZero, [left, right, result]) => {
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
        // The signed-carrier saturating-subtract grammar: `[surviving,
        // victim, result, scratch...]` folds the operand-1 `Use`; the
        // operand-0 `Use` survives and binds the `CopyI64` row's `Use`
        // position, and every operand past the operand-2 `Def` result is
        // a scratch `Def` — the bound output a clamped realization
        // computes its saturation bound through. The validator
        // independently re-derives the dropped-operand custody: each
        // dropped `Def` register must occur nowhere else in the function,
        // because the fold discards a definition a surviving read or
        // second definition would still observe.
        (SourceShape::SaturatingSubtractZeroScratch, [left, right, result, scratch @ ..]) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
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
        // The saturating-subtract zero-minuend grammar: `[victim,
        // subtrahend, result, scratch...]` folds the operand-0 `Use` —
        // the zero minuend — and drops the operand-1 subtrahend `Use`
        // because the constant result never reads it: `0 -| x` is `0`
        // for every `x` an unsigned carrier admits. The rewritten row is
        // the `MaterializeI64` constant row — a lone `Def` binding the
        // consumer's result register — and every operand past the
        // operand-2 `Def` result is a scratch `Def` the fold drops under
        // the occurrence-free custody the validator independently
        // re-derives: each dropped `Def` register must occur nowhere else
        // in the function, because the fold discards a definition a
        // surviving read or second definition would still observe. Only
        // unsigned carriers reach this shape — the kind dispatch already
        // routed a signed carrier's operand-0 literal to the
        // right-literal family, where it rejects as a future-use
        // mismatch — and the unsigned row's operand list is exactly three
        // long, so the scratch slice is empty in practice.
        (SourceShape::SaturatingSubtractZeroMinuend, [victim, right, result, scratch @ ..]) => {
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
        // The saturating-subtract upper-bound grammar: `[dropped, victim,
        // result, scratch...]` folds the operand-1 `Use` — the
        // carrier-maximum subtrahend — and drops the operand-0 minuend
        // `Use` under the constant-result custody: `x -| MAX` is `0` for
        // every `x` an unsigned carrier admits, so the constant result
        // never reads the minuend. The rewritten row is the
        // `MaterializeI64` constant row — a lone `Def` binding the
        // consumer's result register — and every operand past the
        // operand-2 `Def` result is a scratch `Def` the fold drops under
        // the occurrence-free custody the validator independently
        // re-derives: each dropped `Def` register must occur nowhere else
        // in the function, because the fold discards a definition a
        // surviving read or second definition would still observe. Only
        // unsigned carriers reach this shape — the kind dispatch already
        // routed a signed carrier's maximum subtrahend literal to the
        // right-zero family, where it rejects as an unsupported immediate
        // — and the unsigned row's operand list is exactly three long, so
        // the scratch slice is empty in practice.
        (
            SourceShape::SaturatingSubtractUpperBoundSubtrahend,
            [left, victim, result, scratch @ ..],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != candidate.victim
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
        // The saturating-divide identity grammar: `[surviving, victim,
        // result, tail...]` folds the operand-1 `Use`; the operand-0 `Use`
        // survives and binds the `CopyI64` row's `Use` position. Operands
        // past the operand-2 `Def` result admit the mixed custody no other
        // grammar carries. The validator independently re-derives each
        // tail operand's custody: a tail `Use` — the zeroed high-half
        // input an x86-64 `div` realization reads — must be defined in
        // this function only by `MaterializeI64` instructions producing
        // `Unsigned(0)`, because the fold discards whatever the operand
        // carried; a tail `Def` — the bound scratch an aarch64 signed
        // realization writes — must occur nowhere else in the function,
        // because the fold discards a definition a surviving read or
        // second definition would still observe. Any other access — a
        // `UseDef` an in-place constraint would carry — names no droppable
        // role and rejects.
        (SourceShape::SaturatingDivideOne, [left, right, result, tail @ ..]) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != candidate.victim
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
                || !tail.iter().all(|operand| match operand.access {
                    RegisterOperandAccess::Use => {
                        dropped_use_defined_zero(function, operand.virtual_register)
                    }
                    RegisterOperandAccess::Def => {
                        dropped_def_is_dead(function, operand.virtual_register)
                    }
                    _ => false,
                })
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        // The saturating-divide zero-dividend grammar: `[victim, divisor,
        // result, tail...]` folds the operand-0 `Use` — the zero
        // dividend — drops the operand-1 divisor `Use` because the
        // constant result never reads it, and drops every operand past
        // the operand-2 `Def` result under the same mixed custody the
        // divisor-one grammar derives: a tail `Use` — the zeroed
        // high-half input an x86-64 `div`/`idiv` realization reads — must
        // be defined in this function only by `MaterializeI64`
        // instructions producing `Unsigned(0)`, because a literal of zero
        // at operand 0 fixes only the low dividend half; a tail `Def` —
        // the bound scratch an aarch64 clamped signed-divide realization
        // writes — must occur nowhere else in the function, because the
        // fold discards a definition a surviving read or second
        // definition would still observe. Any other access — a `UseDef`
        // an in-place constraint would carry — names no droppable role
        // and rejects.
        (SourceShape::SaturatingDivideZeroDividend, [victim, right, result, tail @ ..]) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != candidate.victim
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
                || !tail.iter().all(|operand| match operand.access {
                    RegisterOperandAccess::Use => {
                        dropped_use_defined_zero(function, operand.virtual_register)
                    }
                    RegisterOperandAccess::Def => {
                        dropped_def_is_dead(function, operand.virtual_register)
                    }
                    _ => false,
                })
            {
                return Err(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                });
            }
            Some(result.virtual_register)
        }
        // The and-ones identity grammar: `[surviving, victim, result]`
        // folds the operand-1 `Use`; the operand-0 `Use` survives and
        // binds the `CopyI64` row's `Use` position. The consumer carries
        // exactly three operands — an operand past the `Def` result has
        // no droppable role under this grammar.
        (SourceShape::AndOnes, [left, right, result]) => {
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
        // The commuted and-ones identity grammar: `[victim, surviving,
        // result]` folds the operand-0 `Use`; the operand-1 `Use`
        // survives into the `CopyI64` row's `Use` position because
        // bitwise and commutes — `MAX & x` is `x & MAX` is `x`.
        (SourceShape::AndOnesLeft, [victim, right, result]) => {
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
    // declared unit-effect surface. Under the divide grammars a
    // `fixed_view` pin is deliberately dropped with the pinned operand
    // form — a surviving operand keeps its other uses' own constraints —
    // while `tied_to` has no carried meaning once the operand list is
    // rebuilt and still rejects under every grammar. Under the
    // constant-result grammar an `early_clobber` mark drops with its
    // operand for the same reason: the write-before-read hazard it names
    // exists only inside the folded operand list. The saturating-add and
    // saturating-subtract grammars drop both marks — the x86-64 rows
    // declare their result and bound scratch `early_clobber` — because
    // both constrain only the operand list the rebuild replaces.
    let drops_fixed_views = matches!(
        shape,
        SourceShape::DivideIdentity
            | SourceShape::DivideZeroDividend
            | SourceShape::RemainderIdentity
            | SourceShape::RemainderMinusOne
            | SourceShape::RemainderZeroDividend
            | SourceShape::SaturatingAddZero
            | SourceShape::SaturatingAddZeroLeft
            | SourceShape::SaturatingAddZeroScratch
            | SourceShape::SaturatingAddZeroLeftScratch
            | SourceShape::SaturatingAddUpperBound
            | SourceShape::SaturatingAddUpperBoundLeft
            | SourceShape::SaturatingSubtractZero
            | SourceShape::SaturatingSubtractZeroScratch
            | SourceShape::SaturatingSubtractZeroMinuend
            | SourceShape::SaturatingSubtractUpperBoundSubtrahend
            | SourceShape::SaturatingDivideOne
            | SourceShape::SaturatingDivideZeroDividend
    );
    let drops_early_clobbers = matches!(
        shape,
        SourceShape::RemainderIdentity
            | SourceShape::RemainderMinusOne
            | SourceShape::RemainderZeroDividend
            | SourceShape::SaturatingAddZero
            | SourceShape::SaturatingAddZeroLeft
            | SourceShape::SaturatingAddZeroScratch
            | SourceShape::SaturatingAddZeroLeftScratch
            | SourceShape::SaturatingAddUpperBound
            | SourceShape::SaturatingAddUpperBoundLeft
            | SourceShape::SaturatingSubtractZero
            | SourceShape::SaturatingSubtractZeroScratch
            | SourceShape::SaturatingSubtractZeroMinuend
            | SourceShape::SaturatingSubtractUpperBoundSubtrahend
            | SourceShape::SaturatingDivideOne
            | SourceShape::SaturatingDivideZeroDividend
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
        // two grammars discharge that surface by different evidence the
        // validator re-derives separately. The divisor-one fold's literal
        // is itself the discharging value. The zero-dividend fold relies
        // on the nonzero-divisor obligation the consumer kind carries —
        // which must appear in the instruction's recorded provenance
        // obligations, because the folded dividend of zero does not by
        // itself discharge the divide-by-zero fault.
        SelectedInstructionKind::ExactDivideU64 { obligation, .. } => match shape {
            SourceShape::DivideZeroDividend => obligation_discharged_fold_admission(
                consumer_declaration,
                rewritten_declaration,
                consumer.provenance.obligations.contains(&obligation),
            ),
            _ => fault_discharged_fold_admission(consumer_declaration, rewritten_declaration),
        },
        // The remainder's encoded alternatives may architecturally fault;
        // the three grammars discharge that surface by different evidence
        // the validator re-derives separately. The divisor-one fold's
        // literal is itself the discharging value — a divide by one can
        // neither divide by zero nor overflow — and the minus-one fold's
        // all-ones literal discharges it the same way: a divisor of `-1`
        // can never divide by zero, and the one dividend whose `idiv`
        // would overflow is the exceptional case the kind's semantics
        // defines to produce zero rather than trap. The zero-dividend
        // fold relies on the nonzero-divisor obligation the consumer kind
        // carries — which must appear in the instruction's recorded
        // provenance obligations, because the folded dividend of zero
        // does not by itself discharge the divide-by-zero fault.
        SelectedInstructionKind::WrappingRemainderI64 { obligation, .. } => match shape {
            SourceShape::RemainderZeroDividend => obligation_discharged_fold_admission(
                consumer_declaration,
                rewritten_declaration,
                consumer.provenance.obligations.contains(&obligation),
            ),
            _ => fault_discharged_fold_admission(consumer_declaration, rewritten_declaration),
        },
        // The saturating add's and saturating subtract's implicit unit
        // *definitions* retire with the folded form — the aarch64 rows
        // write `nzcv`, which the isolated `CopyI64` does not carry — so
        // the declaration-level relationship cannot alone admit the fold:
        // the validator independently re-derives the record-level gate
        // that every unit the consumer record defines is dead in the
        // function — no instruction or terminator implicitly uses it —
        // and that the record declares no implicit uses at all. The gate
        // is carrier-agnostic: every carrier's aarch64 realization is
        // flag-setting.
        SelectedInstructionKind::SaturatingAdd { .. }
        | SelectedInstructionKind::SaturatingSubtract { .. } => {
            dead_unit_defs_fold_admission(consumer_declaration, rewritten_declaration)
                && dropped_unit_defs_dead(function, consumer)
        }
        // The saturating divide's encoded alternatives may architecturally
        // fault *and* may declare implicit unit definitions — the aarch64
        // signed rows write `nzcv` — so either family's admission combines
        // gates the validator re-derives separately. The divisor-one
        // fold's literal of one is itself the evidence the divide-by-zero
        // fault cannot fire. The zero-dividend fold relies on the
        // nonzero-divisor obligation the consumer kind carries — which
        // must appear in the instruction's recorded provenance
        // obligations, because the folded dividend of zero does not by
        // itself discharge the divide-by-zero fault. Under either family
        // every implicit unit the consumer record defines must be dead in
        // the function before the rewritten form retires the definition.
        // The record-level deadness scan runs alongside the
        // declaration-level relationship like the saturating-add gate.
        SelectedInstructionKind::SaturatingDivide { obligation, .. } => match shape {
            SourceShape::SaturatingDivideZeroDividend => {
                obligation_discharged_dead_unit_defs_fold_admission(
                    consumer_declaration,
                    rewritten_declaration,
                    consumer.provenance.obligations.contains(&obligation),
                ) && dropped_unit_defs_dead(function, consumer)
            }
            _ => {
                fault_discharged_dead_unit_defs_fold_admission(
                    consumer_declaration,
                    rewritten_declaration,
                ) && dropped_unit_defs_dead(function, consumer)
            }
        },
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

    // The operand-swapped compare grammar's record-level gate, re-derived
    // on the concrete instruction record rather than read from any
    // producer descriptor: the rewrite keeps the consumer's implicit unit
    // definitions — the rewritten row publishes the identical ones, which
    // the operand-shape and declaration checks above required — but
    // reverses the comparison's operand order, so the fold is admitted
    // only while every reader each defined unit reaches through the CFG
    // is equality-sensing: the boolean-equal materialization and the
    // generic conditional branch. Ordering predicates observe the
    // inverted relation and refuse. The validator re-walks the flow
    // itself: the unit resumes at each live successor's head and ends at
    // any implicit definition or clobber. The right-literal grammar
    // replaces the operand in place and needs no reader audit.
    if shape == SourceShape::CompareLeftImmediate
        && !swapped_condition_flow_admitted(function, block_index, literal_index + 1, consumer)
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
    // operand 1 under the left grammars.
    let surviving = match shape {
        SourceShape::BinaryLeftImmediate
        | SourceShape::CompareLeftImmediate
        | SourceShape::AndZeroLeft
        | SourceShape::XorZeroLeft
        | SourceShape::WrappingAddZeroLeft
        | SourceShape::SaturatingAddZeroLeft
        | SourceShape::SaturatingAddZeroLeftScratch
        | SourceShape::SaturatingAddUpperBoundLeft
        | SourceShape::AndOnesLeft
        | SourceShape::RemainderZeroDividend
        | SourceShape::DivideZeroDividend
        | SourceShape::SaturatingSubtractZeroMinuend
        | SourceShape::SaturatingDivideZeroDividend => consumer.operands[1].virtual_register,
        SourceShape::BinaryImmediate
        | SourceShape::UnaryExtension
        | SourceShape::UnaryCopy
        | SourceShape::DivideIdentity
        | SourceShape::RemainderIdentity
        | SourceShape::RemainderMinusOne
        | SourceShape::AndZero
        | SourceShape::XorZero
        | SourceShape::WrappingAddZero
        | SourceShape::SaturatingAddZero
        | SourceShape::SaturatingAddZeroScratch
        | SourceShape::SaturatingAddUpperBound
        | SourceShape::SaturatingSubtractZero
        | SourceShape::SaturatingSubtractZeroScratch
        | SourceShape::SaturatingSubtractUpperBoundSubtrahend
        | SourceShape::SaturatingDivideOne
        | SourceShape::AndOnes => consumer.operands[0].virtual_register,
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
/// the left `Use` operand — the exact-add commuted grammar, and the
/// `ByteViewAddress` operand-0 backing literal the modular address
/// computation admits under operand exchange — the unary extension and
/// copy forms whose
/// literal is the sole operand, the divide-identity form whose
/// operand-1 divisor literal of one folds into a copy of the dividend and
/// drops every `Use` operand past the operand-2 `Def` result, or the
/// divide zero-dividend form whose operand-0 dividend literal of zero
/// folds into a materialized zero under the consumer's carried
/// nonzero-divisor obligation and drops the operand-1 `Use` and the same
/// provably-zero auxiliary `Use` operands, or the remainder-identity form
/// whose operand-1 divisor literal of one folds
/// into a materialized zero and drops the operand-0 `Use` and every `Def`
/// operand past the operand-2 `Def` result, or the remainder minus-one
/// form whose operand-1 divisor literal of `u64::MAX` — the
/// normalized-i64 divisor `-1`, whose remainder is always zero including
/// the `i64::MIN` dividend the kind defines to produce zero rather than
/// trap — folds into a materialized zero under the same grammar, or the
/// remainder
/// zero-dividend form whose operand-0 dividend literal of zero folds into
/// a materialized zero under the consumer's carried nonzero-divisor
/// obligation and drops the operand-1 `Use` and the same dead scratch
/// `Def`s, or the bitwise-and
/// annihilator forms whose zero literal folds `BitwiseAndI64` into a
/// materialized zero — at the operand-1 `Use`, or at the operand-0 `Use`
/// under the left grammar that drops the operand-1 `Use` instead — or the
/// bitwise-xor identity forms whose zero literal folds `BitwiseXorI64`
/// into a copy of the surviving `Use` — at the operand-1 `Use`, or at
/// the operand-0 `Use` under the commuted left grammar that binds the
/// operand-1 `Use` instead — or the wrapping-add identity forms whose
/// zero literal folds `WrappingAddI64` into a copy of the surviving
/// `Use` under the same two-position grammar — or the bitwise-and
/// identity forms whose all-ones literal folds `BitwiseAndI64` into a
/// copy of the surviving `Use` under the same two-position grammar — or
/// the saturating-add identity forms whose zero literal folds a
/// `SaturatingAdd` into a copy of the surviving `Use` under the same
/// two-position grammar, retiring the consumer's implicit unit
/// definitions under a whole-function deadness gate — on the u64 carrier
/// under the exact three-operand grammar, and on every clamped carrier
/// under the scratch-defs grammar that drops each `Def` operand past the
/// result under occurrence-free custody — or the saturating-add
/// upper-bound forms whose carrier-maximum literal folds an unsigned
/// `SaturatingAdd` into a materialized maximum — `x +| MAX` and
/// `MAX +| x` are both `MAX` for every `x` an unsigned carrier admits,
/// because `x + MAX` reaches the carrier's upper bound and saturates to
/// it — dropping the other `Use` the constant result never reads and
/// every scratch `Def` operand past the result under the same
/// occurrence-free custody, and whose family admits no signed carrier:
/// `x +| MAX` there is `x + MAX` unclamped for every negative `x`, not a
/// constant — or the saturating-subtract
/// identity forms whose zero literal folds a `SaturatingSubtract` into a
/// copy of the operand-0 `Use` under the right-literal grammar —
/// saturating subtraction does not commute, so `0 -| x` names no admitted
/// identity shape — retiring the same dead unit definitions, on the
/// unsigned carriers under the exact three-operand grammar and on the
/// signed carriers under the scratch-defs grammar — or the
/// saturating-subtract zero-minuend form whose operand-0 minuend literal
/// of zero folds an unsigned `SaturatingSubtract` into a materialized
/// zero — `0 -| x` is `0` for every `x` an unsigned carrier admits,
/// because `0 - x` underflows the carrier's lower bound and saturates to
/// it — dropping the operand-1 subtrahend `Use` the constant result
/// never reads, and whose family admits no signed carrier: `0 -| x`
/// there is `-x` clamped to the carrier's bounds, not a constant — or
/// the saturating-divide
/// identity form whose operand-1 divisor literal of one folds a
/// `SaturatingDivide` into a copy of the operand-0 `Use` under the
/// right-literal grammar alone — `1 /| x` is not `x` — whose literal is
/// itself the evidence the encoded fault surface cannot fire, whose
/// implicit unit definitions retire under the same whole-function
/// deadness gate, and whose operands past the `Def` result admit the
/// mixed custody of provably-zero auxiliary `Use`s and occurrence-free
/// scratch `Def`s — or the saturating-divide zero-dividend form whose
/// operand-0 dividend literal of zero folds a `SaturatingDivide` into a
/// materialized zero under the consumer's carried nonzero-divisor
/// obligation, dropping the operand-1 divisor `Use` and every tail
/// operand under the same mixed custody while the consumer's implicit
/// unit definitions retire under the same deadness gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceShape {
    BinaryImmediate,
    BinaryLeftImmediate,
    /// The left-literal compare grammar: `literal - x` rewrites into the
    /// operand-swapped `x - literal` — the `CompareI64Immediate` row is
    /// the same one the right-literal compare binds, and the
    /// preservation of the compare's implicit condition-state
    /// definitions under the reversed subtraction is the flow-sensitive
    /// fact the replay audits independently.
    CompareLeftImmediate,
    UnaryExtension,
    UnaryCopy,
    DivideIdentity,
    DivideZeroDividend,
    RemainderIdentity,
    RemainderMinusOne,
    RemainderZeroDividend,
    AndZero,
    AndZeroLeft,
    XorZero,
    XorZeroLeft,
    WrappingAddZero,
    WrappingAddZeroLeft,
    AndOnes,
    AndOnesLeft,
    SaturatingAddZero,
    SaturatingAddZeroLeft,
    SaturatingAddZeroScratch,
    SaturatingAddZeroLeftScratch,
    SaturatingAddUpperBound,
    SaturatingAddUpperBoundLeft,
    SaturatingSubtractZero,
    SaturatingSubtractZeroScratch,
    SaturatingSubtractZeroMinuend,
    SaturatingSubtractUpperBoundSubtrahend,
    SaturatingDivideOne,
    SaturatingDivideZeroDividend,
}

impl SourceShape {
    const fn victim_operand(self) -> u16 {
        match self {
            Self::BinaryImmediate
            | Self::DivideIdentity
            | Self::RemainderIdentity
            | Self::RemainderMinusOne
            | Self::AndZero
            | Self::XorZero
            | Self::WrappingAddZero
            | Self::AndOnes
            | Self::SaturatingAddZero
            | Self::SaturatingAddZeroScratch
            | Self::SaturatingAddUpperBound
            | Self::SaturatingSubtractZero
            | Self::SaturatingSubtractZeroScratch
            | Self::SaturatingSubtractUpperBoundSubtrahend
            | Self::SaturatingDivideOne => 1,
            Self::BinaryLeftImmediate
            | Self::CompareLeftImmediate
            | Self::UnaryExtension
            | Self::UnaryCopy
            | Self::DivideZeroDividend
            | Self::RemainderZeroDividend
            | Self::AndZeroLeft
            | Self::XorZeroLeft
            | Self::WrappingAddZeroLeft
            | Self::AndOnesLeft
            | Self::SaturatingAddZeroLeft
            | Self::SaturatingAddZeroLeftScratch
            | Self::SaturatingAddUpperBoundLeft
            | Self::SaturatingSubtractZeroMinuend
            | Self::SaturatingDivideZeroDividend => 0,
        }
    }
}

/// Whether the saturating-add consumer's implicit unit definitions are all
/// dead in `function` — the validator's independent re-derivation of the
/// deadness gate the dead-unit-defs fold requires. The consumer record must
/// declare no implicit unit *uses* — one the rewritten form does not carry
/// would be unit state the rewrite silently stops observing — and every unit
/// it *defines* must be implicitly used by no instruction or terminator in
/// the function: a reader of a retired definition would observe a stale
/// unit once the defining instruction disappears. A use textually before
/// the definition still counts — it reads the unit on a later loop
/// iteration — so the whole-function scan is the only sound order.
fn dropped_unit_defs_dead(function: &SelectedFunction, consumer: &SelectedInstruction) -> bool {
    consumer.implicit_uses.is_empty()
        && consumer
            .implicit_defs
            .iter()
            .all(|unit| !implicit_unit_used(function, *unit))
}

/// The validator's independent re-derivation of the operand-swapped
/// compare's record-level gate. The consumer record must declare no
/// implicit unit *uses* — one the rewritten form does not carry would be
/// unit state the rewrite silently stops observing — and the rewrite's
/// preserved definitions are admitted only while every reader each
/// defined unit can reach through the CFG reads the zero condition
/// alone: `x - literal` inverts `literal - x`'s ordering predicates
/// while keeping its zero condition, so an ordering-sensitive reader of
/// a kept definition would observe the reversed relation. When both
/// operands name the same register the subtraction is itself unchanged —
/// `v - v` survives as `v - v` — so no reader can observe the rewrite.
fn swapped_condition_flow_admitted(
    function: &SelectedFunction,
    block_index: usize,
    consumer_index: usize,
    consumer: &SelectedInstruction,
) -> bool {
    let [left, right, ..] = consumer.operands.as_slice() else {
        return false;
    };
    consumer.implicit_uses.is_empty()
        && (left.virtual_register == right.virtual_register
            || consumer.implicit_defs.iter().all(|unit| {
                swapped_unit_reaches_only_equality_readers(
                    function,
                    block_index,
                    consumer_index,
                    *unit,
                )
            }))
}

/// Walk `unit` forward through the CFG from the instruction after the
/// operand-swapped consumer inside `function.blocks[block_index]` —
/// `consumer_index` positions the consumer in its block's instruction
/// list — requiring every reached implicit reader to be equality-sensing
/// and stopping where an implicit definition or clobber ends the live
/// range this definition feeds. A unit still live at a terminator
/// resumes at the head of every successor block; an edge naming a block
/// the function does not contain leaves the unit's reachability
/// unprovable and refuses. The `(block, start)` worklist keys bound the
/// walk: a back edge delivering the unit to the consumer's own block
/// re-enters it at position 0, where the consumer's own definition ends
/// the range.
fn swapped_unit_reaches_only_equality_readers(
    function: &SelectedFunction,
    block_index: usize,
    consumer_index: usize,
    unit: RegisterUnitId,
) -> bool {
    let mut visited = std::collections::BTreeSet::new();
    let mut pending = vec![(block_index, consumer_index + 1)];
    while let Some((index, start)) = pending.pop() {
        if !visited.insert((index, start)) {
            continue;
        }
        let block = &function.blocks[index];
        let mut ended = false;
        for position in start..=block.instructions.len() {
            let instruction = match block.instructions.get(position) {
                Some(instruction) => instruction,
                None => terminator_instruction(&block.terminator),
            };
            if instruction.implicit_uses.contains(&unit) && !equality_only_reader(instruction.kind)
            {
                return false;
            }
            if instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit) {
                ended = true;
                break;
            }
        }
        if ended {
            continue;
        }
        for successor in terminator_successors(&block.terminator) {
            let Some(target) = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == successor.block)
            else {
                return false;
            };
            pending.push((target, 0));
        }
    }
    true
}

/// Whether `kind` observes only the zero condition of the flag state —
/// the predicate the operand-swapped subtraction preserves. The
/// boolean-equal materialization and the generic conditional branch are
/// the only equality consumers in the selected catalog; the ordering
/// materializations and predicate-aware terminators observe the inverted
/// relation, and any other implicit reader is conservatively refused.
fn equality_only_reader(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::ConditionalBranchNonZero
    )
}

/// Whether any instruction or terminator in `function` implicitly uses
/// `unit` — the observation channel a removed definition would leave stale.
/// A terminator's uses include the function's live-out unit state, so the
/// scan walks terminator instruction records alongside the ordinary ones.
fn implicit_unit_used(function: &SelectedFunction, unit: RegisterUnitId) -> bool {
    function.blocks.iter().any(|block| {
        block
            .instructions
            .iter()
            .chain(match &block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                | SelectedTerminator::Jump { instruction, .. }
                | SelectedTerminator::Return { instruction, .. }
                | SelectedTerminator::HostedExitProcess { instruction, .. } => {
                    std::iter::once(instruction)
                }
            })
            .any(|instruction| instruction.implicit_uses.contains(&unit))
    })
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
        | SelectedInstructionKind::WrappingRemainderI64 { .. } => {
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
            // The copy fold binds its own policy-gated row, each remainder
            // fold binds the materialize row under its own policy bit, and
            // the extension consumers bind theirs. The three remainder
            // grammars share the row but not the gate: the victim
            // register's operand position names whether the zero-dividend
            // gate applied — operand 0 — and at operand 1 the removed
            // literal's value names which divisor family's gate applied —
            // the all-ones literal is the minus-one fold, every other
            // admitted literal the divisor-one fold.
            let row = if consumer.kind == SelectedInstructionKind::CopyI64 {
                rows.copy
            } else if matches!(
                consumer.kind,
                SelectedInstructionKind::WrappingRemainderI64 { .. }
            ) {
                let victim_position = consumer
                    .operands
                    .iter()
                    .position(|operand| {
                        operand.access == RegisterOperandAccess::Use
                            && operand.virtual_register == action.victim
                    })
                    .ok_or(LiteralFoldError::ConsumerMismatch {
                        function: function_index,
                    })?;
                if victim_position == 0 {
                    rows.remainder_zero
                } else if matches!(
                    literal.kind,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(bits),
                    } if bits == u128::from(u64::MAX)
                ) {
                    rows.remainder_minus_one
                } else {
                    rows.remainder
                }
            } else {
                rows.materialize
            };
            (row, SelectedInstructionKind::MaterializeI64 { value })
        }
        // A bitwise-and folds under one of two disjoint families the
        // reconstructed immediate names: the and-zero annihilator records
        // the constant zero its rewritten `MaterializeI64` embeds, bound
        // to the `MaterializeI64` row the and-zero policy gate selected,
        // while the and-ones identity records the all-ones literal itself
        // and rebuilds a `CopyI64` of the surviving operand, bound to the
        // `CopyI64` row the and-ones policy gate selected. Any other
        // recorded immediate is a fold the grammar derivation already
        // rejected; keeping the and-zero arm here preserves the
        // consumer-mismatch refusal.
        SelectedInstructionKind::BitwiseAndI64 => {
            if action.immediate == u64::MAX {
                (rows.and_ones, SelectedInstructionKind::CopyI64)
            } else {
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
                (
                    rows.and_zero,
                    SelectedInstructionKind::MaterializeI64 { value },
                )
            }
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
        // Two disjoint families fold `ExactDivideU64`; the victim
        // register's operand position names the family a fold belongs
        // to — operand 0 is the zero-dividend fold. The divisor-one fold
        // rebuilds a `CopyI64` of the surviving operand bound to the
        // `CopyI64` row the divide policy gate selected. The
        // zero-dividend fold rebuilds a `MaterializeI64` of the constant
        // the action payload records, recomputed against the surviving
        // result register's scalar type and bound to the `MaterializeI64`
        // row the zero-dividend policy gate selected.
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            let victim_position = consumer
                .operands
                .iter()
                .position(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && operand.virtual_register == action.victim
                })
                .ok_or(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                })?;
            if victim_position == 0 {
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
                (
                    rows.divide_zero,
                    SelectedInstructionKind::MaterializeI64 { value },
                )
            } else {
                (rows.divide, SelectedInstructionKind::CopyI64)
            }
        }
        // An exclusive-or with a zero literal is the surviving operand:
        // the validator rebuilds the consumer as a `CopyI64` bound to the
        // `CopyI64` row the xor-zero policy gate selected.
        SelectedInstructionKind::BitwiseXorI64 => (rows.xor_zero, SelectedInstructionKind::CopyI64),
        // A wrapping add with a zero literal is the surviving operand:
        // the validator rebuilds the consumer as a `CopyI64` bound to the
        // `CopyI64` row the wrapping-add-zero policy gate selected.
        SelectedInstructionKind::WrappingAddI64 => {
            (rows.wrapping_add_zero, SelectedInstructionKind::CopyI64)
        }
        // Two disjoint families fold `SaturatingAdd` at either `Use`
        // position; the reconstructed immediate names the family a fold
        // belongs to. The zero-identity fold records the folded literal
        // itself — zero — and rebuilds a `CopyI64` of the surviving
        // operand bound to the `CopyI64` row the saturating-add-zero
        // policy gate selected. The upper-bound fold records the constant
        // its rewritten `MaterializeI64` embeds — the carrier's maximum,
        // which an unsigned carrier's maximum literal is — and rebuilds
        // that constant recomputed against the surviving result
        // register's scalar type, bound to the `MaterializeI64` row the
        // upper-bound policy gate selected. A signed carrier's
        // reconstructed immediate can never be its maximum — the grammar
        // derivation already rejected it — so the signedness check keeps
        // the consumer-mismatch refusal for any recorded action the
        // derivation could not produce. Either rebuild replaces the
        // operand list — including the clamped rows' dropped
        // bound-scratch `Def` — and the unit surface wholesale from the
        // bound row, so the retired implicit definitions — the aarch64
        // `nzcv` write — and the retired clobbers — the x86-64 `rflags`
        // write — leave with the folded form.
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            if !carrier.is_signed() && action.immediate == carrier.maximum_bits() {
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
                (
                    rows.saturating_add_upper_bound,
                    SelectedInstructionKind::MaterializeI64 { value },
                )
            } else {
                (rows.saturating_add_zero, SelectedInstructionKind::CopyI64)
            }
        }
        // Three disjoint families fold `SaturatingSubtract`; the victim
        // register's operand position names the operand-0 family — the
        // unsigned zero-minuend fold — while the removed literal's value
        // names which of the two operand-1 subtrahend families a fold
        // belongs to — the carrier-maximum literal is the upper-bound
        // fold, every other admitted literal the right-zero fold. The
        // right-zero identity fold rebuilds a `CopyI64` of the surviving
        // operand bound to the `CopyI64` row the
        // saturating-subtract-zero policy gate selected. The
        // zero-minuend and upper-bound folds each rebuild a
        // `MaterializeI64` of the constant the action payload records,
        // recomputed against the surviving result register's scalar type
        // and bound to the `MaterializeI64` row the matching policy gate
        // selected. Either rebuild
        // replaces the operand list — including the clamped rows' dropped
        // bound-scratch `Def` — and the unit surface wholesale from the
        // bound row, so the retired implicit definitions — the aarch64
        // `nzcv` write — and the retired clobbers — the x86-64 `rflags`
        // write — leave with the folded form.
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            let victim_position = consumer
                .operands
                .iter()
                .position(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && operand.virtual_register == action.victim
                })
                .ok_or(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                })?;
            let maximum_subtrahend = matches!(
                literal.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(bits),
                } if bits == u128::from(carrier.maximum_bits())
            );
            if victim_position == 0 && !carrier.is_signed() {
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
                (
                    rows.saturating_subtract_zero_minuend,
                    SelectedInstructionKind::MaterializeI64 { value },
                )
            } else if !carrier.is_signed() && maximum_subtrahend {
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
                (
                    rows.saturating_subtract_upper_bound,
                    SelectedInstructionKind::MaterializeI64 { value },
                )
            } else {
                (
                    rows.saturating_subtract_zero,
                    SelectedInstructionKind::CopyI64,
                )
            }
        }
        // Two disjoint families fold `SaturatingDivide` on any carrier;
        // the victim register's operand position names the family a fold
        // belongs to — operand 0 is the zero-dividend fold. The
        // divisor-one fold rebuilds a `CopyI64` of the surviving operand
        // bound to the `CopyI64` row the saturating-divide-one policy
        // gate selected. The zero-dividend fold rebuilds a
        // `MaterializeI64` of the constant the action payload records,
        // recomputed against the surviving result register's scalar type
        // and bound to the `MaterializeI64` row the zero-dividend policy
        // gate selected. Either rebuild replaces the operand list —
        // including the x86-64 row's dropped zeroed high-half `Use` and
        // the aarch64 signed rows' dropped bound scratch `Def` — and the
        // unit surface wholesale from the bound row, so the retired
        // implicit definitions — the aarch64 `nzcv` write — and the
        // retired clobbers — the x86-64 `rdx`/`rflags` writes — leave
        // with the folded form.
        SelectedInstructionKind::SaturatingDivide { .. } => {
            let victim_position = consumer
                .operands
                .iter()
                .position(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && operand.virtual_register == action.victim
                })
                .ok_or(LiteralFoldError::ConsumerMismatch {
                    function: function_index,
                })?;
            if victim_position == 0 {
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
                (
                    rows.saturating_divide_zero,
                    SelectedInstructionKind::MaterializeI64 { value },
                )
            } else {
                (rows.saturating_divide_one, SelectedInstructionKind::CopyI64)
            }
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
    // the pinned operand form, the remainder-identity grammar drops the
    // pins and `early_clobber` marks a pinned-scratch realization carries,
    // and the saturating-add and saturating-subtract grammars drop both
    // marks — the x86-64 rows declare their result `early_clobber`, and
    // the clamped rows declare the bound scratch the same way; every other
    // grammar still requires undecorated operands.
    let drops_fixed_views = matches!(
        consumer.kind,
        SelectedInstructionKind::ExactDivideU64 { .. }
            | SelectedInstructionKind::WrappingRemainderI64 { .. }
            | SelectedInstructionKind::SaturatingAdd { .. }
            | SelectedInstructionKind::SaturatingSubtract { .. }
            | SelectedInstructionKind::SaturatingDivide { .. }
    );
    let drops_early_clobbers = matches!(
        consumer.kind,
        SelectedInstructionKind::WrappingRemainderI64 { .. }
            | SelectedInstructionKind::SaturatingAdd { .. }
            | SelectedInstructionKind::SaturatingSubtract { .. }
            | SelectedInstructionKind::SaturatingDivide { .. }
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

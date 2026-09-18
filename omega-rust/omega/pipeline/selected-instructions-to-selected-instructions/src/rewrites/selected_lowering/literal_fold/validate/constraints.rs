use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    TargetRegisterEnvironmentConstraintKeys, ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, ValidatedMachineEffectCatalog,
};

use crate::{LiteralFoldError, LiteralFoldPolicy};

pub(super) struct ValidationImmediateRows<'a> {
    pub(super) add: Option<&'a RegisterInstructionConstraint>,
    pub(super) subtract: Option<&'a RegisterInstructionConstraint>,
    pub(super) compare: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the unary extension folds rewrite into; bound
    /// only when the extension-elimination policy bit is selected.
    pub(super) materialize: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the copy fold rewrites into — the same
    /// constraint row the extension folds bind, gated separately so a fold
    /// the selection did not enable cannot replay under the other family's
    /// policy.
    pub(super) copy: Option<&'a RegisterInstructionConstraint>,
    /// The `Load8` row the indexed byte-load fold rewrites into; bound only
    /// when the load-indexed policy bit is selected and the environment
    /// declares the row.
    pub(super) load8: Option<&'a RegisterInstructionConstraint>,
    /// The `AddressOffset` row the byte-view address fold rewrites into;
    /// bound only when the byte-view-address policy bit is selected and the
    /// environment declares the row.
    pub(super) address_offset: Option<&'a RegisterInstructionConstraint>,
    /// The `CopyI64` row the divide-identity fold rewrites into; bound only
    /// when the exact-divide policy bit is selected.
    pub(super) divide: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the wrapping-remainder fold rewrites into —
    /// the same constraint row the unary folds bind, gated separately so a
    /// fold the selection did not enable cannot replay under another
    /// family's policy.
    pub(super) remainder: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the bitwise-and annihilator fold rewrites
    /// into — the same constraint row the unary and remainder folds bind,
    /// gated separately so a fold the selection did not enable cannot
    /// replay under another family's policy.
    pub(super) and_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `CopyI64` row the bitwise-xor identity fold rewrites into — the
    /// same constraint row the divide fold binds, gated separately so a
    /// fold the selection did not enable cannot replay under another
    /// family's policy.
    pub(super) xor_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `CopyI64` row the wrapping-add identity fold rewrites into —
    /// the same constraint row the divide and xor folds bind, gated
    /// separately so a fold the selection did not enable cannot replay
    /// under another family's policy.
    pub(super) wrapping_add_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `CopyI64` row the bitwise-and identity fold rewrites into —
    /// the same constraint row the divide, xor, and wrapping-add folds
    /// bind, gated separately so a fold the selection did not enable
    /// cannot replay under another family's policy. The and-ones family
    /// shares its consumer kind and operand positions with the and-zero
    /// family; the literal's value names which family a `BitwiseAndI64`
    /// fold belongs to.
    pub(super) and_ones: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the wrapping-remainder zero-dividend fold
    /// rewrites into — the same constraint row the unary, remainder, and
    /// and-zero folds bind, gated separately so a fold the selection did
    /// not enable cannot replay under another family's policy. The
    /// zero-dividend family shares its consumer kind with the divisor-one
    /// family; the folded literal's operand position names which family a
    /// `WrappingRemainderI64` fold belongs to.
    pub(super) remainder_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the exact-divide zero-dividend fold
    /// rewrites into — the same constraint row the unary, remainder, and
    /// and-zero folds bind, gated separately so a fold the selection did
    /// not enable cannot replay under another family's policy. The
    /// zero-dividend family shares its consumer kind with the divisor-one
    /// family; the folded literal's operand position names which family an
    /// `ExactDivideU64` fold belongs to.
    pub(super) divide_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `CopyI64` row the saturating-add identity fold rewrites into —
    /// the same constraint row the divide, xor, wrapping-add, and and-ones
    /// folds bind, gated separately so a fold the selection did not enable
    /// cannot replay under another family's policy.
    pub(super) saturating_add_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `CopyI64` row the saturating-subtract identity fold rewrites
    /// into — the same constraint row the divide, xor, wrapping-add,
    /// and-ones, and saturating-add folds bind, gated separately so a fold
    /// the selection did not enable cannot replay under another family's
    /// policy.
    pub(super) saturating_subtract_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `CopyI64` row the saturating-divide identity fold rewrites
    /// into — the same constraint row the divide, xor, wrapping-add,
    /// and-ones, saturating-add, and saturating-subtract folds bind, gated
    /// separately so a fold the selection did not enable cannot replay
    /// under another family's policy.
    pub(super) saturating_divide_one: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the saturating-divide zero-dividend fold
    /// rewrites into — the same constraint row the unary, remainder,
    /// and-zero, remainder-zero, and divide-zero folds bind, gated
    /// separately so a fold the selection did not enable cannot replay
    /// under another family's policy. The zero-dividend family shares
    /// its consumer kind with the divisor-one family; the folded
    /// literal's operand position names which family a
    /// `SaturatingDivide` fold belongs to.
    pub(super) saturating_divide_zero: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the saturating-subtract zero-minuend fold
    /// rewrites into — the same constraint row the unary, remainder,
    /// and-zero, remainder-zero, divide-zero, and saturating-divide-zero
    /// folds bind, gated separately so a fold the selection did not
    /// enable cannot replay under another family's policy. The
    /// zero-minuend family shares its consumer kind with the right-zero
    /// identity family and covers only unsigned carriers; the folded
    /// literal's operand position names which family a
    /// `SaturatingSubtract` fold belongs to.
    pub(super) saturating_subtract_zero_minuend: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the saturating-add upper-bound fold
    /// rewrites into — the same constraint row the unary, remainder,
    /// and-zero, remainder-zero, divide-zero, saturating-divide-zero, and
    /// saturating-subtract zero-minuend folds bind, gated separately so a
    /// fold the selection did not enable cannot replay under another
    /// family's policy. The upper-bound family shares its consumer kind
    /// and operand positions with the zero-identity family and covers
    /// only unsigned carriers; the folded literal's value names which
    /// family a `SaturatingAdd` fold belongs to.
    pub(super) saturating_add_upper_bound: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the wrapping-remainder minus-one fold
    /// rewrites into — the same constraint row the unary, remainder,
    /// and-zero, remainder-zero, divide-zero, saturating-divide-zero,
    /// saturating-subtract zero-minuend, and saturating-add upper-bound
    /// folds bind, gated separately so a fold the selection did not
    /// enable cannot replay under another family's policy. The minus-one
    /// family shares its consumer kind and divisor operand position with
    /// the divisor-one family; the folded literal's value names which
    /// family a `WrappingRemainderI64` operand-1 fold belongs to.
    pub(super) remainder_minus_one: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the saturating-subtract upper-bound
    /// subtrahend fold rewrites into — the same constraint row the unary,
    /// remainder, and-zero, remainder-zero, divide-zero,
    /// saturating-divide-zero, saturating-subtract zero-minuend,
    /// saturating-add upper-bound, and remainder minus-one folds bind,
    /// gated separately so a fold the selection did not enable cannot
    /// replay under another family's policy. The upper-bound family
    /// shares its consumer kind and operand position with the right-zero
    /// identity family and covers only unsigned carriers; the folded
    /// literal's value names which family a `SaturatingSubtract`
    /// operand-1 fold belongs to.
    pub(super) saturating_subtract_upper_bound: Option<&'a RegisterInstructionConstraint>,
    /// The bound machine-effect catalog the replay resolves producer,
    /// consumer, and rewritten declarations against.
    pub(super) catalog: &'a ValidatedMachineEffectCatalog,
}

pub(super) fn reconstruct_immediate_rows<'a>(
    constraints: &'a ValidatedRegisterConstraintCatalog,
    keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
    catalog: &'a ValidatedMachineEffectCatalog,
) -> Result<ValidationImmediateRows<'a>, LiteralFoldError> {
    let find = |key| {
        constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == key)
            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)
    };
    let add = policy
        .enables_exact_add()
        .then(|| find(keys.add_i64_immediate))
        .transpose()?;
    let subtract = policy
        .enables_exact_subtract()
        .then(|| find(keys.subtract_i64_immediate))
        .transpose()?;
    let compare = policy
        .enables_compare()
        .then(|| find(keys.compare_i64_immediate))
        .transpose()?;
    let materialize = policy
        .enables_extension()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let copy = policy
        .enables_copy()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let load8 = match (policy.enables_load8_indexed(), keys.load8) {
        (true, Some(key)) => Some(find(key)?),
        _ => None,
    };
    let address_offset = match (policy.enables_byte_view_address(), keys.address_offset) {
        (true, Some(key)) => Some(find(key)?),
        _ => None,
    };
    let divide = policy
        .enables_exact_divide()
        .then(|| find(keys.copy_i64))
        .transpose()?;
    let remainder = policy
        .enables_wrapping_remainder()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let and_zero = policy
        .enables_bitwise_and_zero()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let xor_zero = policy
        .enables_bitwise_xor_zero()
        .then(|| find(keys.copy_i64))
        .transpose()?;
    let wrapping_add_zero = policy
        .enables_wrapping_add_zero()
        .then(|| find(keys.copy_i64))
        .transpose()?;
    let and_ones = policy
        .enables_bitwise_and_ones()
        .then(|| find(keys.copy_i64))
        .transpose()?;
    let remainder_zero = policy
        .enables_wrapping_remainder_zero()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let divide_zero = policy
        .enables_exact_divide_zero()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let saturating_add_zero = policy
        .enables_saturating_add_zero()
        .then(|| find(keys.copy_i64))
        .transpose()?;
    let saturating_subtract_zero = policy
        .enables_saturating_subtract_zero()
        .then(|| find(keys.copy_i64))
        .transpose()?;
    let saturating_divide_one = policy
        .enables_saturating_divide_one()
        .then(|| find(keys.copy_i64))
        .transpose()?;
    let saturating_divide_zero = policy
        .enables_saturating_divide_zero()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let saturating_subtract_zero_minuend = policy
        .enables_saturating_subtract_zero_minuend()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let saturating_add_upper_bound = policy
        .enables_saturating_add_upper_bound()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let remainder_minus_one = policy
        .enables_wrapping_remainder_minus_one()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    let saturating_subtract_upper_bound = policy
        .enables_saturating_subtract_upper_bound()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    for row in [
        add,
        subtract,
        compare,
        materialize,
        copy,
        load8,
        address_offset,
        divide,
        remainder,
        and_zero,
        xor_zero,
        wrapping_add_zero,
        and_ones,
        remainder_zero,
        divide_zero,
        saturating_add_zero,
        saturating_subtract_zero,
        saturating_divide_one,
        saturating_divide_zero,
        saturating_subtract_zero_minuend,
        saturating_add_upper_bound,
        remainder_minus_one,
        saturating_subtract_upper_bound,
    ]
    .into_iter()
    .flatten()
    {
        validate_immediate_row(row)?;
    }
    // The validator derives the rewritten semantic each enabled row must
    // declare and requires the bound catalog to carry it with the surface
    // the corresponding fold requires: an isolated effect surface — no
    // memory, trap, stack, or control-flow traffic and no implicit unit
    // uses or clobbers beyond the declared result channel — or, for the
    // indexed byte-load fold, the plain pointer-read form whose
    // alternatives all read memory through a pointer operand.
    for (row, rewritten, admitted) in [
        (
            add,
            MachineSemanticKind::ExactAddI64Immediate,
            isolated_rewritten_declaration as fn(&MachineEffectDeclaration) -> bool,
        ),
        (
            subtract,
            MachineSemanticKind::ExactSubtractI64Immediate,
            isolated_rewritten_declaration,
        ),
        (
            compare,
            MachineSemanticKind::CompareI64Immediate,
            isolated_rewritten_declaration,
        ),
        (
            materialize,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            copy,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            load8,
            MachineSemanticKind::Load8,
            pointer_read_rewritten_declaration,
        ),
        (
            address_offset,
            MachineSemanticKind::AddressOffset,
            isolated_rewritten_declaration,
        ),
        (
            divide,
            MachineSemanticKind::CopyI64,
            isolated_rewritten_declaration,
        ),
        (
            remainder,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            and_zero,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            xor_zero,
            MachineSemanticKind::CopyI64,
            isolated_rewritten_declaration,
        ),
        (
            wrapping_add_zero,
            MachineSemanticKind::CopyI64,
            isolated_rewritten_declaration,
        ),
        (
            and_ones,
            MachineSemanticKind::CopyI64,
            isolated_rewritten_declaration,
        ),
        (
            remainder_zero,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            divide_zero,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            saturating_add_zero,
            MachineSemanticKind::CopyI64,
            isolated_rewritten_declaration,
        ),
        (
            saturating_subtract_zero,
            MachineSemanticKind::CopyI64,
            isolated_rewritten_declaration,
        ),
        (
            saturating_divide_one,
            MachineSemanticKind::CopyI64,
            isolated_rewritten_declaration,
        ),
        (
            saturating_divide_zero,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            saturating_subtract_zero_minuend,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            saturating_add_upper_bound,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            remainder_minus_one,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
        (
            saturating_subtract_upper_bound,
            MachineSemanticKind::MaterializeI64,
            isolated_rewritten_declaration,
        ),
    ] {
        let Some(row) = row else { continue };
        let declaration = effect_declaration(catalog, rewritten, row.key)
            .ok_or(LiteralFoldError::EffectCatalogMismatch)?;
        if !admitted(declaration) {
            return Err(LiteralFoldError::EffectCatalogMismatch);
        }
    }
    Ok(ValidationImmediateRows {
        add,
        subtract,
        compare,
        materialize,
        copy,
        load8,
        address_offset,
        divide,
        remainder,
        and_zero,
        xor_zero,
        wrapping_add_zero,
        and_ones,
        remainder_zero,
        divide_zero,
        saturating_add_zero,
        saturating_subtract_zero,
        saturating_divide_one,
        saturating_divide_zero,
        saturating_subtract_zero_minuend,
        saturating_add_upper_bound,
        remainder_minus_one,
        saturating_subtract_upper_bound,
        catalog,
    })
}

fn validate_immediate_row(row: &RegisterInstructionConstraint) -> Result<(), LiteralFoldError> {
    match row.operands.as_slice() {
        [left, result] => {
            if left.operand != 0
                || left.access != RegisterOperandAccess::Use
                || result.operand != 1
                || result.access != RegisterOperandAccess::Def
                || left.class != result.class
                || [left, result].iter().any(|operand| {
                    operand.fixed_view.is_some()
                        || operand.tied_to.is_some()
                        || operand.early_clobber
                })
                || !row.implicit_uses.is_empty()
                || !row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        // Flag-defining immediate rows admit one `Use` operand; the target
        // condition state must remain an explicit implicit definition.
        [left] if left.access == RegisterOperandAccess::Use => {
            if left.operand != 0
                || left.fixed_view.is_some()
                || left.tied_to.is_some()
                || left.early_clobber
                || !row.implicit_uses.is_empty()
                || row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        // Constant-materialization rows carry one `Def` operand and no
        // register input; no implicit unit traffic may hide in the row.
        [result] if result.access == RegisterOperandAccess::Def => {
            if result.operand != 0
                || result.fixed_view.is_some()
                || result.tied_to.is_some()
                || result.early_clobber
                || !row.implicit_uses.is_empty()
                || !row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        _ => return Err(LiteralFoldError::ImmediateConstraintMismatch),
    }
    Ok(())
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form. The
/// validator resolves declarations itself rather than trusting the
/// producer's admission.
pub(super) fn effect_declaration(
    catalog: &ValidatedMachineEffectCatalog,
    semantic: MachineSemanticKind,
    constraint: RegisterConstraintKey,
) -> Option<&MachineEffectDeclaration> {
    let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
        declaration.semantic == semantic && declaration.constraint == constraint
    });
    let declaration = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(declaration)
}

/// The non-unit declaration surface an isolated form must carry: no memory
/// access, no trap, no barrier, no call, no cleanup.
pub(super) fn isolated_effect_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}

/// The non-unit encoded surface an isolated form's alternatives must carry:
/// no memory access, unchanged stack, never traps, falls through.
pub(super) fn isolated_effect_alternative(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
}

/// The effect surface the validator requires of a rewritten form's
/// declaration: isolated outside its unit traffic, with no implicit unit
/// uses or clobbers — the implicit definitions it may carry are the result
/// channel the operand-shape check already established.
pub(super) fn isolated_rewritten_declaration(declaration: &MachineEffectDeclaration) -> bool {
    isolated_effect_declaration(declaration)
        && declaration.alternatives.iter().all(|alternative| {
            isolated_effect_alternative(alternative)
                && alternative.encoded.implicit_unit_uses.is_empty()
                && alternative.encoded.implicit_unit_clobbers.is_empty()
        })
}

/// The effect surface the validator requires of the direct-offset load the
/// indexed byte-load fold rewrites into: a pointer-read declaration whose
/// alternatives all read memory through a pointer operand and byte count,
/// fall through, leave the stack unchanged, and declare no implicit unit
/// uses or clobbers. Trap behavior is not pinned here — the per-action
/// admission binds it equal to the consumer's declared trap surface.
pub(super) fn pointer_read_rewritten_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::ReadPointerV1
        && declaration.alternatives.iter().all(|alternative| {
            matches!(
                alternative.encoded.memory,
                MachineEncodedMemoryEffect::ReadPointerV1 { .. }
            ) && alternative.encoded.stack == MachineEncodedStackEffect::UnchangedV1
                && alternative.encoded.control == MachineEncodedControlEffect::FallThroughV1
                && alternative.encoded.implicit_unit_uses.is_empty()
                && alternative.encoded.implicit_unit_clobbers.is_empty()
        })
}

/// The relationship the validator re-derives between the indexed byte load
/// and its rewritten direct-offset form: both declarations carry the same
/// pointer-read memory surface and identical trap, barrier, call, and
/// cleanup behavior, and every consumer alternative's encoded indexed read
/// — index register at `index_operand` — is matched by every rewritten
/// alternative's direct read over the same pointer operand and byte count,
/// with pairwise-identical stack, trap, and control encodings. Every
/// implicit unit the consumer defines stays defined, no side may
/// implicitly use a unit, and the rewritten form clobbers nothing.
pub(super) fn indexed_read_fold_admission(
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
    index_operand: u16,
) -> bool {
    consumer.memory == MachineMemoryEffect::ReadPointerV1
        && consumer.memory == rewritten.memory
        && consumer.trap == rewritten.trap
        && consumer.barrier == rewritten.barrier
        && consumer.call == rewritten.call
        && consumer.cleanup == rewritten.cleanup
        && consumer.alternatives.iter().all(|consumer_alternative| {
            rewritten.alternatives.iter().all(|rewritten_alternative| {
                let (
                    MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                        pointer_operand,
                        index_operand: index,
                        byte_count,
                    },
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: rewritten_pointer,
                        byte_count: rewritten_bytes,
                    },
                ) = (
                    consumer_alternative.encoded.memory,
                    rewritten_alternative.encoded.memory,
                )
                else {
                    return false;
                };
                index == index_operand
                    && pointer_operand == rewritten_pointer
                    && byte_count == rewritten_bytes
                    && consumer_alternative.encoded.stack == rewritten_alternative.encoded.stack
                    && consumer_alternative.encoded.trap == rewritten_alternative.encoded.trap
                    && consumer_alternative.encoded.control == rewritten_alternative.encoded.control
                    && consumer_alternative.encoded.implicit_unit_uses.is_empty()
                    && consumer_alternative
                        .encoded
                        .implicit_unit_defs
                        .iter()
                        .all(|unit| {
                            rewritten_alternative
                                .encoded
                                .implicit_unit_defs
                                .contains(unit)
                        })
                    && rewritten_alternative.encoded.implicit_unit_uses.is_empty()
                    && rewritten_alternative
                        .encoded
                        .implicit_unit_clobbers
                        .is_empty()
            })
        })
}

/// The relationship the validator re-derives between a fault-carrying
/// consumer — the `ExactDivideU64` form whose encoded alternatives may
/// architecturally fault — and the fully isolated copy form a divisor
/// literal of one rewrites into. The consumer declaration keeps the
/// isolated non-unit surface except its alternatives may encode
/// `NeverV1` or `MayArchitecturalFaultV1` trap behavior: the folded
/// divisor of one discharges every such fault, so the rewrite retires the
/// trap surface wholesale. The consumer may declare no implicit unit uses,
/// every implicit unit it defines must stay defined under every rewritten
/// alternative, and the rewritten declaration itself must satisfy the
/// fully isolated surface — the discharged fault may not reappear.
pub(super) fn fault_discharged_fold_admission(
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
) -> bool {
    consumer.memory == MachineMemoryEffect::NoneV1
        && matches!(
            consumer.trap,
            MachineTrapBehavior::NeverV1 | MachineTrapBehavior::MayArchitecturalFaultV1
        )
        && consumer.barrier == MachineBarrier::None
        && consumer.call == MachineCallEffect::NoneV1
        && consumer.cleanup == MachineCleanupEffect::NoneV1
        && consumer.alternatives.iter().all(|alternative| {
            let encoded = &alternative.encoded;
            encoded.memory == MachineEncodedMemoryEffect::NoneV1
                && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                && matches!(
                    encoded.trap,
                    MachineEncodedTrapBehavior::NeverV1
                        | MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                )
                && encoded.control == MachineEncodedControlEffect::FallThroughV1
                && encoded.implicit_unit_uses.is_empty()
                && encoded.implicit_unit_defs.iter().all(|unit| {
                    rewritten.alternatives.iter().all(|rewritten_alternative| {
                        rewritten_alternative
                            .encoded
                            .implicit_unit_defs
                            .contains(unit)
                    })
                })
        })
        && isolated_rewritten_declaration(rewritten)
}

/// The relationship the validator re-derives between a fault-carrying
/// consumer whose encoded fault is unreachable under its own carried
/// obligation — rather than under the folded literal — and the fully
/// isolated materialization the fold rewrites into. The declaration
/// surface is the one `fault_discharged_fold_admission` requires; the
/// distinguishing evidence is `obligation_carried`, which the caller
/// re-derives from the instruction record itself: under the
/// zero-dividend remainder and exact-divide grammars the folded dividend
/// of zero does not discharge the divide-by-zero fault — a zero dividend
/// over an unproven divisor would still fault — so the nonzero-divisor
/// obligation the `WrappingRemainderI64` or `ExactDivideU64` kind names
/// must appear in the consumer's recorded provenance obligations for the
/// rewrite to retire the trap surface.
pub(super) fn obligation_discharged_fold_admission(
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
    obligation_carried: bool,
) -> bool {
    obligation_carried && fault_discharged_fold_admission(consumer, rewritten)
}

/// The relationship the validator re-derives between a consumer whose
/// implicit unit *definitions* the rewrite retires — a saturating add on
/// any carrier whose aarch64 realization writes `nzcv` and whose x86-64
/// row clobbers `rflags` — and the fully isolated copy form a zero literal
/// rewrites into. The consumer declaration must be isolated outside its
/// unit surface with no implicit uses: a use the rewritten form does not
/// carry would be unit state the rewrite silently stops observing. Unlike
/// the isolated folds, no implicit definition needs rewritten coverage —
/// retiring those definitions is the relationship's own point, gated on
/// the record-level deadness the caller re-derives from the concrete
/// instruction and function. Clobbers retire wholesale: dropping one only
/// narrows what may be destroyed.
pub(super) fn dead_unit_defs_fold_admission(
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
) -> bool {
    isolated_effect_declaration(consumer)
        && consumer.alternatives.iter().all(|alternative| {
            isolated_effect_alternative(alternative)
                && alternative.encoded.implicit_unit_uses.is_empty()
        })
        && isolated_rewritten_declaration(rewritten)
}

/// The relationship the validator re-derives between a consumer that both
/// may architecturally fault and retires implicit unit *definitions* — a
/// `SaturatingDivide` on any carrier, whose x86-64 `div`/`idiv`
/// realizations encode `MayArchitecturalFaultV1` and whose aarch64 signed
/// realizations define `nzcv` — and the fully isolated copy form a divisor
/// literal of one rewrites into. The consumer declaration keeps the
/// isolated non-unit surface except its alternatives may encode `NeverV1`
/// or `MayArchitecturalFaultV1` trap behavior: the folded divisor of one
/// discharges every such fault — a divide by one can neither divide by
/// zero nor overflow, and the signed `MIN /| -1` clamp lies outside the
/// divisor the grammar admits — so the rewrite retires the trap surface
/// wholesale. The consumer may declare no implicit unit uses; no implicit
/// definition needs rewritten coverage, because retiring those definitions
/// is the relationship's own point, gated on the record-level deadness the
/// caller re-derives from the concrete instruction and function. Clobbers
/// retire wholesale: dropping one only narrows what may be destroyed. The
/// rewritten declaration itself must satisfy the fully isolated surface —
/// neither the discharged fault nor a retired definition may reappear.
pub(super) fn fault_discharged_dead_unit_defs_fold_admission(
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
) -> bool {
    consumer.memory == MachineMemoryEffect::NoneV1
        && matches!(
            consumer.trap,
            MachineTrapBehavior::NeverV1 | MachineTrapBehavior::MayArchitecturalFaultV1
        )
        && consumer.barrier == MachineBarrier::None
        && consumer.call == MachineCallEffect::NoneV1
        && consumer.cleanup == MachineCleanupEffect::NoneV1
        && consumer.alternatives.iter().all(|alternative| {
            let encoded = &alternative.encoded;
            encoded.memory == MachineEncodedMemoryEffect::NoneV1
                && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                && matches!(
                    encoded.trap,
                    MachineEncodedTrapBehavior::NeverV1
                        | MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                )
                && encoded.control == MachineEncodedControlEffect::FallThroughV1
                && encoded.implicit_unit_uses.is_empty()
        })
        && isolated_rewritten_declaration(rewritten)
}

/// The relationship the validator re-derives between a consumer that both
/// may architecturally fault and retires implicit unit *definitions* —
/// a `SaturatingDivide` on any carrier, whose x86-64 `div`/`idiv`
/// realizations encode `MayArchitecturalFaultV1` and whose aarch64
/// signed realizations define `nzcv` — and the fully isolated
/// materialization a dividend literal of zero rewrites into, where the
/// encoded fault is unreachable under the consumer's own carried
/// obligation rather than under the folded literal alone. The
/// declaration surface is the one
/// `fault_discharged_dead_unit_defs_fold_admission` requires; the
/// distinguishing evidence is `obligation_carried`, which the caller
/// re-derives from the instruction record itself: under the
/// zero-dividend saturating-divide grammar the folded dividend of zero
/// does not discharge the divide-by-zero fault — a zero dividend over
/// an unproven divisor would still fault — so the nonzero-divisor
/// obligation the `SaturatingDivide` kind names must appear in the
/// consumer's recorded provenance obligations for the rewrite to retire
/// the trap surface, and the record-level deadness the caller re-derives
/// from the concrete instruction and function is what admits retiring
/// every implicit unit the consumer defines.
pub(super) fn obligation_discharged_dead_unit_defs_fold_admission(
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
    obligation_carried: bool,
) -> bool {
    obligation_carried && fault_discharged_dead_unit_defs_fold_admission(consumer, rewritten)
}

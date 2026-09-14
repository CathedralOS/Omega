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
    for row in [add, subtract, compare, materialize].into_iter().flatten() {
        validate_immediate_row(row)?;
    }
    // The validator derives the rewritten semantic each enabled row must
    // declare and requires the bound catalog to carry it with an isolated
    // effect surface — no memory, trap, stack, or control-flow traffic and
    // no implicit unit uses or clobbers beyond the declared result channel.
    for (row, rewritten) in [
        (add, MachineSemanticKind::ExactAddI64Immediate),
        (subtract, MachineSemanticKind::ExactSubtractI64Immediate),
        (compare, MachineSemanticKind::CompareI64Immediate),
        (materialize, MachineSemanticKind::MaterializeI64),
    ] {
        let Some(row) = row else { continue };
        let declaration = effect_declaration(catalog, rewritten, row.key)
            .ok_or(LiteralFoldError::EffectCatalogMismatch)?;
        if !isolated_rewritten_declaration(declaration) {
            return Err(LiteralFoldError::EffectCatalogMismatch);
        }
    }
    Ok(ValidationImmediateRows {
        add,
        subtract,
        compare,
        materialize,
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

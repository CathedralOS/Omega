//! Independent replay of a projected-access fold.
//!
//! The validator never consults
//! [`ProjectedAccessRule`](super::pair::ProjectedAccessRule) or the declared
//! table: it re-derives the whole grammar from the instruction records —
//! the consumer kind and its two-operand shape, the pointer register's last
//! in-block definition as an `AddressOffset` producer, the base register's
//! freedom from intervening redefinition, the access width and the
//! combined-displacement bound, and the catalog surface the access row and
//! the isolated projection row must carry — then rebuilds the expected
//! folded record, requires the proposal to equal it, and restores the
//! complete source by content.

use super::retained_access;
use crate::rewrites::operand_probes::{effect_declaration, last_definition_before};
use crate::rewrites::sealed;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineTrapBehavior,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    ValidatedMachineEffectCatalog,
};

use super::{ProjectedAccessError, ProjectedAccessReceipt, ValidatedProjectedAccess};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: the replay re-derives the
/// projected-access fold and its reconstructed access record from the
/// source records — re-scanning the pointer's last definition, re-checking
/// the base's intervening definitions, re-summing the displacements under
/// the access-scaled bound, and re-stating the retained dereference surface
/// on the bound catalog — requires the proposed instruction to equal that
/// reconstruction, and reinserts the source instruction to restore the
/// complete source by content: every other instruction, terminator,
/// register, roster row, call, settlement, and function included. The pair
/// descriptor table is never read.
pub(crate) fn validate_projected_access_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedProjectedAccess, ProjectedAccessError> {
    let located =
        super::locate_access(source, function_index, access, environment, effect_catalog)?;
    let plan = located.plan;
    let function = located.function;
    let block_index = located.block_index;
    let position = located.position;
    let consumer = located.consumer;
    // The replay's own grammar: the consumer kind must be one of the
    // emitted displacement-carrying dereference forms — a body instruction
    // reading its referent pointer at operand 0 with the access's own tail
    // operand at 1 — and the access width the kind fixes is the scale the
    // combined displacement must satisfy.
    let (consumer_kind, tail_access, width) = match consumer.kind {
        SelectedInstructionKind::Load8 { .. } => {
            (MachineSemanticKind::Load8, RegisterOperandAccess::Def, 1)
        }
        SelectedInstructionKind::Load16 { .. } => {
            (MachineSemanticKind::Load16, RegisterOperandAccess::Def, 2)
        }
        SelectedInstructionKind::Load32 { .. } => {
            (MachineSemanticKind::Load32, RegisterOperandAccess::Def, 4)
        }
        SelectedInstructionKind::Load64 { .. } => {
            (MachineSemanticKind::Load64, RegisterOperandAccess::Def, 8)
        }
        SelectedInstructionKind::Store { byte_size, .. } => match byte_size {
            1 | 2 | 4 | 8 => (
                MachineSemanticKind::Store,
                RegisterOperandAccess::Use,
                u32::from(byte_size),
            ),
            _ => return Err(ProjectedAccessError::UnsupportedConsumer),
        },
        _ => return Err(ProjectedAccessError::UnsupportedConsumer),
    };
    let [pointer_operand, tail_operand] = consumer.operands.as_slice() else {
        return Err(ProjectedAccessError::UnsupportedConsumer);
    };
    if pointer_operand.operand != 0
        || pointer_operand.access != RegisterOperandAccess::Use
        || tail_operand.operand != 1
        || tail_operand.access != tail_access
    {
        return Err(ProjectedAccessError::UnsupportedConsumer);
    }
    // The register-only unit surface, restated from the record: no implicit
    // traffic and no operand bindings.
    if !register_only(consumer) {
        return Err(ProjectedAccessError::UnsupportedConsumer);
    }
    let pointer = pointer_operand.virtual_register;
    let find_register = |register| function.virtual_register(register);
    let pointer_register = find_register(pointer).ok_or(ProjectedAccessError::UnsupportedUse)?;
    let tail_register =
        find_register(tail_operand.virtual_register).ok_or(ProjectedAccessError::UnsupportedUse)?;
    let row = environment
        .constraint(consumer.constraint)
        .ok_or(ProjectedAccessError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != pointer_operand.class
        || pointer_operand.class != pointer_register.class
        || row.operands[1].operand != 1
        || row.operands[1].access != tail_access
        || row.operands[1].class != tail_operand.class
        || tail_operand.class != tail_register.class
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(ProjectedAccessError::ConstraintMismatch);
    }
    // The pointer's last definition before the consumer must be the
    // emitted `[use base, def pointer]` projection — re-derived from the
    // record alone, with the same register-only surface.
    let producer_index = last_definition_before(
        &function.blocks[block_index].instructions,
        position,
        pointer,
    )
    .ok_or(ProjectedAccessError::UnsupportedProducer)?;
    let producer = &function.blocks[block_index].instructions[producer_index];
    let SelectedInstructionKind::AddressOffset {
        byte_offset: producer_offset,
    } = producer.kind
    else {
        return Err(ProjectedAccessError::UnsupportedProducer);
    };
    if !register_only(producer) {
        return Err(ProjectedAccessError::UnsupportedProducer);
    }
    let [base_operand, result_operand] = producer.operands.as_slice() else {
        return Err(ProjectedAccessError::UnsupportedProducer);
    };
    if base_operand.operand != 0
        || base_operand.access != RegisterOperandAccess::Use
        || result_operand.operand != 1
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.virtual_register != pointer
    {
        return Err(ProjectedAccessError::UnsupportedProducer);
    }
    let base = base_operand.virtual_register;
    if base == pointer {
        return Err(ProjectedAccessError::UnsupportedProducer);
    }
    let base_register = find_register(base).ok_or(ProjectedAccessError::UnsupportedUse)?;
    if base_register.class != pointer_operand.class {
        return Err(ProjectedAccessError::UnsupportedUse);
    }
    let producer_row = environment
        .constraint(producer.constraint)
        .ok_or(ProjectedAccessError::ConstraintMismatch)?;
    if producer_row.operands.len() != 2
        || producer_row.operands[0].operand != 0
        || producer_row.operands[0].access != RegisterOperandAccess::Use
        || producer_row.operands[0].class != base_operand.class
        || base_operand.class != base_register.class
        || producer_row.operands[1].operand != 1
        || producer_row.operands[1].access != RegisterOperandAccess::Def
        || producer_row.operands[1].class != result_operand.class
        || result_operand.class != pointer_register.class
        || !producer_row.implicit_uses.is_empty()
        || !producer_row.implicit_defs.is_empty()
        || !producer_row.clobbers.is_empty()
    {
        return Err(ProjectedAccessError::ConstraintMismatch);
    }
    // The open interval between the projection and the access must leave
    // the base alone: the folded operand reads the base at the consumer's
    // position.
    for instruction in &function.blocks[block_index].instructions[producer_index + 1..position] {
        if instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == base
        }) {
            return Err(ProjectedAccessError::UnsupportedUse);
        }
    }
    // The displacement relationship is restated from the kind fields: the
    // consumer's `byte_offset` and the producer's combine under the scaled
    // unsigned immediate every target's access form encodes.
    let consumer_offset = match consumer.kind {
        SelectedInstructionKind::Load8 { byte_offset }
        | SelectedInstructionKind::Load16 { byte_offset }
        | SelectedInstructionKind::Load32 { byte_offset }
        | SelectedInstructionKind::Load64 { byte_offset }
        | SelectedInstructionKind::Store { byte_offset, .. } => byte_offset,
        _ => return Err(ProjectedAccessError::UnsupportedConsumer),
    };
    let combined = consumer_offset
        .checked_add(producer_offset)
        .ok_or(ProjectedAccessError::UnsupportedDisplacement)?;
    if !combined.is_multiple_of(width) || combined / width > 4095 {
        return Err(ProjectedAccessError::UnsupportedDisplacement);
    }
    let expected_kind = match consumer.kind {
        SelectedInstructionKind::Load8 { .. } => SelectedInstructionKind::Load8 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load16 { .. } => SelectedInstructionKind::Load16 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load32 { .. } => SelectedInstructionKind::Load32 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load64 { .. } => SelectedInstructionKind::Load64 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Store { byte_size, .. } => SelectedInstructionKind::Store {
            byte_offset: combined,
            byte_size,
        },
        _ => return Err(ProjectedAccessError::UnsupportedConsumer),
    };

    // The effect surface is restated on the bound catalog: the retained
    // producer is the isolated projection and the access row carries its
    // dereference — memory effect, fault surface, operand roster, unchanged
    // stack, fall-through — verbatim, because the rewritten form is the
    // same constraint row performing the identical access.
    let producer_declaration = effect_declaration(
        effect_catalog,
        MachineSemanticKind::AddressOffset,
        producer.constraint,
    )
    .ok_or(ProjectedAccessError::EffectSurfaceMismatch)?;
    let access_declaration = effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
        .ok_or(ProjectedAccessError::EffectSurfaceMismatch)?;
    if !isolated_projection(producer_declaration)
        || !retained_access(access_declaration, consumer_kind, width)
    {
        return Err(ProjectedAccessError::EffectSurfaceMismatch);
    }
    let mut expected = consumer.clone();
    expected.kind = expected_kind;
    expected.operands[0].virtual_register = base;
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(ProjectedAccessError::ReplayMismatch)?;
    if proposed_function
        .blocks
        .get(block_index)
        .and_then(|block| block.instructions.get(position))
        != Some(&expected)
    {
        return Err(ProjectedAccessError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[block_index].instructions[position] =
        consumer.clone();
    if restored != *source.selected_plan() {
        return Err(ProjectedAccessError::ReplayMismatch);
    }
    // The replay charges the same bounded walk into the budget even though
    // admission already paid it: a proposal validated on a starved budget
    // must not pass where admission would refuse.
    let block = &function.blocks[block_index];
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            total
                .checked_add(block.instructions.len())?
                .checked_add(block.instructions.len())
        })
        .ok_or(ProjectedAccessError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || ProjectedAccessError::IdentityOverflow,
        || ProjectedAccessError::WorkBudgetExceeded,
    )?;
    Ok(sealed!(
        ValidatedProjectedAccess,
        ProjectedAccessReceipt,
        source,
        proposed
    ))
}

/// The instruction defining `register` at `position`: the last instruction
/// in the block before it carrying a non-`Use` operand on that register —
/// the replay's own last-definition scan.
/// The register-only unit surface, restated on one record: no implicit
/// uses, definitions, or clobbers, and no operand unit bindings.
fn register_only(instruction: &SelectedInstruction) -> bool {
    instruction.implicit_uses.is_empty()
        && instruction.implicit_defs.is_empty()
        && instruction.clobbers.is_empty()
        && instruction.operands.iter().all(|operand| {
            operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
        })
}

/// The declared surface the retained producer must carry, restated on the
/// catalog row alone: the `AddressOffset` projection observes no memory,
/// never traps, and publishes no barrier, call, or cleanup — and every
/// alternative encodes no memory, unchanged stack, `NeverV1`, fall-through,
/// and no implicit unit traffic.
fn isolated_projection(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
        && declaration.alternatives.iter().all(|alternative| {
            let encoded = &alternative.encoded;
            encoded.memory == MachineEncodedMemoryEffect::NoneV1
                && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                && encoded.trap == MachineEncodedTrapBehavior::NeverV1
                && encoded.control == MachineEncodedControlEffect::FallThroughV1
                && encoded.implicit_unit_uses.is_empty()
                && encoded.implicit_unit_defs.is_empty()
                && encoded.implicit_unit_clobbers.is_empty()
        })
}

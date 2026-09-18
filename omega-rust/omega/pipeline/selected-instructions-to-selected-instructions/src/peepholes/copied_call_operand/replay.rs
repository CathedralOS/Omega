//! Independent replay of a copied-call-operand reroute.
//!
//! The validator never consults
//! [`CopiedCallOperandRule`](super::pair::CopiedCallOperandRule) or the
//! declared table: it re-derives the whole grammar from the instruction
//! records — the consumer kind and its leading-`Use`, trailing-`Def`
//! operand roster, the named producer as the destination register's last
//! in-block definition under a clean `CopyI64` record, the source
//! register's freedom from intervening redefinition, the operand classes
//! against the roster, and the catalog surface the call row and the
//! isolated copy row must carry — then rebuilds the expected rerouted
//! record, requires the proposal to equal it, and restores the complete
//! source by content.

use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, ValidatedMachineEffectCatalog,
    VirtualRegisterId,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{CopiedCallOperandError, CopiedCallOperandReceipt, ValidatedCopiedCallOperand};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: the replay re-derives the
/// copied-call-operand reroute and its reconstructed call record from the
/// source records — re-scanning the destination's last definition,
/// re-checking the source's intervening definitions, re-checking the
/// operand roster and the ABI views against the bound constraint row, and
/// re-stating the retained call surface — the `Call` barrier, the
/// `DirectInternalNormalReturnV1` effect, `DirectRelativeCallV1` control,
/// the retained fault surface, and the target's stack lifecycle — on the
/// bound catalog — requires the proposed instruction to equal that
/// reconstruction, and reinserts the source instruction to restore the
/// complete source by content: every other instruction, terminator,
/// register, roster row, call, settlement, and function included. The pair
/// descriptor table is never read.
pub fn validate_copied_call_operand_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    producer: SelectedInstructionId,
    call: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCopiedCallOperand, CopiedCallOperandError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CopiedCallOperandError::SourceMismatch);
    }
    // The replay re-derives the catalog binding too: the bound catalog must
    // describe this plan's target and this environment's constraint catalog
    // and selected-key inventory.
    let catalog = effect_catalog.catalog();
    if catalog.target != plan.target
        || catalog.register_constraints != environment.constraints().identity()
        || catalog.selected_keys != environment.selected_keys()
    {
        return Err(CopiedCallOperandError::EffectSurfaceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CopiedCallOperandError::SourceMismatch)?;
    let (block_index, position, consumer) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find(|(_, instruction)| instruction.id == call)
                .map(|(position, instruction)| (block_index, position, instruction))
        })
        .ok_or(CopiedCallOperandError::UnsupportedConsumer)?;
    // The replay's own grammar: the consumer kind must be one of the
    // emitted direct internal call forms — a body instruction whose operand
    // roster is the leading-`Use` argument list ahead of the result roster
    // its semantic owns: none for `CallUnit`, one for `CallScalar`, one or
    // two for `CallAggregate`. The normalized foreign call's boundary
    // custody is not a direct internal call contract and cannot replay as
    // one.
    let (consumer_kind, result_roster) = match consumer.kind {
        SelectedInstructionKind::CallUnit { .. } => (MachineSemanticKind::CallUnit, 0..=0),
        SelectedInstructionKind::CallScalar { .. } => (MachineSemanticKind::CallScalar, 1..=1),
        SelectedInstructionKind::CallAggregate { .. } => {
            (MachineSemanticKind::CallAggregate, 1..=2)
        }
        _ => return Err(CopiedCallOperandError::UnsupportedConsumer),
    };
    // The operand roster, restated from the record alone: dense operand
    // numbers, the leading `Use` run ahead of the trailing `Def` results,
    // the semantic's result count — and at least one operand overall for
    // the aggregate roster — with no `tied_to` or `early_clobber` binding.
    let arity = consumer
        .operands
        .iter()
        .take_while(|operand| operand.access == RegisterOperandAccess::Use)
        .count();
    let (arguments, results) = consumer.operands.split_at(arity);
    if !result_roster.contains(&results.len())
        || (consumer_kind == MachineSemanticKind::CallAggregate && consumer.operands.is_empty())
        || !results
            .iter()
            .all(|operand| operand.access == RegisterOperandAccess::Def)
        || !consumer
            .operands
            .iter()
            .enumerate()
            .all(|(index, operand)| {
                operand.operand as usize == index
                    && operand.tied_to.is_none()
                    && !operand.early_clobber
            })
        || !arguments
            .iter()
            .all(|operand| operand.access == RegisterOperandAccess::Use)
    {
        return Err(CopiedCallOperandError::UnsupportedConsumer);
    }
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    if consumer
        .operands
        .iter()
        .any(|operand| find_register(operand.virtual_register).is_none())
    {
        return Err(CopiedCallOperandError::UnsupportedUse);
    }
    // The bound constraint row must restate the record verbatim — dense
    // operand numbers, matching accesses, classes, and ABI `fixed_view`
    // pins on every operand, and the implicit uses, implicit definitions,
    // and clobber roster the rewritten record republishes — and every
    // operand's class must equal its roster entry's.
    let row = environment
        .constraint(consumer.constraint)
        .ok_or(CopiedCallOperandError::ConstraintMismatch)?;
    if row.operands.len() != consumer.operands.len()
        || row
            .operands
            .iter()
            .zip(&consumer.operands)
            .any(|(row_operand, operand)| {
                row_operand.operand != operand.operand
                    || row_operand.access != operand.access
                    || row_operand.class != operand.class
                    || row_operand.fixed_view != operand.fixed_view
                    || row_operand.tied_to != operand.tied_to
                    || row_operand.early_clobber != operand.early_clobber
            })
        || row.implicit_uses != consumer.implicit_uses
        || row.implicit_defs != consumer.implicit_defs
        || row.clobbers != consumer.clobbers
        || !row
            .operands
            .iter()
            .all(|operand| operand.fixed_view.is_some())
        || consumer.operands.iter().any(|operand| {
            find_register(operand.virtual_register)
                .is_some_and(|register| register.class != operand.class)
        })
    {
        return Err(CopiedCallOperandError::ConstraintMismatch);
    }
    // The named producer must be a body instruction in the consumer's block
    // ahead of it — the emitted `[use source, def destination]` `CopyI64`
    // under the register-only surface.
    let producer_index = function.blocks[block_index].instructions[..position]
        .iter()
        .position(|instruction| instruction.id == producer)
        .ok_or(CopiedCallOperandError::UnsupportedProducer)?;
    let producer_instruction = &function.blocks[block_index].instructions[producer_index];
    if !matches!(producer_instruction.kind, SelectedInstructionKind::CopyI64)
        || !register_only(producer_instruction)
    {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    let [source_operand, destination_operand] = producer_instruction.operands.as_slice() else {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    };
    if source_operand.operand != 0
        || source_operand.access != RegisterOperandAccess::Use
        || destination_operand.operand != 1
        || destination_operand.access != RegisterOperandAccess::Def
    {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    let copied_source = source_operand.virtual_register;
    let destination = destination_operand.virtual_register;
    if copied_source == destination {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    // The producer must be the destination's last definition before the
    // call — the value every copied operand observes — re-derived from the
    // records alone.
    if last_definition_before(
        &function.blocks[block_index].instructions,
        position,
        destination,
    ) != Some(producer_index)
    {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    if !consumer.operands.iter().any(|operand| {
        operand.access == RegisterOperandAccess::Use && operand.virtual_register == destination
    }) {
        return Err(CopiedCallOperandError::UnsupportedUse);
    }
    let source_register =
        find_register(copied_source).ok_or(CopiedCallOperandError::UnsupportedUse)?;
    // The producer's own row must declare the `[use, def]` shape its
    // operands carry at matching classes — a copy whose operand classes
    // differ forwards bits between register classes, not the transparent
    // value this fold names — under the register-only surface.
    let producer_row = environment
        .constraint(producer_instruction.constraint)
        .ok_or(CopiedCallOperandError::ConstraintMismatch)?;
    if producer_row.operands.len() != 2
        || producer_row.operands[0].operand != 0
        || producer_row.operands[0].access != RegisterOperandAccess::Use
        || producer_row.operands[0].class != source_operand.class
        || producer_row.operands[0].class != source_register.class
        || producer_row.operands[1].operand != 1
        || producer_row.operands[1].access != RegisterOperandAccess::Def
        || producer_row.operands[1].class != destination_operand.class
        || producer_row.operands[0].class != producer_row.operands[1].class
        || !producer_row.implicit_uses.is_empty()
        || !producer_row.implicit_defs.is_empty()
        || !producer_row.clobbers.is_empty()
    {
        return Err(CopiedCallOperandError::ConstraintMismatch);
    }
    if consumer.operands.iter().any(|operand| {
        operand.access == RegisterOperandAccess::Use
            && operand.virtual_register == destination
            && operand.class != source_register.class
    }) {
        return Err(CopiedCallOperandError::UnsupportedUse);
    }
    // The open interval between the copy and the call must leave the
    // source alone: a rebound operand reads the source at the call's
    // position.
    for instruction in &function.blocks[block_index].instructions[producer_index + 1..position] {
        if instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use
                && operand.virtual_register == copied_source
        }) {
            return Err(CopiedCallOperandError::UnsupportedUse);
        }
    }
    // The effect surface is restated on the bound catalog: the retained
    // producer is the isolated copy and the call row carries its complete
    // activation contract — operand and implicit-unit rosters, barrier,
    // call effect, control, retained fault, and the target's stack
    // lifecycle — verbatim, because the rewritten form is the same
    // constraint row performing the identical call.
    let producer_declaration = effect_declaration(
        effect_catalog,
        MachineSemanticKind::CopyI64,
        producer_instruction.constraint,
    )
    .ok_or(CopiedCallOperandError::EffectSurfaceMismatch)?;
    let call_declaration = effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
        .ok_or(CopiedCallOperandError::EffectSurfaceMismatch)?;
    if !isolated_copy(producer_declaration) || !retained_call(call_declaration, row) {
        return Err(CopiedCallOperandError::EffectSurfaceMismatch);
    }
    let mut expected = consumer.clone();
    for operand in &mut expected.operands {
        if operand.access == RegisterOperandAccess::Use && operand.virtual_register == destination {
            operand.virtual_register = copied_source;
        }
    }
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CopiedCallOperandError::ReplayMismatch)?;
    if proposed_function
        .blocks
        .get(block_index)
        .and_then(|block| block.instructions.get(position))
        != Some(&expected)
    {
        return Err(CopiedCallOperandError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[block_index].instructions[position] =
        consumer.clone();
    if restored != *source.selected_plan() {
        return Err(CopiedCallOperandError::ReplayMismatch);
    }
    // The replay charges the same bounded walk into the budget even though
    // admission already paid it: a proposal validated on a starved budget
    // must not pass where admission would refuse.
    let block = &function.blocks[block_index];
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            total
                .checked_add(block.instructions.len())?
                .checked_add(block.instructions.len())
        })
        .ok_or(CopiedCallOperandError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CopiedCallOperandError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CopiedCallOperandError::WorkBudgetExceeded);
    }
    Ok(ValidatedCopiedCallOperand {
        receipt: CopiedCallOperandReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

/// The instruction defining `register` at `position`: the last instruction
/// in the block before it carrying a non-`Use` operand on that register —
/// the replay's own last-definition scan.
fn last_definition_before(
    instructions: &[SelectedInstruction],
    position: usize,
    register: VirtualRegisterId,
) -> Option<usize> {
    instructions[..position].iter().rposition(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
        })
    })
}

/// The register-only unit surface, restated on one record: no implicit
/// uses, definitions, or clobbers, and no operand unit bindings — the
/// `CopyI64` producer's contract.
fn register_only(instruction: &SelectedInstruction) -> bool {
    instruction.implicit_uses.is_empty()
        && instruction.implicit_defs.is_empty()
        && instruction.clobbers.is_empty()
        && instruction.operands.iter().all(|operand| {
            operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
        })
}

/// The declared surface the retained producer must carry, restated on the
/// catalog row alone: the `CopyI64` observes no memory, never traps, and
/// publishes no barrier, call, or cleanup — and every alternative encodes
/// no memory, unchanged stack, `NeverV1`, fall-through, and no implicit
/// unit traffic.
fn isolated_copy(declaration: &MachineEffectDeclaration) -> bool {
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

/// The declared surface the call row must carry, restated on the catalog
/// row alone: `NoneV1` memory and `NeverV1` trap at the declaration level,
/// the `Call` barrier, the `DirectInternalNormalReturnV1` call effect, no
/// cleanup, and every alternative encoding the constraint row's operand
/// and implicit-unit rosters exactly, `DirectRelativeCallV1` control, the
/// retained `MayArchitecturalFaultV1` trap, and the target's activation-
/// stack lifecycle.
fn retained_call(
    declaration: &MachineEffectDeclaration,
    row: &RegisterInstructionConstraint,
) -> bool {
    let reads: Vec<u16> = row
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Use)
        .map(|operand| operand.operand)
        .collect();
    let writes: Vec<u16> = row
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Def)
        .map(|operand| operand.operand)
        .collect();
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::Call
        && matches!(
            declaration.call,
            MachineCallEffect::DirectInternalNormalReturnV1 { .. }
        )
        && declaration.cleanup == MachineCleanupEffect::NoneV1
        && declaration
            .alternatives
            .iter()
            .all(|alternative| call_alternative(alternative, &reads, &writes, row))
}

/// The encoded surface a retained-call alternative must carry, restated
/// without the table: the constraint row's operand and implicit-unit
/// rosters exactly, the `MayArchitecturalFaultV1` trap the call retains,
/// `DirectRelativeCallV1` control, and the activation-stack lifecycle —
/// `WriteReturnAddressBelowStackPointerV1` memory paired with
/// `CallReturnAddressLifecycleV1` stack at matching nonzero byte counts on
/// a stack-pushing target, or no memory and `UnchangedV1` stack where the
/// link register holds the return address.
fn call_alternative(
    alternative: &MachineAlternative,
    reads: &[u16],
    writes: &[u16],
    row: &RegisterInstructionConstraint,
) -> bool {
    let encoded = &alternative.encoded;
    let lifecycle = match (encoded.memory, encoded.stack) {
        (
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: memory_pointer,
                byte_count,
            },
            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer,
                return_address_byte_count,
            },
        ) => {
            memory_pointer == stack_pointer
                && byte_count == return_address_byte_count
                && return_address_byte_count != 0
        }
        (MachineEncodedMemoryEffect::NoneV1, MachineEncodedStackEffect::UnchangedV1) => true,
        _ => false,
    };
    lifecycle
        && encoded.external_operand_reads == reads
        && encoded.external_operand_writes == writes
        && encoded.implicit_unit_uses == row.implicit_uses
        && encoded.implicit_unit_defs == row.implicit_defs
        && encoded.implicit_unit_clobbers == row.clobbers
        && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        && encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form.
fn effect_declaration(
    catalog: &ValidatedMachineEffectCatalog,
    semantic: MachineSemanticKind,
    constraint: register_model::RegisterConstraintKey,
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

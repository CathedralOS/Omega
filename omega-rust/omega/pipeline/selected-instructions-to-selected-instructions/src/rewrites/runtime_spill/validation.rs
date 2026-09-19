use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedCasePayloadTransport, SelectedInstructionPlan,
    SelectedLocalStorageSlot, SelectedStructuralTransport, SelectedValueTransport,
    VirtualRegisterId,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    RuntimeSpillError, RuntimeSpillReceipt, RuntimeSpillSpanPolicy, ValidatedRuntimeSpill,
    admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed instruction stream, checking every private
/// access and rewritten operand against its original use. Stripping these exact
/// additions must restore the entire admitted source, including calls and fuel.
/// Replays under the bounded span policy the default rewrite produces.
pub fn validate_runtime_spill(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    validate_runtime_spill_with_span_policy(
        source,
        function_index,
        register,
        environment,
        budget,
        proposed,
        RuntimeSpillSpanPolicy::UnitWriteBounded,
    )
}

/// The same independent consumption under an explicit span policy: replay
/// must follow the producer's own open/close decision exactly, so a plan
/// produced under the other policy — a shared reload reaching across a
/// unit-writing instruction, or a private pair where the crossing span
/// stayed open — mismatches rather than validates.
pub fn validate_runtime_spill_with_span_policy(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
    span_policy: RuntimeSpillSpanPolicy,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    let admitted = admission::admit(
        source,
        function_index,
        register,
        environment,
        span_policy,
        budget,
    )?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(RuntimeSpillError::ReplayMismatch)?;
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    if restored_function.boundary_settlements.len() != admitted.function.boundary_settlements.len()
    {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    let mut values = function
        .virtual_registers
        .iter()
        .skip(admitted.function.virtual_registers.len());
    let mut next_instruction = admitted.first_instruction;
    let mut next_register = admitted.first_register;
    for (block_index, source_block) in admitted.function.blocks.iter().enumerate() {
        if !admitted.use_blocks.contains(&block_index)
            && !admitted
                .definitions
                .iter()
                .any(|definition| definition.block_index == block_index)
        {
            continue;
        }
        let block = function
            .blocks
            .get(block_index)
            .ok_or(RuntimeSpillError::ReplayMismatch)?;
        let mut stream = block.instructions.iter();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        let mut consumed = 0u32;
        // Replay reconstructs the same block-local decision independently:
        // when admission proved a surviving view, the first unpinned
        // instruction-operand use emits the pair and each later unpinned use
        // names that still-open reload register until an instruction that can
        // destroy register content — and is not crossed under the admitted
        // span policy — closes it. A pinned operand or an unadmitted block
        // consumes a fresh pair at every use.
        let shared = admitted.shared_reload[block_index];
        let mut open_reload: Option<VirtualRegisterId> = None;
        // The register each restored instruction's first victim use must
        // name — replay computes the same binding-argument target the
        // proposal had to produce.
        let mut use_reloads = std::collections::BTreeMap::new();
        // A boundary definition's store opens the block; replay consumes it
        // from the stream before the first source instruction, in the same
        // order the proposal emitted it.
        for definition in admitted.definitions.iter().filter(|definition| {
            definition.block_index == block_index
                && matches!(definition.position, admission::StoragePosition::BlockStart)
        }) {
            let store = admission::store(
                &admitted,
                definition.register,
                &mut next_instruction,
                &mut next_register,
            )?;
            let (registers, sequence) = store.into_streams();
            for register in &registers {
                if values.next() != Some(register) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            for instruction in &sequence {
                if stream.next() != Some(instruction) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            consumed = consumed
                .checked_add(
                    u32::try_from(sequence.len())
                        .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                )
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
        }
        for (instruction_index, original) in source_block.instructions.iter().enumerate() {
            boundaries.push(consumed);
            let mut restored = original.clone();
            // `UseDef` operands read the victim like uses: replay redirects
            // them to the same reload registers, and the operand's own write
            // is what the instruction's following store then reads. A plain
            // `Def` — including one tied to a victim use — keeps the victim
            // register its store replays.
            for operand in &mut restored.operands {
                if operand.virtual_register != register
                    || !matches!(
                        operand.access,
                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                    )
                {
                    continue;
                }
                let share = shared && operand.fixed_view.is_none();
                let reloaded = match (share, open_reload) {
                    (true, Some(existing)) => existing,
                    _ => {
                        let reload = admission::reload(
                            &admitted,
                            register,
                            &mut next_instruction,
                            &mut next_register,
                        )?;
                        let reloaded = reload.reload_register.id;
                        let (registers, sequence) = reload.into_streams();
                        for register in &registers {
                            if values.next() != Some(register) {
                                return Err(RuntimeSpillError::ReplayMismatch);
                            }
                        }
                        for instruction in &sequence {
                            if stream.next() != Some(instruction) {
                                return Err(RuntimeSpillError::ReplayMismatch);
                            }
                        }
                        consumed = consumed
                            .checked_add(
                                u32::try_from(sequence.len())
                                    .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                            )
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                        if share {
                            open_reload = Some(reloaded);
                        }
                        reloaded
                    }
                };
                use_reloads.entry(original.id).or_insert(reloaded);
                operand.virtual_register = reloaded;
            }
            instruction_positions.push(consumed);
            if stream.next() != Some(&restored) {
                return Err(RuntimeSpillError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(1)
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            // The same span-closing instruction — a victim redefinition, or
            // a unit writer the admitted span policy did not cross — that
            // closed the proposal's open reload closes it here, so the next
            // unpinned use must name a fresh pair rather than the
            // pre-boundary register.
            if admitted.span_closes[block_index].contains(&instruction_index) {
                open_reload = None;
            }
            for definition in admitted.definitions.iter().filter(|definition| {
                definition.block_index == block_index
                    && match definition.position {
                        admission::StoragePosition::AfterInstruction(anchor)
                        | admission::StoragePosition::AfterUseDef {
                            instruction: anchor,
                            ..
                        } => anchor == original.id,
                        admission::StoragePosition::BlockStart => false,
                    }
            }) {
                // The `UseDef` store reads the register its operand was
                // redirected to — replay resolves it on the rebuilt
                // instruction, exactly where the proposal's emitter read it.
                let stored = match definition.position {
                    admission::StoragePosition::AfterUseDef { operand, .. } => restored
                        .operands
                        .iter()
                        .find(|candidate| candidate.operand == operand)
                        .map(|operand| operand.virtual_register)
                        .ok_or(RuntimeSpillError::ReplayMismatch)?,
                    _ => definition.register,
                };
                let store =
                    admission::store(&admitted, stored, &mut next_instruction, &mut next_register)?;
                let (registers, sequence) = store.into_streams();
                for register in &registers {
                    if values.next() != Some(register) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                for instruction in &sequence {
                    if stream.next() != Some(instruction) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                consumed = consumed
                    .checked_add(
                        u32::try_from(sequence.len())
                            .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                    )
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
            }
        }
        // The rewrite appends terminator-operand reloads after the last body
        // instruction; replay consumes them in operand order, then requires
        // the proposed terminator to equal the source with exactly those
        // operands redirected to their reload registers.
        let mut expected_terminator = source_block.terminator.clone();
        for operand in &mut super::control_mut(&mut expected_terminator).operands {
            if operand.virtual_register != register || operand.access != RegisterOperandAccess::Use
            {
                continue;
            }
            let reload = admission::reload(
                &admitted,
                register,
                &mut next_instruction,
                &mut next_register,
            )?;
            let reloaded = reload.reload_register.id;
            let (registers, sequence) = reload.into_streams();
            for register in &registers {
                if values.next() != Some(register) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            for instruction in &sequence {
                if stream.next() != Some(instruction) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            consumed = consumed
                .checked_add(
                    u32::try_from(sequence.len())
                        .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                )
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            operand.virtual_register = reloaded;
        }
        // Binding-argument reloads follow the terminator-operand pairs in
        // successor, then binding, then case-payload order; the expected
        // terminator carries the moved argument on each matching transport
        // and nothing else.
        for successor in super::control_successors_mut(&mut expected_terminator)
            .into_iter()
            .flatten()
        {
            for binding in &mut successor.bindings {
                if !matches!(
                    binding.transport,
                    SelectedValueTransport::Registers { argument, .. } if argument == register)
                {
                    continue;
                }
                let reload = admission::reload(
                    &admitted,
                    register,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                let reloaded = reload.reload_register.id;
                let (registers, sequence) = reload.into_streams();
                for register in &registers {
                    if values.next() != Some(register) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                for instruction in &sequence {
                    if stream.next() != Some(instruction) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                consumed = consumed
                    .checked_add(
                        u32::try_from(sequence.len())
                            .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                    )
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
                let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport
                else {
                    unreachable!()
                };
                *argument = reloaded;
            }
            // A stored structural-transport argument consumes no pair: the
            // expected terminator only retargets it to the single register
            // the binding's snapshot chunk loads were proven to name.
            for binding in &mut successor.structural_bindings {
                let Some(byte_size) = admission::stored_transport_size(binding.transport) else {
                    continue;
                };
                let (SelectedStructuralTransport::Descriptor { argument, .. }
                | SelectedStructuralTransport::WholeValue { argument, .. }) =
                    &mut binding.transport
                else {
                    continue;
                };
                if *argument != register {
                    continue;
                }
                let Some(pending) = admitted.structural_uses.iter().find(|pending| {
                    pending.block == block_index
                        && pending.edge == successor.psi_edge
                        && pending.place == binding.semantic.argument.place
                        && pending.byte_size == byte_size
                }) else {
                    return Err(RuntimeSpillError::ReplayMismatch);
                };
                let reloaded = pending
                    .loads
                    .iter()
                    .try_fold(None, |found: Option<VirtualRegisterId>, load| {
                        let named = use_reloads
                            .get(load)
                            .copied()
                            .ok_or(RuntimeSpillError::ReplayMismatch)?;
                        match found {
                            None => Ok(Some(named)),
                            Some(existing) if existing == named => Ok(found),
                            _ => Err(RuntimeSpillError::ReplayMismatch),
                        }
                    })?
                    .ok_or(RuntimeSpillError::ReplayMismatch)?;
                *argument = reloaded;
            }
            if let Some(case) = &mut successor.structural_case {
                for payload in &mut case.payloads {
                    if !matches!(
                        payload.transport,
                        SelectedCasePayloadTransport::Registers { argument, .. }
                            if argument == register)
                    {
                        continue;
                    }
                    let reload = admission::reload(
                        &admitted,
                        register,
                        &mut next_instruction,
                        &mut next_register,
                    )?;
                    let reloaded = reload.reload_register.id;
                    let (registers, sequence) = reload.into_streams();
                    for register in &registers {
                        if values.next() != Some(register) {
                            return Err(RuntimeSpillError::ReplayMismatch);
                        }
                    }
                    for instruction in &sequence {
                        if stream.next() != Some(instruction) {
                            return Err(RuntimeSpillError::ReplayMismatch);
                        }
                    }
                    consumed = consumed
                        .checked_add(
                            u32::try_from(sequence.len())
                                .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                        )
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    let SelectedCasePayloadTransport::Registers { argument, .. } =
                        &mut payload.transport
                    else {
                        unreachable!()
                    };
                    *argument = reloaded;
                }
            }
        }
        if block.terminator != expected_terminator {
            return Err(RuntimeSpillError::ReplayMismatch);
        }
        restored_function.blocks[block_index]
            .terminator
            .clone_from(&source_block.terminator);
        boundaries.push(consumed);
        instruction_positions.push(consumed);
        if stream.next().is_some() {
            return Err(RuntimeSpillError::ReplayMismatch);
        }
        for (actual, original) in restored_function
            .boundary_settlements
            .iter_mut()
            .zip(&admitted.function.boundary_settlements)
        {
            if original.block != source_block.id {
                continue;
            }
            let positions = if matches!(
                original.settlement,
                SelectedBoundarySettlementPayload::ClaimCompletion(_)
            ) {
                &boundaries
            } else {
                &instruction_positions
            };
            let expected = positions
                .get(original.instruction_index as usize)
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            if actual.block != source_block.id || actual.instruction_index != *expected {
                return Err(RuntimeSpillError::ReplayMismatch);
            }
            actual.instruction_index = original.instruction_index;
        }
        restored_function.blocks[block_index]
            .instructions
            .clone_from(&source_block.instructions);
    }
    if values.next().is_some() {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    // A fresh private slot must be the appended tail entry; a shared slot
    // declared nothing, so the storage list must equal the source exactly.
    if admitted.fresh_slot {
        if restored_function.local_storage_slots.pop()
            != Some(SelectedLocalStorageSlot {
                id: admitted.slot,
                byte_size: 8,
                alignment: 8,
            })
        {
            return Err(RuntimeSpillError::ReplayMismatch);
        }
    } else if restored_function.local_storage_slots != admitted.function.local_storage_slots {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    restored_function
        .virtual_registers
        .truncate(admitted.function.virtual_registers.len());
    if restored != *source.selected_plan() {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    Ok(ValidatedRuntimeSpill {
        receipt: RuntimeSpillReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    LocalStorageSlotId, SelectedBoundarySettlementPayload, SelectedCasePayloadTransport,
    SelectedInstruction, SelectedLocalStorageSlot, SelectedStructuralTransport,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId,
};

use super::{
    RuntimeSpillDerivation, RuntimeSpillError, RuntimeSpillSpanPolicy, ValidatedRuntimeSpill,
    admission, derive_runtime_spill,
};
use crate::ValidatedSelectedAnalysis;

/// Emit one private address/load pair for a use, or — when `share` admits a
/// block-local shared reload — reuse the pair still open from the span's
/// first flexible use so its interval covers every later flexible use in the
/// span. The caller clears the open pair after each admitted span-closing
/// instruction — a victim redefinition, or a unit writer the policy did not
/// cross — so under the bounded policy the interval never reaches across a
/// call.
/// Returns the register the rewritten use must name.
fn reload_for_use(
    admitted: &RuntimeSpillDerivation<'_>,
    slot: LocalStorageSlotId,
    register: VirtualRegisterId,
    share: bool,
    open: &mut Option<VirtualRegisterId>,
    virtual_registers: &mut Vec<VirtualRegister>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<VirtualRegisterId, RuntimeSpillError> {
    if share && let Some(existing) = *open {
        return Ok(existing);
    }
    let reload = admission::reload(admitted, slot, register, next_instruction, next_register)?;
    let reloaded = reload.reload_register.id;
    let (registers, sequence) = reload.into_streams();
    virtual_registers.extend(registers);
    instructions.extend(sequence);
    if share {
        *open = Some(reloaded);
    }
    Ok(reloaded)
}

/// Store one runtime value after its definition and reload before each
/// flexible use. Publish its exact source-bound transformation after admission.
/// The historical bounded policy keeps every produced interval inside
/// unit-free spans.
pub fn spill_selected_runtime_value(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    spill_selected_runtime_value_with_span_policy(
        source,
        function_index,
        register,
        environment,
        budget,
        RuntimeSpillSpanPolicy::UnitWriteBounded,
    )
}

/// The same rewrite under an explicit span policy. `UnitWriteCrossing` lets a
/// still-open reload reach across a unit-writing instruction — most often a
/// `CallUnit` — while admission proves a view of the victim's class survives
/// every unit written inside the span; the allocator then decides whether the
/// surviving callee-saved home is actually free.
pub fn spill_selected_runtime_value_with_span_policy(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    span_policy: RuntimeSpillSpanPolicy,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    spill_selected_runtime_value_with_crossing_status(
        source,
        function_index,
        register,
        environment,
        budget,
        span_policy,
    )
    .map(|(spill, _)| spill)
}

/// Like [`spill_selected_runtime_value_with_span_policy`], also reporting
/// whether `UnitWriteCrossing` crossed at least one unit-writing instruction
/// (when `false`, the produced rewrite is identical to `UnitWriteBounded`).
pub fn spill_selected_runtime_value_with_crossing_status(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    span_policy: RuntimeSpillSpanPolicy,
) -> Result<(ValidatedRuntimeSpill, bool), RuntimeSpillError> {
    let admitted = derive_runtime_spill(
        source,
        function_index,
        register,
        environment,
        span_policy,
        budget,
    )?;
    let crossed_unit_write = admitted.crossed_unit_write;
    let mut transformed = source.selected_plan().clone();
    transformed
        .functions
        .mutate_preserving_register_prefix(
            function_index,
            |local_storage_slots, virtual_registers, boundary_settlements, blocks| -> Result<(), RuntimeSpillError> {
                // Reused slots are already declared, so their bytes stay charged exactly
                // once in the function's frame demand; the private slot is appended only
                // when at least one storage window landed on it.
                if admitted.assignment.declares_private {
                    local_storage_slots.push(SelectedLocalStorageSlot {
                        id: admitted.assignment.private,
                        byte_size: 8,
                        alignment: 8,
                    });
                }
                let mut next_instruction = admitted.first_instruction;
                let mut next_register = admitted.first_register;
                for (block_index, block) in admitted.function.blocks.iter().enumerate() {
                    if !admitted.use_blocks.contains(&block_index)
                        && !admitted
                            .definitions
                            .iter()
                            .any(|definition| definition.block_index == block_index)
                    {
                        continue;
                    }
                    let shared = admitted.shared_reload[block_index];
                    let mut open_reload = None;
                    // The reload register each rewritten instruction's first victim use
                    // names. Structural-snapshot chunk loads each carry exactly one victim
                    // operand, so their entries let the binding's `argument` field follow
                    // the register its loads actually read.
                    let mut use_reloads = std::collections::BTreeMap::new();
                    let has_settlements = boundary_settlements
                        .iter()
                        .any(|settlement| settlement.block == block.id);
                    let mut instructions = Vec::with_capacity(block.instructions.len() + 8);
                    let mut boundaries = if has_settlements {
                        Vec::with_capacity(block.instructions.len() + 1)
                    } else {
                        Vec::new()
                    };
                    let mut instruction_positions = if has_settlements {
                        Vec::with_capacity(block.instructions.len() + 1)
                    } else {
                        Vec::new()
                    };
                    let mut has_block_start_store = false;
                    // An entry parameter's store opens its block: the register is live-in,
                    // so position zero is the earliest point the slot holds the value and
                    // the only one every use — body, terminator, or edge — follows.
                    for (definition_index, definition) in
                        admitted
                            .definitions
                            .iter()
                            .enumerate()
                            .filter(|(_, definition)| {
                                definition.block_index == block_index
                                    && matches!(
                                        definition.position,
                                        super::StoragePosition::BlockStart
                                    )
                            })
                    {
                        has_block_start_store = true;
                        let store = admission::store(
                            &admitted,
                            admitted.assignment.store_slot(definition_index),
                            definition.register,
                            &mut next_instruction,
                            &mut next_register,
                        )?;
                        let (registers, sequence) = store.into_streams();
                        virtual_registers.extend(registers);
                        instructions.extend(sequence);
                    }
                    let active_window = if has_settlements || block.instructions.is_empty() {
                        if block.instructions.is_empty() {
                            None
                        } else {
                            Some((0usize, block.instructions.len() - 1))
                        }
                    } else {
                        let mut window = admitted
                            .function
                            .blocks
                            .block_register_instruction_span(block_index, register);
                        for definition in &admitted.definitions {
                            if definition.block_index != block_index {
                                continue;
                            }
                            if let super::StoragePosition::AfterInstruction(anchor)
                            | super::StoragePosition::AfterUseDef {
                                instruction: anchor,
                                ..
                            } = definition.position
                                && let Some(idx) = admitted
                                    .function
                                    .blocks
                                    .instruction_index_in_block(block_index, anchor)
                            {
                                window = Some(match window {
                                    Some((s, e)) => (s.min(idx), e.max(idx)),
                                    None => (idx, idx),
                                });
                            }
                        }
                        window
                    };
                    let unchanged_prefix = if has_block_start_store {
                        0
                    } else {
                        active_window.map_or(block.instructions.len(), |(start, _)| start)
                    };
                    if let Some((active_start, active_end)) = active_window {
                        if active_start > 0 {
                            instructions.extend_from_slice(&block.instructions[..active_start]);
                        }
                        let has_retained_uses = !admitted.retained_uses.is_empty();
                        let block_dedicated_ties = &admitted.dedicated_tie_uses[block_index];
                        let has_dedicated_ties = !block_dedicated_ties.is_empty();
                        let block_span_closes = &admitted.span_closes[block_index];
                        let has_span_closes = !block_span_closes.is_empty();
                        let has_after_defs = admitted.definitions.iter().any(|definition| {
                            definition.block_index == block_index
                                && !matches!(definition.position, super::StoragePosition::BlockStart)
                        });
                        for (offset, original) in block.instructions[active_start..=active_end]
                            .iter()
                            .enumerate()
                        {
                            let instruction_index = active_start + offset;
                            if has_settlements {
                                boundaries.push(
                                    u32::try_from(instructions.len())
                                        .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                                );
                            }
                            let mut rewritten = original.clone();
                            // Uses and `UseDef` operands both read the victim and move to a
                            // reload register; a `UseDef` additionally writes that register
                            // back, which the instruction's following store then reads. A
                            // plain `Def` keeps the victim register so its own store reads
                            // the post-write value — possibly bound to a tied use's reload
                            // home, which is the read-modify-write idiom's point.
                            for operand in &mut rewritten.operands {
                                if operand.virtual_register != register
                                    || (has_retained_uses
                                        && admitted
                                            .retained_uses
                                            .contains(&(original.id, operand.operand)))
                                    || !matches!(
                                        operand.access,
                                        RegisterOperandAccess::Use
                                            | RegisterOperandAccess::UseDef
                                    )
                                {
                                    continue;
                                }
                                // A use admission recorded beside an early-clobber tied write
                                // takes a pair of its own: the write's tied home must be a
                                // register the instruction's unpinned co-readers never share.
                                let dedicated = has_dedicated_ties
                                    && block_dedicated_ties
                                        .contains(&(instruction_index, operand.operand));
                                let reloaded = reload_for_use(
                                    &admitted,
                                    admitted
                                        .assignment
                                        .load_slot(block_index, instruction_index),
                                    register,
                                    shared && operand.fixed_view.is_none() && !dedicated,
                                    &mut open_reload,
                                    virtual_registers,
                                    &mut instructions,
                                    &mut next_instruction,
                                    &mut next_register,
                                )?;
                                use_reloads.entry(rewritten.id).or_insert(reloaded);
                                operand.virtual_register = reloaded;
                            }
                            if has_settlements {
                                instruction_positions.push(
                                    u32::try_from(instructions.len())
                                        .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                                );
                            }
                            instructions.push(rewritten);
                            // A clobber or implicit definition may write any unit, including
                            // the one hosting the still-open reload, and a redefinition makes
                            // the held value stale, so the shared interval ends at each
                            // admitted span-closing instruction and the next flexible use
                            // opens a fresh pair — unless admission proved a surviving view
                            // lets the produced interval cross a unit-writing instruction to
                            // its home.
                            if has_span_closes
                                && open_reload.is_some()
                                && block_span_closes.contains(&instruction_index)
                            {
                                open_reload = None;
                            }
                            if has_after_defs {
                                for (definition_index, definition) in
                                    admitted
                                        .definitions
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, definition)| {
                                            definition.block_index == block_index
                                                && match definition.position {
                                                    super::StoragePosition::AfterInstruction(
                                                        anchor,
                                                    )
                                                    | super::StoragePosition::AfterUseDef {
                                                        instruction: anchor,
                                                        ..
                                                    } => anchor == original.id,
                                                    super::StoragePosition::BlockStart => false,
                                                }
                                        })
                                {
                                    // A `UseDef` write left the new value in the register its
                                    // operand was redirected to — the emitted instruction's
                                    // operand names it; the victim register itself was never
                                    // written. Every other definition stores the recorded
                                    // register directly.
                                    let stored = match definition.position {
                                        super::StoragePosition::AfterUseDef { operand, .. } => {
                                            instructions
                                                .last()
                                                .and_then(|emitted| emitted.operand(operand))
                                                .map(|operand| operand.virtual_register)
                                                .ok_or(RuntimeSpillError::SourceMismatch)?
                                        }
                                        _ => definition.register,
                                    };
                                    let store = admission::store(
                                        &admitted,
                                        admitted.assignment.store_slot(definition_index),
                                        stored,
                                        &mut next_instruction,
                                        &mut next_register,
                                    )?;
                                    let (registers, sequence) = store.into_streams();
                                    virtual_registers.extend(registers);
                                    instructions.extend(sequence);
                                }
                            }
                        }
                        if active_end + 1 < block.instructions.len() {
                            instructions
                                .extend_from_slice(&block.instructions[active_end + 1..]);
                        }
                    } else if !block.instructions.is_empty() {
                        instructions.extend_from_slice(&block.instructions);
                    }
                    let pre_terminator_len = instructions.len();
                    // Terminator operand uses reload after the last block instruction.
                    // Their pairs occupy the end-of-block gap, so the closing boundary and
                    // instruction positions are pushed only after they are appended.
                    let mut terminator = block.terminator.clone();
                    for operand in &mut super::control_mut(&mut terminator).operands {
                        if operand.virtual_register != register
                            || operand.access != RegisterOperandAccess::Use
                        {
                            continue;
                        }
                        let reload = admission::reload(
                            &admitted,
                            admitted.assignment.end_of_block_slot(block_index),
                            register,
                            &mut next_instruction,
                            &mut next_register,
                        )?;
                        let reloaded = reload.reload_register.id;
                        let (registers, sequence) = reload.into_streams();
                        virtual_registers.extend(registers);
                        instructions.extend(sequence);
                        operand.virtual_register = reloaded;
                    }
                    // Edge-transport arguments read at the same end-of-block position,
                    // after the terminator instruction executes. Their pairs follow any
                    // terminator-operand pairs in successor, then binding, then
                    // case-payload order; only the argument register moves while the
                    // binding or payload keeps its declaration.
                    for successor in super::control_successors_mut(&mut terminator)
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
                                admitted.assignment.end_of_block_slot(block_index),
                                register,
                                &mut next_instruction,
                                &mut next_register,
                            )?;
                            let reloaded = reload.reload_register.id;
                            let (registers, sequence) = reload.into_streams();
                            virtual_registers.extend(registers);
                            instructions.extend(sequence);
                            let SelectedValueTransport::Registers { argument, .. } =
                                &mut binding.transport
                            else {
                                unreachable!()
                            };
                            *argument = reloaded;
                        }
                        // A stored structural-transport argument whose chunk loads share
                        // one register names no new use: the `argument` field follows the
                        // register they read. Where admission recorded they cannot share
                        // it — pinned operands, divergent loads, or a span close at or
                        // after the first load — the binding emits a dedicated
                        // end-of-block pair instead, the same idiom a value binding uses,
                        // so `argument` reads the victim's edge-time value.
                        for binding in &mut successor.structural_bindings {
                            if let SelectedStructuralTransport::ScalarReference { base, .. }
                            | SelectedStructuralTransport::Address { base, .. } =
                                &mut binding.transport
                                && *base
                                    == selected_instructions::SelectedAddressBase::Register(
                                        register,
                                    )
                            {
                                let reload = admission::reload(
                                    &admitted,
                                    admitted.assignment.end_of_block_slot(block_index),
                                    register,
                                    &mut next_instruction,
                                    &mut next_register,
                                )?;
                                let reloaded = reload.reload_register.id;
                                let (registers, sequence) = reload.into_streams();
                                virtual_registers.extend(registers);
                                instructions.extend(sequence);
                                *base = selected_instructions::SelectedAddressBase::Register(
                                    reloaded,
                                );
                                continue;
                            }
                            let Some(byte_size) =
                                super::stored_transport_size(binding.transport)
                            else {
                                continue;
                            };
                            let SelectedStructuralTransport::WholeValue {
                                argument,
                                source_byte_offset,
                                ..
                            } = &mut binding.transport
                            else {
                                continue;
                            };
                            if *argument != register {
                                continue;
                            }
                            let Some(pending) = admitted.structural_uses.iter().find(|pending| {
                                pending.block == block_index
                                    && Some(pending.edge) == successor.psi_edge
                                    && pending.place == binding.semantic.argument.place
                                    && pending.source_byte_offset == *source_byte_offset
                                    && pending.byte_size == byte_size
                            }) else {
                                return Err(RuntimeSpillError::SourceMismatch);
                            };
                            let reloaded = if pending.dedicated_reload {
                                let reload = admission::reload(
                                    &admitted,
                                    admitted.assignment.end_of_block_slot(block_index),
                                    register,
                                    &mut next_instruction,
                                    &mut next_register,
                                )?;
                                let reloaded = reload.reload_register.id;
                                let (registers, sequence) = reload.into_streams();
                                virtual_registers.extend(registers);
                                instructions.extend(sequence);
                                reloaded
                            } else {
                                pending
                                    .loads
                                    .iter()
                                    .try_fold(None, |found: Option<VirtualRegisterId>, load| {
                                        let named = use_reloads
                                            .get(load)
                                            .copied()
                                            .ok_or(RuntimeSpillError::SourceMismatch)?;
                                        match found {
                                            None => Ok(Some(named)),
                                            Some(existing) if existing == named => Ok(found),
                                            _ => Err(RuntimeSpillError::SourceMismatch),
                                        }
                                    })?
                                    .ok_or(RuntimeSpillError::SourceMismatch)?
                            };
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
                                    admitted.assignment.end_of_block_slot(block_index),
                                    register,
                                    &mut next_instruction,
                                    &mut next_register,
                                )?;
                                let reloaded = reload.reload_register.id;
                                let (registers, sequence) = reload.into_streams();
                                virtual_registers.extend(registers);
                                instructions.extend(sequence);
                                let SelectedCasePayloadTransport::Registers { argument, .. } =
                                    &mut payload.transport
                                else {
                                    unreachable!()
                                };
                                *argument = reloaded;
                            }
                        }
                    }
                    let final_len = u32::try_from(instructions.len())
                        .map_err(|_| RuntimeSpillError::IdentityOverflow)?;
                    if has_settlements {
                        boundaries.push(final_len);
                        instruction_positions.push(final_len);
                        for settlement in &mut *boundary_settlements {
                            if settlement.block != block.id {
                                continue;
                            }
                            // Eliminated completions retain their gap; hosted settlements name
                            // the surviving instruction, after any newly inserted reloads.
                            let positions = if matches!(
                                settlement.settlement,
                                SelectedBoundarySettlementPayload::ClaimCompletion(_)
                            ) {
                                &boundaries
                            } else {
                                &instruction_positions
                            };
                            settlement.instruction_index = *positions
                                .get(settlement.instruction_index as usize)
                                .ok_or(RuntimeSpillError::SourceMismatch)?;
                        }
                    }
                    let unchanged_suffix = if instructions.len() == pre_terminator_len {
                        active_window
                            .map_or(block.instructions.len(), |(_, end)| {
                                block.instructions.len() - (end + 1)
                            })
                            .min(block.instructions.len() - unchanged_prefix)
                    } else {
                        0
                    };
                    blocks.replace_block_contents(
                        block_index,
                        instructions,
                        terminator,
                        unchanged_prefix,
                        unchanged_suffix,
                    );
                }
                Ok(())
            },
        )?;
    let sealed = super::validation::seal_runtime_spill_with_span_policy(source, transformed)?;
    Ok((sealed, crossed_unit_write))
}

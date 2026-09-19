use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedLocalStorageSlot, SelectedStructuralTransport,
    SelectedValueTransport, VirtualRegisterId,
};

use super::{
    RuntimeSpillError, RuntimeSpillSpanPolicy, ValidatedRuntimeSpill, admission,
    validate_runtime_spill_with_span_policy,
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
    admitted: &admission::Admission<'_>,
    register: VirtualRegisterId,
    share: bool,
    open: &mut Option<VirtualRegisterId>,
    function: &mut SelectedFunction,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<VirtualRegisterId, RuntimeSpillError> {
    if share && let Some(existing) = *open {
        return Ok(existing);
    }
    let reload = admission::reload(admitted, register, next_instruction, next_register)?;
    let reloaded = reload.reload_register.id;
    let (registers, sequence) = reload.into_streams();
    function.virtual_registers.extend(registers);
    instructions.extend(sequence);
    if share {
        *open = Some(reloaded);
    }
    Ok(reloaded)
}

/// Store one runtime value after its definition and reload before each
/// flexible use. The independently checked output is the only admitted result.
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
    let admitted = admission::admit(
        source,
        function_index,
        register,
        environment,
        span_policy,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    // A reused slot is already declared, so its bytes stay charged exactly
    // once in the function's frame demand; only a private slot is appended.
    if admitted.fresh_slot {
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: admitted.slot,
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
        let mut instructions = Vec::new();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        // An entry parameter's store opens its block: the register is live-in,
        // so position zero is the earliest point the slot holds the value and
        // the only one every use — body, terminator, or edge — follows.
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
            function.virtual_registers.extend(registers);
            instructions.extend(sequence);
        }
        for (instruction_index, original) in block.instructions.iter().enumerate() {
            boundaries.push(
                u32::try_from(instructions.len())
                    .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
            );
            let mut rewritten = original.clone();
            // Uses and `UseDef` operands both read the victim and move to a
            // reload register; a `UseDef` additionally writes that register
            // back, which the instruction's following store then reads. A
            // plain `Def` keeps the victim register so its own store reads
            // the post-write value — possibly bound to a tied use's reload
            // home, which is the read-modify-write idiom's point.
            for operand in &mut rewritten.operands {
                if operand.virtual_register != register
                    || !matches!(
                        operand.access,
                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                    )
                {
                    continue;
                }
                let reloaded = reload_for_use(
                    &admitted,
                    register,
                    shared && operand.fixed_view.is_none(),
                    &mut open_reload,
                    function,
                    &mut instructions,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                use_reloads.entry(rewritten.id).or_insert(reloaded);
                operand.virtual_register = reloaded;
            }
            instruction_positions.push(
                u32::try_from(instructions.len())
                    .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
            );
            instructions.push(rewritten);
            // A clobber or implicit definition may write any unit, including
            // the one hosting the still-open reload, and a redefinition makes
            // the held value stale, so the shared interval ends at each
            // admitted span-closing instruction and the next flexible use
            // opens a fresh pair — unless admission proved a surviving view
            // lets the produced interval cross a unit-writing instruction to
            // its home.
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
                // A `UseDef` write left the new value in the register its
                // operand was redirected to — the emitted instruction's
                // operand names it; the victim register itself was never
                // written. Every other definition stores the recorded
                // register directly.
                let stored = match definition.position {
                    admission::StoragePosition::AfterUseDef { operand, .. } => instructions
                        .last()
                        .and_then(|emitted| {
                            emitted
                                .operands
                                .iter()
                                .find(|candidate| candidate.operand == operand)
                        })
                        .map(|operand| operand.virtual_register)
                        .ok_or(RuntimeSpillError::SourceMismatch)?,
                    _ => definition.register,
                };
                let store =
                    admission::store(&admitted, stored, &mut next_instruction, &mut next_register)?;
                let (registers, sequence) = store.into_streams();
                function.virtual_registers.extend(registers);
                instructions.extend(sequence);
            }
        }
        // Terminator operand uses reload after the last block instruction.
        // Their pairs occupy the end-of-block gap, so the closing boundary and
        // instruction positions are pushed only after they are appended.
        let mut terminator = block.terminator.clone();
        for operand in &mut super::control_mut(&mut terminator).operands {
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
            function.virtual_registers.extend(registers);
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
                    register,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                let reloaded = reload.reload_register.id;
                let (registers, sequence) = reload.into_streams();
                function.virtual_registers.extend(registers);
                instructions.extend(sequence);
                let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport
                else {
                    unreachable!()
                };
                *argument = reloaded;
            }
            // A stored structural-transport argument names no new use: the
            // bridge's snapshot chunk loads already read the reload, so the
            // binding's `argument` field moves to the single register those
            // loads name after rewriting — admission proved they all share
            // it. No reload pair is emitted here.
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
                    return Err(RuntimeSpillError::SourceMismatch);
                };
                let reloaded = pending
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
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
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
                    function.virtual_registers.extend(registers);
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
        boundaries.push(
            u32::try_from(instructions.len()).map_err(|_| RuntimeSpillError::IdentityOverflow)?,
        );
        instruction_positions.push(
            u32::try_from(instructions.len()).map_err(|_| RuntimeSpillError::IdentityOverflow)?,
        );
        for settlement in &mut function.boundary_settlements {
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
        function.blocks[block_index].instructions = instructions;
        function.blocks[block_index].terminator = terminator;
    }
    validate_runtime_spill_with_span_policy(
        source,
        function_index,
        register,
        environment,
        budget,
        transformed,
        span_policy,
    )
}

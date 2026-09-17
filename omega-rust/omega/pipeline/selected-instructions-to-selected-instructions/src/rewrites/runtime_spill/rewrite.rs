use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedLocalStorageSlot,
    SelectedStructuralTransport, SelectedValueTransport, VirtualRegisterId,
};

use super::{RuntimeSpillError, ValidatedRuntimeSpill, admission, validate_runtime_spill};
use crate::ValidatedSelectedAnalysis;

/// Emit one private address/load pair for a use, or — when `share` admits a
/// block-local shared reload — reuse the pair still open from the span's
/// first flexible use so its interval covers every later flexible use in the
/// span. The caller clears the open pair after any instruction that can
/// destroy register content, so the interval never reaches across a call.
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
    function.virtual_registers.push(reload.address_register);
    function.virtual_registers.push(reload.reload_register);
    instructions.push(reload.address);
    instructions.push(reload.load);
    if share {
        *open = Some(reloaded);
    }
    Ok(reloaded)
}

/// Store one runtime value after its definition and reload before each
/// flexible use. The independently checked output is the only admitted result.
pub fn spill_selected_runtime_value(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    let admitted = admission::admit(source, function_index, register, environment, budget)?;
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
        for original in &block.instructions {
            boundaries.push(
                u32::try_from(instructions.len())
                    .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
            );
            let mut rewritten = original.clone();
            for operand in &mut rewritten.operands {
                if operand.virtual_register != register
                    || operand.access != RegisterOperandAccess::Use
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
            // the one hosting the still-open reload, so the shared interval
            // ends here and the next flexible use opens a fresh pair.
            if !original.clobbers.is_empty() || !original.implicit_defs.is_empty() {
                open_reload = None;
            }
            for definition in admitted.definitions.iter().filter(|definition| {
                definition.block_index == block_index && original.id == definition.instruction
            }) {
                instructions.push(admission::instruction(
                    SelectedInstructionId(admission::fresh(&mut next_instruction)?),
                    SelectedInstructionKind::Store64 {
                        slot: admission::frame(admitted.slot),
                        byte_offset: 0,
                    },
                    admitted.store,
                    &[definition.register],
                ));
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
            function.virtual_registers.push(reload.address_register);
            function.virtual_registers.push(reload.reload_register);
            instructions.push(reload.address);
            instructions.push(reload.load);
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
                function.virtual_registers.push(reload.address_register);
                function.virtual_registers.push(reload.reload_register);
                instructions.push(reload.address);
                instructions.push(reload.load);
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
                    function.virtual_registers.push(reload.address_register);
                    function.virtual_registers.push(reload.reload_register);
                    instructions.push(reload.address);
                    instructions.push(reload.load);
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
    validate_runtime_spill(
        source,
        function_index,
        register,
        environment,
        budget,
        transformed,
    )
}

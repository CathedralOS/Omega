//! Snapshot edge inputs before writing any destination-associated transfer value.
use super::{
    SelectedBlockOrigin, SelectedFunction, SelectedSuccessorRole, SelectedValueTransport,
    VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::edge_transfers::instruction_count;
use crate::selection::edge_transfers::invalid;
use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedSelectionConstraints, SelectedTerminator,
    VirtualRegister, VirtualRegisterOrigin, terminator_successors_mut,
};
mod structural_case;

pub(in crate::selection) fn prepare(
    function_index: usize,
    function: &mut SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    mut edge_work: Vec<super::EdgeWork>,
) -> Result<(), SelectedInstructionError> {
    let source_count = function.blocks.len();
    // Detached cleanup rows already own IDs and call/memory contracts. Reserve
    // them before allocating snapshots, copies, or continuation instructions.
    let mut next_instruction = instruction_count(function)
        + edge_work
            .iter()
            .map(|work| work.preparation.len() + work.cleanup.len())
            .sum::<usize>();
    let mut bridges = Vec::new();
    let mut unbridged_fallthroughs = Vec::new();
    // A conditional instruction must physically precede its zero successor.
    // Give any edge that would close a fallthrough cycle, enter the function
    // entry, or claim an already-used destination its own bridge block.
    let mut fallthrough_next = Vec::<(SelectedBlockId, SelectedBlockId)>::new();
    let entry_block = function.entry_block;
    for source in &mut function.blocks {
        let conditional = matches!(
            source.terminator,
            SelectedTerminator::ConditionalBranch { .. }
                | selected_instructions::conditional_branch_less_pair_pattern!(..)
        );
        for (position, successor) in terminator_successors_mut(&mut source.terminator)
            .into_iter()
            .enumerate()
        {
            let work = edge_work
                .iter()
                .position(|work| {
                    successor.role == SelectedSuccessorRole::Semantic
                        && work.edge == successor.psi_edge
                })
                .map(|position| edge_work.remove(position));
            let (preparation, mut cleanup) = work.map_or_else(
                || (Vec::new(), Vec::new()),
                |work| (work.preparation, work.cleanup),
            );
            if successor.structural_case.is_some() {
                if !preparation.is_empty() {
                    return Err(invalid(function_index));
                }
                if let Some(bridge) = structural_case::prepare(
                    function_index,
                    successor,
                    source_count + bridges.len(),
                    &mut function.virtual_registers,
                    &mut function.memory_accesses,
                    &function.local_storage_slots,
                    &mut next_instruction,
                    constraints,
                    catalog,
                    environment,
                    cleanup.clone(),
                )? {
                    bridges.push(bridge);
                    continue;
                }
                // A case with no payload work still needs the same physical
                // fallthrough ordering as every other conditional edge.
            }
            let active = successor
                .bindings
                .iter()
                .filter_map(|binding| match binding.transport {
                    SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => Some((binding.semantic, argument, parameter)),
                    SelectedValueTransport::Unused => None,
                })
                .collect::<Vec<_>>();
            if preparation.is_empty()
                && cleanup.is_empty()
                && active.is_empty()
                && successor.structural_bindings.iter().all(|binding| {
                    binding.transport == selected_instructions::SelectedStructuralTransport::Unused
                })
            {
                if !conditional || position != 1 {
                    continue;
                }
                // Two conditional instructions cannot both physically precede
                // one fallthrough block. Reuse the ordinary edge bridge for the
                // later claimant; existing transport bridges already isolate it.
                let mut current = successor.block;
                let mut closes_cycle = current == source.id;
                while !closes_cycle {
                    let Some((_, next)) = fallthrough_next
                        .iter()
                        .find(|(origin, _)| *origin == current)
                    else {
                        break;
                    };
                    current = *next;
                    closes_cycle = current == source.id;
                }
                if successor.block != entry_block
                    && !unbridged_fallthroughs.contains(&successor.block)
                    && !closes_cycle
                {
                    unbridged_fallthroughs.push(successor.block);
                    fallthrough_next.push((source.id, successor.block));
                    continue;
                }
            }
            let bridge_id = SelectedBlockId(
                u32::try_from(source_count + bridges.len()).map_err(|_| invalid(function_index))?,
            );
            let mut continuation = successor.clone();
            continuation.role = SelectedSuccessorRole::EdgeTransferContinuation;
            continuation.fuel.clear();
            let mut instructions = preparation;
            let mut snapshots = Vec::new();
            let mut transfers = Vec::new();
            let mut descriptors = Vec::new();
            let mut scalar_references = Vec::new();
            for phase in 0..2 {
                if phase == 1 {
                    descriptors = super::descriptors::snapshot(
                        function_index,
                        &mut function.virtual_registers,
                        &mut function.memory_accesses,
                        &mut function.local_storage_slots,
                        &mut instructions,
                        &mut next_instruction,
                        successor.psi_edge,
                        &successor.structural_bindings,
                        constraints,
                        catalog,
                        environment,
                    )?;
                    scalar_references = super::scalar_references::snapshot(
                        function_index,
                        &mut function.virtual_registers,
                        &mut function.memory_accesses,
                        &mut instructions,
                        &mut next_instruction,
                        successor.psi_edge,
                        &successor.structural_bindings,
                        constraints,
                        catalog,
                    )?;
                    // All outgoing values are now snapshots. Hooks can clobber
                    // registers but cannot observe partially installed bindings.
                    instructions.append(&mut cleanup);
                }
                for (position, (semantic, argument, _)) in active.iter().enumerate() {
                    let input = if phase == 0 {
                        *argument
                    } else {
                        snapshots[position]
                    };
                    let source_register = function
                        .virtual_registers
                        .get(argument.0 as usize)
                        .cloned()
                        .ok_or_else(|| invalid(function_index))?;
                    let output = VirtualRegisterId(
                        u32::try_from(function.virtual_registers.len())
                            .map_err(|_| invalid(function_index))?,
                    );
                    let instruction_id = SelectedInstructionId(
                        u32::try_from(next_instruction).map_err(|_| invalid(function_index))?,
                    );
                    next_instruction += 1;
                    function.virtual_registers.push(VirtualRegister {
                        id: output,
                        scalar_type: source_register.scalar_type,
                        class: source_register.class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: instruction_id,
                            source_value: semantic.argument,
                        },
                        definition_site: source_register.definition_site,
                        entry_fixed_view: None,
                    });
                    instructions.push(super::super::constraints::instruction(
                        instruction_id,
                        SelectedInstructionKind::CopyI64,
                        constraints.keys.copy_i64,
                        &[input, output],
                        SelectedInstructionProvenance {
                            values: vec![semantic.argument],
                            ..Default::default()
                        },
                        catalog,
                    )?);
                    if phase == 0 {
                        snapshots.push(output);
                    } else {
                        transfers.push(output);
                    }
                }
            }
            super::descriptors::store(
                function_index,
                &mut function.virtual_registers,
                &mut function.memory_accesses,
                &mut instructions,
                &mut next_instruction,
                successor.psi_edge,
                &successor.structural_bindings,
                &descriptors,
                constraints,
                catalog,
                environment,
            )?;
            super::scalar_references::store(
                function_index,
                &mut function.virtual_registers,
                &mut function.memory_accesses,
                &mut instructions,
                &mut next_instruction,
                successor.psi_edge,
                &scalar_references,
                constraints,
                catalog,
            )?;
            super::addresses::store(
                function_index,
                &mut function.virtual_registers,
                &mut function.memory_accesses,
                &mut instructions,
                &mut next_instruction,
                successor.psi_edge,
                &successor.structural_bindings,
                constraints,
                catalog,
            )?;
            let mut position = 0;
            for binding in &mut continuation.bindings {
                if let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport {
                    *argument = transfers[position];
                    position += 1;
                }
            }
            let jump_id = SelectedInstructionId(
                u32::try_from(next_instruction).map_err(|_| invalid(function_index))?,
            );
            next_instruction += 1;
            let jump = super::super::constraints::instruction(
                jump_id,
                SelectedInstructionKind::Jump,
                constraints.keys.jump,
                &[],
                SelectedInstructionProvenance::default(),
                catalog,
            )?;
            bridges.push(SelectedBlock {
                id: bridge_id,
                origin: SelectedBlockOrigin::EdgeTransfer {
                    edge: successor.psi_edge,
                    target: successor.source_target,
                },
                instructions,
                terminator: SelectedTerminator::Jump {
                    instruction: jump,
                    successor: continuation,
                },
            });
            successor.block = bridge_id;
            for binding in &mut successor.bindings {
                binding.transport = SelectedValueTransport::Unused;
            }
            for binding in &mut successor.structural_bindings {
                binding.transport = selected_instructions::SelectedStructuralTransport::Unused;
            }
        }
    }
    if !edge_work.is_empty() {
        return Err(invalid(function_index));
    }
    function.blocks.extend(bridges);
    Ok(())
}

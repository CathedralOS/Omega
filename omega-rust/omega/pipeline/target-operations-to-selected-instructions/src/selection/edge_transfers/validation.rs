//! Check the claimed expansion in place, then recover the original selected CFG.
use super::*;
mod structural_case;

pub(in crate::selection) fn project(
    function_index: usize,
    prepared: &SelectedFunction,
    constraints: &SelectedSelectionConstraints,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let error = || invalid(function_index);
    structural_case::validate_prepared_states(function_index, prepared)?;
    let source_count = prepared
        .blocks
        .iter()
        .take_while(|block| matches!(block.origin, SelectedBlockOrigin::Source(_)))
        .count();
    if source_count == 0 {
        return Err(error());
    }
    let extra_registers =
        prepared.blocks[source_count..]
            .iter()
            .try_fold(0usize, |count, block| {
                count
                    .checked_add(
                        block
                            .instructions
                            .iter()
                            .filter(|instruction| {
                                matches!(
                                    instruction.kind,
                                    SelectedInstructionKind::CopyI64
                                        | SelectedInstructionKind::Load64 { .. }
                                        | SelectedInstructionKind::Load32 { .. }
                                        | SelectedInstructionKind::FrameAddress { .. }
                                )
                            })
                            .count(),
                    )
                    .ok_or_else(error)
            })?;
    let register_count = prepared
        .virtual_registers
        .len()
        .checked_sub(extra_registers)
        .ok_or_else(error)?;
    let mut projected = prepared.clone();
    projected.blocks.truncate(source_count);
    projected.virtual_registers.truncate(register_count);
    let memory_count = projected
        .memory_accesses
        .iter()
        .take_while(|access| {
            !matches!(
                access.origin,
                selected_instructions::SelectedMemoryAccessOrigin::Edge(_)
            )
        })
        .count();
    projected.memory_accesses.truncate(memory_count);
    let mut descriptor_accesses = Vec::new();
    let mut next_instruction = instruction_count(&projected);
    let mut next_register = register_count;
    let mut next_bridge = source_count;
    for source in &mut projected.blocks {
        for successor in successors_mut(&mut source.terminator) {
            if successor.role != SelectedSuccessorRole::Semantic {
                return Err(error());
            }
            if (successor.block.0 as usize) < source_count {
                if let Some(case) = &successor.structural_case
                    && (!successor.bindings.is_empty()
                        || !successor.structural_bindings.is_empty()
                        || case.payloads.iter().any(|payload| {
                            payload.transport
                                != selected_instructions::SelectedCasePayloadTransport::Unused
                        }))
                {
                    return Err(error());
                }
                if !successor.structural_bindings.is_empty()
                    || successor.bindings.iter().any(|binding| {
                        matches!(binding.transport, SelectedValueTransport::Registers { .. })
                    })
                {
                    return Err(error());
                }
                continue;
            }
            if successor.structural_case.is_some() {
                let (register_delta, instruction_delta, accesses) = structural_case::project(
                    function_index,
                    prepared,
                    successor,
                    constraints,
                    source_count,
                    next_bridge,
                    next_instruction,
                    next_register,
                    register_count,
                )?;
                descriptor_accesses.extend(accesses);
                next_instruction = next_instruction
                    .checked_add(instruction_delta)
                    .ok_or_else(error)?;
                next_register = next_register
                    .checked_add(register_delta)
                    .ok_or_else(error)?;
                next_bridge += 1;
                continue;
            }
            let bridge = prepared.blocks.get(next_bridge).ok_or_else(error)?;
            if successor.block.0 as usize != next_bridge
                || bridge.id != successor.block
                || bridge.origin
                    != (SelectedBlockOrigin::EdgeTransfer {
                        edge: successor.psi_edge,
                        target: successor.source_target,
                    })
                || successor
                    .bindings
                    .iter()
                    .any(|binding| binding.transport != SelectedValueTransport::Unused)
                || successor.structural_bindings.iter().any(|binding| {
                    binding.transport != selected_instructions::SelectedStructuralTransport::Unused
                })
            {
                return Err(error());
            }
            let SelectedTerminator::Jump {
                instruction: jump,
                successor: continuation,
            } = &bridge.terminator
            else {
                return Err(error());
            };
            if continuation.role != SelectedSuccessorRole::EdgeTransferContinuation
                || continuation.structural_case.is_some()
                || continuation.psi_edge != successor.psi_edge
                || continuation.source_target != successor.source_target
                || continuation.block.0 as usize >= source_count
                || !continuation.fuel.is_empty()
                || continuation.bindings.len() != successor.bindings.len()
                || continuation.structural_bindings.len() != successor.structural_bindings.len()
                || !continuation
                    .structural_bindings
                    .iter()
                    .zip(&successor.structural_bindings)
                    .all(|(left, right)| left.semantic == right.semantic)
                || !continuation
                    .bindings
                    .iter()
                    .zip(&successor.bindings)
                    .all(|(left, right)| left.semantic == right.semantic)
            {
                return Err(error());
            }
            let active = continuation
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
            let descriptor_words = continuation
                .structural_bindings
                .len()
                .checked_mul(2)
                .ok_or_else(error)?;
            let register_delta = active
                .len()
                .checked_mul(2)
                .and_then(|count| count.checked_add(descriptor_words))
                .ok_or_else(error)?;
            if (active.is_empty() && descriptor_words == 0)
                || register_delta.checked_add(descriptor_words) != Some(bridge.instructions.len())
            {
                return Err(error());
            }
            descriptor_accesses.extend(super::descriptor_validation::check(
                function_index,
                prepared,
                bridge,
                continuation,
                active.len(),
                next_instruction,
                next_register,
                register_count,
                constraints,
            )?);
            let mut originals = Vec::new();
            for (position, (semantic, transfer, _)) in active.iter().enumerate() {
                let snapshot = &bridge.instructions[position];
                let copied = &bridge.instructions[position + active.len() + descriptor_words];
                let [input, output] = snapshot.operands.as_slice() else {
                    return Err(error());
                };
                let original = input.virtual_register;
                if original.0 as usize >= register_count {
                    return Err(error());
                }
                let original_row = prepared
                    .virtual_registers
                    .get(original.0 as usize)
                    .ok_or_else(error)?;
                if original_row.scalar_type != semantic.scalar_type {
                    return Err(error());
                }
                check_copy(
                    function_index,
                    prepared,
                    snapshot,
                    next_instruction + position,
                    next_register + position,
                    original,
                    original_row,
                    semantic.argument,
                    constraints,
                )?;
                check_copy(
                    function_index,
                    prepared,
                    copied,
                    next_instruction + active.len() + descriptor_words + position,
                    next_register + active.len() + descriptor_words + position,
                    output.virtual_register,
                    original_row,
                    semantic.argument,
                    constraints,
                )?;
                if copied.operands[1].virtual_register != *transfer {
                    return Err(error());
                }
                originals.push(original);
            }
            let jump_identity = next_instruction
                .checked_add(bridge.instructions.len())
                .ok_or_else(error)?;
            if usize::try_from(jump.id.0).ok() != Some(jump_identity)
                || jump.kind != SelectedInstructionKind::Jump
                || jump.constraint != constraints.keys.jump
                || !jump.operands.is_empty()
                || jump.provenance != SelectedInstructionProvenance::default()
            {
                return Err(error());
            }
            successor.block = continuation.block;
            successor.structural_bindings = continuation.structural_bindings.clone();
            let mut active_position = 0;
            for (binding, retained) in successor.bindings.iter_mut().zip(&continuation.bindings) {
                binding.transport = match retained.transport {
                    SelectedValueTransport::Registers { parameter, .. } => {
                        let argument = originals[active_position];
                        active_position += 1;
                        SelectedValueTransport::Registers {
                            argument,
                            parameter,
                        }
                    }
                    SelectedValueTransport::Unused => SelectedValueTransport::Unused,
                };
            }
            next_instruction = jump_identity.checked_add(1).ok_or_else(error)?;
            next_register = next_register
                .checked_add(register_delta)
                .ok_or_else(error)?;
            next_bridge += 1;
        }
    }
    if next_bridge != prepared.blocks.len()
        || next_register != prepared.virtual_registers.len()
        || prepared.memory_accesses[memory_count..] != descriptor_accesses
    {
        return Err(error());
    }
    Ok(projected)
}

fn check_copy(
    function: usize,
    prepared: &SelectedFunction,
    copy: &SelectedInstruction,
    instruction: usize,
    register: usize,
    input: VirtualRegisterId,
    original: &VirtualRegister,
    value: semantic_vocabulary::ValueId,
    constraints: &SelectedSelectionConstraints,
) -> Result<(), SelectedInstructionError> {
    let error = || invalid(function);
    let [used, defined] = copy.operands.as_slice() else {
        return Err(error());
    };
    let output = prepared.virtual_registers.get(register).ok_or_else(error)?;
    if copy.id.0 as usize != instruction
        || copy.kind != SelectedInstructionKind::CopyI64
        || copy.constraint != constraints.keys.copy_i64
        || used.virtual_register != input
        || defined.virtual_register.0 as usize != register
        || output.id != defined.virtual_register
        || output.scalar_type != original.scalar_type
        || output.class != original.class
        || output.definition_site != original.definition_site
        || output.entry_fixed_view.is_some()
        || output.origin
            != (VirtualRegisterOrigin::InstructionResult {
                instruction: copy.id,
                source_value: value,
            })
        || copy.provenance
            != (SelectedInstructionProvenance {
                values: vec![value],
                ..Default::default()
            })
    {
        return Err(error());
    }
    Ok(())
}

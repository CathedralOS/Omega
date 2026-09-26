//! Receive edge-local case loads before recovering the unmaterialized source edge.
use super::super::SelectedSuccessor;
use super::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedSelectionConstraints, SelectedSuccessorRole,
    SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selected_instructions::{
    FrameStorageSlotId, SelectedCasePayloadTransport as Transport, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};
use crate::selection::edge_transfers::invalid;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

pub(super) fn validate_prepared_states(
    function: usize,
    prepared: &SelectedFunction,
) -> Result<(), SelectedInstructionError> {
    for block in &prepared.blocks {
        let edges = match &block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            _ => Vec::new(),
        };
        if edges
            .iter()
            .filter_map(|edge| edge.structural_case.as_ref())
            .flat_map(|case| &case.payloads)
            .any(|payload| matches!(payload.transport, Transport::Unmaterialized { .. }))
        {
            return Err(invalid(function));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    function: usize,
    prepared: &SelectedFunction,
    successor: &mut SelectedSuccessor,
    constraints: &SelectedSelectionConstraints,
    source_count: usize,
    bridge_index: usize,
    instruction_start: usize,
    register_start: usize,
    source_register_count: usize,
) -> Result<(usize, usize, Vec<SelectedMemoryAccess>), SelectedInstructionError> {
    let error = || invalid(function);
    let case = successor.structural_case.as_ref().ok_or_else(error)?;
    let bridge = prepared.blocks.get(bridge_index).ok_or_else(error)?;
    if successor.block.0 as usize != bridge_index
        || bridge.id != successor.block
        || bridge.origin
            != (SelectedBlockOrigin::EdgeTransfer {
                edge: successor.psi_edge,
                target: successor.source_target,
            })
        || !successor.bindings.is_empty()
        || !successor.structural_bindings.is_empty()
        || case
            .payloads
            .iter()
            .any(|payload| payload.transport != Transport::Unused)
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
    let retained = continuation.structural_case.as_ref().ok_or_else(error)?;
    if continuation.role != SelectedSuccessorRole::EdgeTransferContinuation
        || continuation.psi_edge != successor.psi_edge
        || continuation.source_target != successor.source_target
        || continuation.block.0 as usize >= source_count
        || !continuation.fuel.is_empty()
        || !continuation.bindings.is_empty()
        || !continuation.structural_bindings.is_empty()
        || retained.source != case.source
        || retained.case != case.case
        || retained.case_tag != case.case_tag
        || !retained.trivial_affine_discards.is_empty()
        || retained.payloads.len() != case.payloads.len()
        || retained
            .payloads
            .iter()
            .zip(&case.payloads)
            .any(|(left, right)| left.semantic != right.semantic)
    {
        return Err(error());
    }
    let (place, byte_bound) = match case.source {
        selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
            let mut slots = prepared
                .local_storage_slots
                .iter()
                .filter(|row| row.id == slot);
            let slot = slots.next().ok_or_else(error)?;
            if slots.next().is_some() {
                return Err(error());
            }
            (
                slot.id.structural_place().ok_or_else(error)?,
                slot.byte_size,
            )
        }
        selected_instructions::SelectedCaseDispatchSource::Borrowed {
            place, byte_size, ..
        } => (place, byte_size),
    };
    let integer = |sign, bits| {
        IntegerType::new(sign, bits)
            .map(ScalarType::Integer)
            .map_err(|_| error())
    };
    let pointer_type = integer(IntegerSign::Unsigned, 64)?;
    // Cleanup belongs to the semantic source leg; projection restores that
    // roster, never the deliberately empty implementation continuation roster.
    let mut projected = case.clone();
    let mut accesses = Vec::new();
    let mut used = 0usize;
    // A local source addresses its slot per payload; a borrowed source's
    // referent pointer is already a register, so each payload is one load.
    let per_payload = match case.source {
        selected_instructions::SelectedCaseDispatchSource::Local { .. } => 2usize,
        selected_instructions::SelectedCaseDispatchSource::Borrowed { .. } => 1usize,
    };
    for (payload, output) in retained.payloads.iter().zip(&mut projected.payloads) {
        match payload.transport {
            Transport::Unused => {
                output.transport = Transport::Unused;
                continue;
            }
            Transport::Unmaterialized { .. } => return Err(error()),
            Transport::Registers {
                argument,
                parameter,
            } => {
                let payload_type = payload.semantic.parameter.scalar_type;
                let ScalarType::Integer(integer) = payload_type else {
                    return Err(error());
                };
                let (load_kind, load_key, byte_count) = match integer.bits() {
                    32 => (
                        SelectedInstructionKind::Load32 {
                            byte_offset: payload.semantic.field_byte_offset,
                        },
                        constraints.keys.load32,
                        4,
                    ),
                    64 => (
                        SelectedInstructionKind::Load64 {
                            byte_offset: payload.semantic.field_byte_offset,
                        },
                        constraints.keys.load64,
                        8,
                    ),
                    _ => return Err(error()),
                };
                let load_index = used * per_payload;
                let register_index = register_start.checked_add(load_index).ok_or_else(error)?;
                let destination = prepared
                    .virtual_registers
                    .get(parameter.0 as usize)
                    .ok_or_else(error)?;
                let provenance = SelectedInstructionProvenance {
                    edges: vec![successor.psi_edge],
                    ..Default::default()
                };
                let (pointer, loaded, load) = match case.source {
                    selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                        let instruction_index = instruction_start
                            .checked_add(load_index)
                            .ok_or_else(error)?;
                        let address = bridge.instructions.get(load_index).ok_or_else(error)?;
                        let load = bridge.instructions.get(load_index + 1).ok_or_else(error)?;
                        let pointer = prepared
                            .virtual_registers
                            .get(register_index)
                            .ok_or_else(error)?;
                        let loaded = prepared
                            .virtual_registers
                            .get(register_index + 1)
                            .ok_or_else(error)?;
                        if pointer.id.0 as usize != register_index
                            || loaded.id.0 as usize != register_index + 1
                            || pointer.origin
                                != (VirtualRegisterOrigin::AbiTransport {
                                    instruction: address.id,
                                    place,
                                    byte_offset: 0,
                                })
                        {
                            return Err(error());
                        }
                        check_instruction(
                            address,
                            instruction_index,
                            SelectedInstructionKind::FrameAddress {
                                slot: FrameStorageSlotId::Local(slot),
                                byte_offset: 0,
                            },
                            constraints.keys.frame_address.ok_or_else(error)?,
                            &[pointer.id],
                            &provenance,
                            function,
                        )?;
                        accesses.push(SelectedMemoryAccess {
                            instruction: address.id,
                            origin: SelectedMemoryAccessOrigin::Edge(successor.psi_edge),
                            place,
                            byte_offset: 0,
                            byte_count: prepared
                                .local_storage_slots
                                .iter()
                                .find(|row| row.id == slot)
                                .map(|row| row.byte_size)
                                .ok_or_else(error)?,
                            role: SelectedMemoryAccessRole::AddressLocal { slot },
                        });
                        (pointer, loaded, load)
                    }
                    selected_instructions::SelectedCaseDispatchSource::Borrowed {
                        pointer: borrow_pointer,
                        ..
                    } => {
                        let load = bridge.instructions.get(load_index).ok_or_else(error)?;
                        let pointer = prepared
                            .virtual_registers
                            .get(borrow_pointer.0 as usize)
                            .ok_or_else(error)?;
                        let loaded = prepared
                            .virtual_registers
                            .get(register_index)
                            .ok_or_else(error)?;
                        if pointer.id != borrow_pointer
                            || loaded.id.0 as usize != register_index
                            || !matches!(pointer.origin,
                                VirtualRegisterOrigin::AbiTransport {
                                    instruction,
                                    place: origin_place,
                                    byte_offset: 0,
                                } if instruction.0 as usize <= instruction_start
                                    && origin_place == place)
                        {
                            return Err(error());
                        }
                        (pointer, loaded, load)
                    }
                };
                if parameter.0 as usize >= source_register_count
                    || destination.scalar_type != payload_type
                    || payload.semantic.parameter.scalar_type != payload_type
                    || destination.definition_site
                        != Some(payload.semantic.parameter.definition_site)
                    || !matches!(destination.origin, VirtualRegisterOrigin::BlockParameter {source_value, block, ..}
                        if source_value == payload.semantic.parameter.value && block == continuation.block)
                    || payload
                        .semantic
                        .field_byte_offset
                        .checked_add(byte_count)
                        .is_none_or(|end| end > byte_bound)
                    || payload.semantic.field_byte_offset % byte_count != 0
                    || loaded.id != argument
                    || pointer.scalar_type != pointer_type
                    || loaded.scalar_type != payload_type
                    || pointer.class != destination.class
                    || loaded.class != destination.class
                    || pointer.definition_site.is_some()
                    || loaded.definition_site.is_some()
                    || pointer.entry_fixed_view.is_some()
                    || loaded.entry_fixed_view.is_some()
                    || loaded.origin
                        != (VirtualRegisterOrigin::StructuralObservation {
                            instruction: load.id,
                            place,
                            byte_offset: payload.semantic.field_byte_offset,
                        })
                {
                    return Err(error());
                }
                check_instruction(
                    load,
                    instruction_start + load_index + (per_payload - 1),
                    load_kind,
                    load_key.ok_or_else(error)?,
                    &[pointer.id, loaded.id],
                    &provenance,
                    function,
                )?;
                accesses.push(SelectedMemoryAccess {
                    instruction: load.id,
                    origin: SelectedMemoryAccessOrigin::Edge(successor.psi_edge),
                    place,
                    byte_offset: payload.semantic.field_byte_offset,
                    byte_count,
                    role: SelectedMemoryAccessRole::ReadPlace,
                });
                output.transport = Transport::Unmaterialized { parameter };
                used += 1;
            }
        }
    }
    if used == 0 || bridge.instructions.len() != used * per_payload {
        return Err(error());
    }
    check_instruction(
        jump,
        instruction_start + used * per_payload,
        SelectedInstructionKind::Jump,
        constraints.keys.jump,
        &[],
        &Default::default(),
        function,
    )?;
    successor.block = continuation.block;
    successor.structural_case = Some(projected);
    Ok((used * per_payload, used * per_payload + 1, accesses))
}

fn check_instruction(
    actual: &SelectedInstruction,
    position: usize,
    kind: SelectedInstructionKind,
    key: crate::register_model::RegisterConstraintKey,
    registers: &[VirtualRegisterId],
    provenance: &SelectedInstructionProvenance,
    function: usize,
) -> Result<(), SelectedInstructionError> {
    if actual.id.0 as usize != position
        || actual.kind != kind
        || actual.constraint != key
        || actual.provenance != *provenance
        || actual
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .ne(registers.iter().copied())
    {
        return Err(invalid(function));
    }
    Ok(())
}

/*
We construct and emit block-edge value transfers, structural-case branch targets,
descriptor copies, and scalar reference edge arguments for selected control-flow
edges.
*/

use crate::selection::model::SelectedInstructionError;
use selected_instructions::{SelectedBlockOrigin, SelectedSuccessorRole, SelectedValueTransport};
use selected_instructions::{SelectedFunction, VirtualRegisterId};
pub(crate) mod addresses {
    use super::emission::Emission;
    use crate::SelectedInstructionError;
    use crate::selection::edge_transfers::invalid;
    use selected_instructions::{
        SelectedAddressBase, SelectedInstructionKind, SelectedMemoryAccessRole,
        SelectedStructuralBinding, SelectedStructuralTransport,
    };

    type Address = (
        SelectedAddressBase,
        u32,
        u32,
        selected_instructions::LocalStorageSlotId,
    );

    fn address(transport: SelectedStructuralTransport) -> Option<Address> {
        match transport {
            SelectedStructuralTransport::Address {
                base,
                byte_offset,
                byte_count,
                destination,
            } => Some((base, byte_offset, byte_count, destination)),
            SelectedStructuralTransport::Unused
            | SelectedStructuralTransport::WholeValue { .. }
            | SelectedStructuralTransport::ScalarReference { .. } => None,
        }
    }

    fn computed(base: SelectedAddressBase, byte_offset: u32) -> bool {
        matches!(base, SelectedAddressBase::Local(_)) || byte_offset != 0
    }

    pub(crate) fn store(
        emission: &mut Emission<'_>,
        bindings: &[SelectedStructuralBinding],
    ) -> Result<(), SelectedInstructionError> {
        let function = emission.function;
        let error = || invalid(function);
        for binding in bindings {
            let Some((base, byte_offset, byte_count, destination)) = address(binding.transport)
            else {
                continue;
            };
            let place = binding.semantic.argument.place;
            let pointer = if computed(base, byte_offset) {
                let (kind, key, inputs, access) = match base {
                    SelectedAddressBase::Local(slot) => (
                        SelectedInstructionKind::FrameAddress {
                            slot: selected_instructions::FrameStorageSlotId::Local(slot),
                            byte_offset,
                        },
                        emission.constraints.keys.frame_address.ok_or_else(error)?,
                        Vec::new(),
                        Some((byte_count, SelectedMemoryAccessRole::AddressLocal { slot })),
                    ),
                    SelectedAddressBase::Register(register) => (
                        SelectedInstructionKind::AddressOffset { byte_offset },
                        emission.constraints.keys.address_offset.ok_or_else(error)?,
                        vec![register],
                        None,
                    ),
                };
                emission.emit(kind, key, &inputs, place, byte_offset, access)?
            } else {
                let SelectedAddressBase::Register(register) = base else {
                    return Err(error());
                };
                register
            };
            emission.store_local_word(destination, 0, pointer, binding.semantic.parameter)?;
        }
        Ok(())
    }
}
pub(crate) mod construction {
    use super::emission::Emission;
    use super::{
        SelectedBlockOrigin, SelectedFunction, SelectedSuccessorRole, SelectedValueTransport,
        VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::value_provenance;
    use crate::selection::edge_transfers::instruction_count;
    use crate::selection::edge_transfers::invalid;
    use register_model::PreparedRegisterConstraintCatalog;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance, SelectedSelectionConstraints, SelectedTerminator,
        VirtualRegister, VirtualRegisterOrigin, terminator_successors_mut,
    };
    pub(crate) mod structural_case {
        use super::{
            PreparedRegisterConstraintCatalog, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, SelectedSelectionConstraints, VirtualRegister,
            VirtualRegisterId, VirtualRegisterOrigin,
        };
        use crate::SelectedInstructionError;
        use crate::selection::scalar_call_abi::load_transfer;
        use crate::selection::scalar_call_abi::{integer_carrier_normalization, scalar_shape};
        use selected_instructions::SelectedSuccessor;
        use selected_instructions::{
            FrameStorageSlotId, SelectedCasePayloadTransport, SelectedLocalStorageSlot,
            SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
        };
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn snapshot(
            function: usize,
            successor: &mut SelectedSuccessor,
            registers: &mut Vec<VirtualRegister>,
            memory: &mut Vec<SelectedMemoryAccess>,
            slots: &[SelectedLocalStorageSlot],
            next_instruction: &mut usize,
            constraints: &SelectedSelectionConstraints,
            catalog: &PreparedRegisterConstraintCatalog,
            environment: &register_environment::ValidatedTargetRegisterEnvironment,
            instructions: &mut Vec<selected_instructions::SelectedInstruction>,
        ) -> Result<(), SelectedInstructionError> {
            let invalid = || super::super::invalid(function);
            let psi_edge = successor.psi_edge.ok_or_else(invalid)?;
            let case = successor.structural_case.as_mut().ok_or_else(invalid)?;
            if !successor.bindings.is_empty() {
                return Err(invalid());
            }
            let (place, byte_bound) = match case.source {
                selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                    let slot = slots
                        .iter()
                        .find(|row| row.id == slot)
                        .ok_or_else(invalid)?;
                    (
                        slot.id.structural_place().ok_or_else(invalid)?,
                        slot.byte_size,
                    )
                }
                selected_instructions::SelectedCaseDispatchSource::Borrowed {
                    place,
                    byte_size,
                    ..
                } => (place, byte_size),
            };
            let source = case.source;
            for payload in &mut case.payloads {
                let parameter = match payload.transport {
                    SelectedCasePayloadTransport::Unused => continue,
                    SelectedCasePayloadTransport::Unmaterialized { parameter } => parameter,
                    SelectedCasePayloadTransport::Registers { .. } => return Err(invalid()),
                };
                let destination = registers
                    .get(parameter.0 as usize)
                    .cloned()
                    .ok_or_else(invalid)?;
                let shape =
                    scalar_shape(payload.semantic.parameter.scalar_type).ok_or_else(invalid)?;
                let (load_key, byte_count) = match shape.byte_size {
                    1 => (constraints.keys.load8, 1),
                    2 => (constraints.keys.load16, 2),
                    4 => (constraints.keys.load32, 4),
                    8 => (constraints.keys.load64, 8),
                    _ => return Err(invalid()),
                };
                if destination.scalar_type != payload.semantic.parameter.scalar_type
                    || destination.definition_site
                        != Some(payload.semantic.parameter.definition_site)
                    || payload
                        .semantic
                        .field_byte_offset
                        .checked_add(byte_count)
                        .is_none_or(|end| end > byte_bound)
                    || payload.semantic.field_byte_offset % byte_count != 0
                {
                    return Err(invalid());
                }
                let provenance = || SelectedInstructionProvenance {
                    edges: vec![psi_edge],
                    ..Default::default()
                };
                let pointer = match source {
                    selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                        let slot = slots
                            .iter()
                            .find(|row| row.id == slot)
                            .ok_or_else(invalid)?;
                        let address_instruction = SelectedInstructionId(
                            (*next_instruction).try_into().map_err(|_| invalid())?,
                        );
                        *next_instruction += 1;
                        let pointer =
                            VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
                        registers.push(VirtualRegister {
                            id: pointer,
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64)
                                    .map_err(|_| invalid())?,
                            ),
                            class: destination.class,
                            origin: VirtualRegisterOrigin::AbiTransport {
                                instruction: address_instruction,
                                place,
                                byte_offset: 0,
                            },
                            definition_site: None,
                            entry_fixed_view: None,
                        });
                        instructions.push(crate::selection::constraints::instruction(
                            address_instruction,
                            SelectedInstructionKind::FrameAddress {
                                slot: FrameStorageSlotId::Local(slot.id),
                                byte_offset: 0,
                            },
                            constraints.keys.frame_address.ok_or_else(invalid)?,
                            &[pointer],
                            provenance(),
                            catalog,
                        )?);
                        memory.push(SelectedMemoryAccess {
                            instruction: address_instruction,
                            origin: SelectedMemoryAccessOrigin::Edge(psi_edge),
                            place,
                            byte_offset: 0,
                            byte_count: slot.byte_size,
                            role: SelectedMemoryAccessRole::AddressLocal { slot: slot.id },
                        });
                        pointer
                    }
                    selected_instructions::SelectedCaseDispatchSource::Borrowed {
                        pointer, ..
                    } => {
                        if registers.get(pointer.0 as usize).is_none_or(|register| {
                            register.scalar_type
                                != ScalarType::Integer(
                                    IntegerType::new(IntegerSign::Unsigned, 64)
                                        .expect("u64 pointer type"),
                                )
                        }) {
                            return Err(invalid());
                        }
                        pointer
                    }
                };
                let (pointer, displacement) = if environment
                    .admits_pointer_displacement(payload.semantic.field_byte_offset, byte_count)
                {
                    (pointer, payload.semantic.field_byte_offset)
                } else {
                    let input = registers
                        .get(pointer.0 as usize)
                        .cloned()
                        .ok_or_else(invalid)?;
                    let address_instruction = SelectedInstructionId(
                        (*next_instruction).try_into().map_err(|_| invalid())?,
                    );
                    *next_instruction += 1;
                    let address =
                        VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
                    registers.push(VirtualRegister {
                        id: address,
                        scalar_type: input.scalar_type,
                        class: input.class,
                        origin: VirtualRegisterOrigin::AbiTransport {
                            instruction: address_instruction,
                            place,
                            byte_offset: payload.semantic.field_byte_offset,
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    });
                    instructions.push(crate::selection::constraints::instruction(
                        address_instruction,
                        SelectedInstructionKind::AddressOffset {
                            byte_offset: payload.semantic.field_byte_offset,
                        },
                        constraints.keys.address_offset.ok_or_else(invalid)?,
                        &[pointer, address],
                        provenance(),
                        catalog,
                    )?);
                    (address, 0)
                };
                let (load_kind, _) = load_transfer(&constraints.keys, byte_count, displacement)
                    .ok_or_else(invalid)?;
                let load_instruction =
                    SelectedInstructionId((*next_instruction).try_into().map_err(|_| invalid())?);
                *next_instruction += 1;
                let loaded = VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
                registers.push(VirtualRegister {
                    id: loaded,
                    scalar_type: destination.scalar_type,
                    class: destination.class,
                    origin: VirtualRegisterOrigin::StructuralObservation {
                        instruction: load_instruction,
                        place,
                        byte_offset: payload.semantic.field_byte_offset,
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                });
                instructions.push(crate::selection::constraints::instruction(
                    load_instruction,
                    load_kind,
                    load_key.ok_or_else(invalid)?,
                    &[pointer, loaded],
                    provenance(),
                    catalog,
                )?);
                memory.push(SelectedMemoryAccess {
                    instruction: load_instruction,
                    origin: SelectedMemoryAccessOrigin::Edge(psi_edge),
                    place,
                    byte_offset: payload.semantic.field_byte_offset,
                    byte_count,
                    role: SelectedMemoryAccessRole::ReadPlace,
                });
                let loaded = if matches!(destination.scalar_type, ScalarType::Integer(integer)
                    if integer.sign() == IntegerSign::Signed && matches!(integer.bits(), 8 | 16 | 32))
                {
                    let normalization = SelectedInstructionId(
                        (*next_instruction).try_into().map_err(|_| invalid())?,
                    );
                    *next_instruction += 1;
                    let normalized =
                        VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
                    registers.push(VirtualRegister {
                        id: normalized,
                        scalar_type: destination.scalar_type,
                        class: destination.class,
                        origin: VirtualRegisterOrigin::AbiTransport {
                            instruction: normalization,
                            place,
                            byte_offset: payload.semantic.field_byte_offset,
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    });
                    instructions.push(crate::selection::constraints::instruction(
                        normalization,
                        integer_carrier_normalization(destination.scalar_type),
                        constraints.keys.copy_i64,
                        &[loaded, normalized],
                        provenance(),
                        catalog,
                    )?);
                    normalized
                } else {
                    loaded
                };
                payload.transport = SelectedCasePayloadTransport::Registers {
                    argument: loaded,
                    parameter,
                };
            }
            Ok(())
        }
    }

    pub(crate) fn prepare(
        function_index: usize,
        function: &mut SelectedFunction,
        constraints: &SelectedSelectionConstraints,
        catalog: &PreparedRegisterConstraintCatalog,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        mut edge_work: Vec<super::EdgeWork>,
    ) -> Result<(), SelectedInstructionError> {
        let source_count = function.blocks.len();
        let mut next_instruction = instruction_count(function)
            + edge_work
                .iter()
                .map(|work| work.preparation.len() + work.cleanup.len())
                .sum::<usize>();
        let mut bridges = Vec::new();
        let mut unbridged_fallthroughs = Vec::new();
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
                            && Some(work.edge) == successor.psi_edge
                    })
                    .map(|position| edge_work.remove(position));
                let (preparation, mut cleanup) = work.map_or_else(
                    || (Vec::new(), Vec::new()),
                    |work| (work.preparation, work.cleanup),
                );
                if successor.structural_case.is_some() && !successor.bindings.is_empty() {
                    return Err(invalid(function_index));
                }
                let payload_work = successor.structural_case.as_ref().is_some_and(|case| {
                    case.payloads.iter().any(|payload| {
                        payload.transport
                            != selected_instructions::SelectedCasePayloadTransport::Unused
                    })
                });
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
                    && !payload_work
                    && successor.structural_bindings.iter().all(|binding| {
                        binding.transport
                            == selected_instructions::SelectedStructuralTransport::Unused
                    })
                {
                    if !conditional || position != 1 {
                        continue;
                    }
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
                    u32::try_from(source_count + bridges.len())
                        .map_err(|_| invalid(function_index))?,
                );
                if let SelectedSuccessorRole::OperationContinuation { operation } = successor.role {
                    if successor.psi_edge.is_some()
                        || !successor.work.is_empty()
                        || !successor.bindings.is_empty()
                        || !successor.structural_bindings.is_empty()
                        || successor.structural_case.is_some()
                        || !preparation.is_empty()
                        || !cleanup.is_empty()
                    {
                        return Err(invalid(function_index));
                    }
                    let jump_id = SelectedInstructionId(
                        u32::try_from(next_instruction).map_err(|_| invalid(function_index))?,
                    );
                    next_instruction += 1;
                    let jump = crate::selection::constraints::instruction(
                        jump_id,
                        SelectedInstructionKind::Jump,
                        constraints.keys.jump,
                        &[],
                        SelectedInstructionProvenance {
                            operations: vec![operation],
                            ..Default::default()
                        },
                        catalog,
                    )?;
                    bridges.push(SelectedBlock {
                        id: bridge_id,
                        origin: SelectedBlockOrigin::Operation {
                            source: successor.source_target,
                            operation,
                        },
                        instructions: Vec::new(),
                        terminator: SelectedTerminator::Jump {
                            instruction: jump,
                            successor: successor.clone(),
                        },
                    });
                    successor.block = bridge_id;
                    continue;
                }
                let psi_edge = successor.psi_edge.ok_or_else(|| invalid(function_index))?;
                let mut continuation = successor.clone();
                continuation.role = SelectedSuccessorRole::EdgeTransferContinuation;
                continuation.work.clear();
                let mut instructions = preparation;
                if continuation.structural_case.is_some() {
                    structural_case::snapshot(
                        function_index,
                        &mut continuation,
                        &mut function.virtual_registers,
                        &mut function.memory_accesses,
                        &function.local_storage_slots,
                        &mut next_instruction,
                        constraints,
                        catalog,
                        environment,
                        &mut instructions,
                    )?;
                    continuation
                        .structural_case
                        .as_mut()
                        .ok_or_else(|| invalid(function_index))?
                        .cleanup_actions
                        .clear();
                }
                let mut snapshots = Vec::new();
                let mut transfers = Vec::new();
                let mut descriptors = Vec::new();
                let mut scalar_references = Vec::new();
                for phase in 0..2 {
                    if phase == 1 {
                        let mut emission = Emission {
                            function: function_index,
                            registers: &mut function.virtual_registers,
                            memory: &mut function.memory_accesses,
                            instructions: &mut instructions,
                            next_instruction: &mut next_instruction,
                            edge: psi_edge,
                            environment,
                            constraints,
                            catalog,
                        };
                        descriptors = super::descriptors::snapshot(
                            &mut emission,
                            &mut function.local_storage_slots,
                            &successor.structural_bindings,
                        )?;
                        scalar_references = super::scalar_references::snapshot(
                            &mut emission,
                            &successor.structural_bindings,
                        )?;
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
                        instructions.push(crate::selection::constraints::instruction(
                            instruction_id,
                            SelectedInstructionKind::CopyI64,
                            constraints.keys.copy_i64,
                            &[input, output],
                            value_provenance(semantic.argument),
                            catalog,
                        )?);
                        if phase == 0 {
                            snapshots.push(output);
                        } else {
                            transfers.push(output);
                        }
                    }
                }
                {
                    let mut emission = Emission {
                        function: function_index,
                        registers: &mut function.virtual_registers,
                        memory: &mut function.memory_accesses,
                        instructions: &mut instructions,
                        next_instruction: &mut next_instruction,
                        edge: psi_edge,
                        environment,
                        constraints,
                        catalog,
                    };
                    super::descriptors::store(
                        &mut emission,
                        &successor.structural_bindings,
                        &descriptors,
                    )?;
                    super::scalar_references::store(&mut emission, &scalar_references)?;
                    super::addresses::store(&mut emission, &successor.structural_bindings)?;
                }
                let mut position = 0;
                for binding in &mut continuation.bindings {
                    if let SelectedValueTransport::Registers { argument, .. } =
                        &mut binding.transport
                    {
                        *argument = transfers[position];
                        position += 1;
                    }
                }
                let jump_id = SelectedInstructionId(
                    u32::try_from(next_instruction).map_err(|_| invalid(function_index))?,
                );
                next_instruction += 1;
                let jump = crate::selection::constraints::instruction(
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
                        edge: psi_edge,
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
                if let Some(case) = &mut successor.structural_case {
                    for payload in &mut case.payloads {
                        payload.transport =
                            selected_instructions::SelectedCasePayloadTransport::Unused;
                    }
                }
            }
        }
        if !edge_work.is_empty() {
            return Err(invalid(function_index));
        }
        function.blocks.extend(bridges);
        Ok(())
    }
}
pub(crate) mod descriptors {
    use super::emission::{Emission, provenance};
    use crate::SelectedInstructionError;
    use crate::selection::edge_transfers::{invalid, stored_transport};
    use crate::selection::extent_copy;
    use crate::selection::scalar_call_abi::load_transfer;
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedInstructionKind, SelectedLocalStorageSlot,
        SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
        SelectedStructuralBinding, VirtualRegisterId,
    };
    use semantic_vocabulary::{IntegerValue, PlaceId};

    pub(crate) struct Snapshot {
        pointer: VirtualRegisterId,
        byte_size: u32,
        source_byte_offset: u32,
    }

    pub(crate) fn snapshot(
        emission: &mut Emission<'_>,
        slots: &mut Vec<SelectedLocalStorageSlot>,
        bindings: &[SelectedStructuralBinding],
    ) -> Result<Vec<Snapshot>, SelectedInstructionError> {
        let function = emission.function;
        let mut snapshots = Vec::new();
        for (binding_index, binding) in bindings.iter().enumerate() {
            let Some((argument, source_byte_offset, _, byte_size, alignment)) =
                stored_transport(binding.transport)
            else {
                continue;
            };
            let slot = LocalStorageSlotId::EdgeSnapshot {
                edge: emission.edge,
                binding_index: u32::try_from(binding_index).map_err(|_| invalid(function))?,
            };
            if slots.iter().any(|existing| existing.id == slot) {
                return Err(invalid(function));
            }
            slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: byte_size.max(1),
                alignment,
            });
            let pointer = frame_address(emission, slot, binding.semantic.argument.place)?;
            copy(
                emission,
                CopyExtent {
                    source: argument,
                    source_offset: source_byte_offset,
                    destination: pointer,
                    byte_size,
                    lineage_place: binding.semantic.argument.place,
                    lineage_offset: source_byte_offset,
                    read_binding: Some(binding),
                    write_local: None,
                },
            )?;
            snapshots.push(Snapshot {
                pointer,
                byte_size,
                source_byte_offset,
            });
        }
        Ok(snapshots)
    }

    pub(crate) fn store(
        emission: &mut Emission<'_>,
        bindings: &[SelectedStructuralBinding],
        snapshots: &[Snapshot],
    ) -> Result<(), SelectedInstructionError> {
        let function = emission.function;
        let bindings = bindings
            .iter()
            .filter(|binding| stored_transport(binding.transport).is_some());
        if bindings.clone().count() != snapshots.len() {
            return Err(invalid(function));
        }
        for (binding, snapshot) in bindings.zip(snapshots) {
            let Some((_, _, destination, byte_size, _)) = stored_transport(binding.transport)
            else {
                return Err(invalid(function));
            };
            if snapshot.byte_size != byte_size {
                return Err(invalid(function));
            }
            let pointer = frame_address(emission, destination, binding.semantic.parameter)?;
            let address_instruction = emission
                .instructions
                .last()
                .ok_or_else(|| invalid(function))?
                .id;
            emission.memory.push(SelectedMemoryAccess {
                instruction: address_instruction,
                origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
                place: binding.semantic.parameter,
                byte_offset: 0,
                byte_count: byte_size,
                role: SelectedMemoryAccessRole::AddressLocal { slot: destination },
            });
            copy(
                emission,
                CopyExtent {
                    source: snapshot.pointer,
                    source_offset: 0,
                    destination: pointer,
                    byte_size,
                    lineage_place: binding.semantic.argument.place,
                    lineage_offset: snapshot.source_byte_offset,
                    read_binding: None,
                    write_local: Some((binding.semantic.parameter, destination)),
                },
            )?;
        }
        Ok(())
    }

    struct CopyExtent<'binding> {
        source: VirtualRegisterId,
        source_offset: u32,
        destination: VirtualRegisterId,
        byte_size: u32,
        lineage_place: PlaceId,
        lineage_offset: u32,
        read_binding: Option<&'binding SelectedStructuralBinding>,
        write_local: Option<(PlaceId, LocalStorageSlotId)>,
    }

    fn frame_address(
        emission: &mut Emission<'_>,
        slot: LocalStorageSlotId,
        place: PlaceId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let function = emission.function;
        let key = emission
            .constraints
            .keys
            .frame_address
            .ok_or_else(|| invalid(function))?;
        emission.emit(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            key,
            &[],
            place,
            0,
            None,
        )
    }

    fn memory_pointer(
        emission: &mut Emission<'_>,
        pointer: VirtualRegisterId,
        place: PlaceId,
        byte_offset: u32,
        byte_size: u32,
    ) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
        let function = emission.function;
        if emission
            .environment
            .admits_pointer_displacement(byte_offset, byte_size)
        {
            return Ok((pointer, byte_offset));
        }
        let key = emission
            .constraints
            .keys
            .address_offset
            .ok_or_else(|| invalid(function))?;
        let output = emission.emit(
            SelectedInstructionKind::AddressOffset { byte_offset },
            key,
            &[pointer],
            place,
            byte_offset,
            None,
        )?;
        Ok((output, 0))
    }

    fn copy(
        emission: &mut Emission<'_>,
        extent: CopyExtent,
    ) -> Result<(), SelectedInstructionError> {
        let function = emission.function;
        if extent.byte_size > extent_copy::INLINE_MAX_BYTES {
            return bulk_copy(emission, extent);
        }
        for (cursor, width) in extent_copy::chunks(extent.byte_size) {
            let byte_offset = extent
                .source_offset
                .checked_add(cursor)
                .ok_or_else(|| invalid(function))?;
            let (source, displacement) = memory_pointer(
                emission,
                extent.source,
                extent.lineage_place,
                byte_offset,
                u32::from(width),
            )?;
            let lineage_offset = extent
                .lineage_offset
                .checked_add(cursor)
                .ok_or_else(|| invalid(function))?;
            let (kind, key) =
                load_transfer(&emission.constraints.keys, u32::from(width), displacement)
                    .ok_or_else(|| invalid(function))?;
            let value = emission.emit(
                kind,
                key.ok_or_else(|| invalid(function))?,
                &[source],
                extent.lineage_place,
                lineage_offset,
                None,
            )?;
            if let Some(binding) = extent.read_binding {
                let load = emission
                    .instructions
                    .last()
                    .ok_or_else(|| invalid(function))?
                    .id;
                emission
                    .memory
                    .extend(
                        binding
                            .source_read_roles()
                            .map(|role| SelectedMemoryAccess {
                                instruction: load,
                                origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
                                place: binding.semantic.argument.place,
                                byte_offset,
                                byte_count: u32::from(width),
                                role,
                            }),
                    );
            }
            let (destination, displacement) = memory_pointer(
                emission,
                extent.destination,
                extent.lineage_place,
                cursor,
                u32::from(width),
            )?;
            let store = emission.instruction_id()?;
            emission
                .instructions
                .push(crate::selection::constraints::instruction(
                    store,
                    SelectedInstructionKind::Store {
                        byte_offset: displacement,
                        byte_size: width,
                    },
                    emission
                        .constraints
                        .keys
                        .store
                        .ok_or_else(|| invalid(function))?,
                    &[destination, value],
                    provenance(emission.edge),
                    emission.catalog,
                )?);
            if let Some((place, slot)) = extent.write_local {
                emission.memory.push(SelectedMemoryAccess {
                    instruction: store,
                    origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
                    place,
                    byte_offset: cursor,
                    byte_count: u32::from(width),
                    role: SelectedMemoryAccessRole::WriteLocal { slot },
                });
            }
        }
        Ok(())
    }

    fn bulk_copy(
        emission: &mut Emission<'_>,
        extent: CopyExtent,
    ) -> Result<(), SelectedInstructionError> {
        let function = emission.function;
        let last_byte = extent
            .byte_size
            .checked_sub(1)
            .ok_or_else(|| invalid(function))?;
        extent
            .source_offset
            .checked_add(last_byte)
            .ok_or_else(|| invalid(function))?;
        extent
            .lineage_offset
            .checked_add(last_byte)
            .ok_or_else(|| invalid(function))?;
        let source = if extent.source_offset == 0 {
            extent.source
        } else {
            emission.emit(
                SelectedInstructionKind::AddressOffset {
                    byte_offset: extent.source_offset,
                },
                emission
                    .constraints
                    .keys
                    .address_offset
                    .ok_or_else(|| invalid(function))?,
                &[extent.source],
                extent.lineage_place,
                extent.lineage_offset,
                None,
            )?
        };
        let count = emission.emit(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(extent.byte_size)),
            },
            emission.constraints.keys.materialize_i64,
            &[],
            extent.lineage_place,
            extent.lineage_offset,
            None,
        )?;
        let instruction = emission.instruction_id()?;
        let key = emission
            .constraints
            .keys
            .copy_bytes
            .ok_or_else(|| invalid(function))?;
        let operands = extent_copy::bulk_operands(
            instruction,
            [source, extent.destination, count],
            key,
            emission.catalog,
            emission.registers,
        )?;
        emission
            .instructions
            .push(crate::selection::constraints::instruction(
                instruction,
                SelectedInstructionKind::CopyBytes,
                key,
                &operands,
                provenance(emission.edge),
                emission.catalog,
            )?);
        if let Some(binding) = extent.read_binding {
            emission.memory.extend(
                binding
                    .source_read_roles()
                    .map(|role| SelectedMemoryAccess {
                        instruction,
                        origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
                        place: binding.semantic.argument.place,
                        byte_offset: extent.source_offset,
                        byte_count: extent.byte_size,
                        role,
                    }),
            );
        }
        if let Some((place, slot)) = extent.write_local {
            emission.memory.push(SelectedMemoryAccess {
                instruction,
                origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
                place,
                byte_offset: 0,
                byte_count: extent.byte_size,
                role: SelectedMemoryAccessRole::WriteLocal { slot },
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use selected_instructions::{
            SelectedSelectionConstraints, SelectedStructuralTransport, VirtualRegister,
            VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{BlockId, EdgeId};
        use target::NativeTarget;

        fn emitted(
            target: NativeTarget,
            byte_size: u32,
            source_offset: u32,
        ) -> (
            Vec<selected_instructions::SelectedInstruction>,
            Vec<SelectedMemoryAccess>,
            Vec<VirtualRegister>,
            Vec<SelectedLocalStorageSlot>,
        ) {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                fixed_inputs: vec![],
            };
            let catalog = environment.constraints();
            let place = PlaceId::new(1).unwrap();
            let parameter = PlaceId::new(2).unwrap();
            let edge = EdgeId::new(3).unwrap();
            let destination = LocalStorageSlotId::StructuralBlockParameter {
                block: BlockId::new(4).unwrap(),
                place: parameter,
            };
            let binding = SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter,
                    argument: terminal_psi::StructuralArgument::whole(
                        place,
                        terminal_psi::StructuralAccess::Owned,
                    ),
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: VirtualRegisterId(0),
                    source_byte_offset: source_offset,
                    destination,
                    byte_size,
                    alignment: 16,
                },
                source_indices: vec![],
            };
            let class =
                crate::selection::constraints::row(catalog, constraints.keys.materialize_i64)
                    .unwrap()
                    .operands[0]
                    .class;
            let mut registers = vec![VirtualRegister {
                id: VirtualRegisterId(0),
                scalar_type: super::super::emission::pointer_type(0).unwrap(),
                class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: selected_instructions::SelectedInstructionId(99),
                    place,
                    byte_offset: source_offset,
                },
                definition_site: None,
                entry_fixed_view: None,
            }];
            let mut instructions = vec![];
            let mut memory = vec![];
            let mut next_instruction = 0;
            let mut slots = vec![];
            let mut emission = Emission {
                function: 0,
                registers: &mut registers,
                memory: &mut memory,
                instructions: &mut instructions,
                next_instruction: &mut next_instruction,
                edge,
                environment: &environment,
                constraints: &constraints,
                catalog,
            };
            let snapshots =
                snapshot(&mut emission, &mut slots, std::slice::from_ref(&binding)).unwrap();
            let capture_end = emission.instructions.len();
            store(&mut emission, std::slice::from_ref(&binding), &snapshots).unwrap();
            assert!(
                emission
                    .memory
                    .iter()
                    .filter(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                    .all(|access| access.instruction.0 < capture_end as u32)
            );
            assert!(
                emission
                    .memory
                    .iter()
                    .filter(|access| matches!(
                        access.role,
                        SelectedMemoryAccessRole::WriteLocal { .. }
                    ))
                    .all(|access| access.instruction.0 >= capture_end as u32)
            );
            assert_eq!(slots.len(), 1);
            assert_eq!(slots[0].byte_size, byte_size.max(1));
            assert_eq!(slots[0].alignment, 16);
            assert_eq!(
                slots[0].id,
                LocalStorageSlotId::EdgeSnapshot {
                    edge,
                    binding_index: 0
                }
            );
            (instructions, memory, registers, slots)
        }

        #[test]
        fn edge_copy_zero_small_tail_and_bulk_boundary_emit_exact_extents() {
            for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
                for (size, chunks) in [(0, 0), (1, 1), (15, 4), (4096, 512)] {
                    let (instructions, memory, _, _) = emitted(target, size, 0);
                    assert_eq!(instructions.len(), 2 + 4 * chunks);
                    assert!(
                        !instructions
                            .iter()
                            .any(|instruction| instruction.kind
                                == SelectedInstructionKind::CopyBytes)
                    );
                    let reads: Vec<_> = memory
                        .iter()
                        .filter(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                        .collect();
                    let writes: Vec<_> = memory
                        .iter()
                        .filter(|access| {
                            matches!(access.role, SelectedMemoryAccessRole::WriteLocal { .. })
                        })
                        .collect();
                    assert_eq!(reads.len(), chunks);
                    assert_eq!(writes.len(), chunks);
                    assert_eq!(
                        reads.iter().map(|access| access.byte_count).sum::<u32>(),
                        size
                    );
                    assert_eq!(
                        writes.iter().map(|access| access.byte_count).sum::<u32>(),
                        size
                    );
                    for (read, write) in reads.iter().zip(writes) {
                        assert_eq!(
                            (read.byte_offset, read.byte_count),
                            (write.byte_offset, write.byte_count)
                        );
                    }
                }
                let (instructions, memory, _, _) = emitted(target, 4097, 0);
                assert_eq!(instructions.len(), 6);
                assert_eq!(
                    instructions
                        .iter()
                        .filter(|instruction| instruction.kind == SelectedInstructionKind::CopyBytes)
                        .count(),
                    2
                );
                assert_eq!(
                    memory
                        .iter()
                        .filter(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                        .map(|access| access.byte_count)
                        .collect::<Vec<_>>(),
                    [4097]
                );
            }
        }

        #[test]
        fn huge_edge_copy_emission_is_bounded_and_retains_offset_count_and_private_homes() {
            for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
                for size in [5_242_880, u32::MAX - 17] {
                    let (instructions, memory, registers, _) = emitted(target, size, 17);
                    assert_eq!(instructions.len(), 7);
                    assert_eq!(registers.len(), 10);
                    assert!(instructions.iter().any(|instruction| instruction.kind
                        == SelectedInstructionKind::AddressOffset { byte_offset: 17 }));
                    let copies: Vec<_> = instructions
                        .iter()
                        .filter(|instruction| {
                            instruction.kind == SelectedInstructionKind::CopyBytes
                        })
                        .collect();
                    assert_eq!(copies.len(), 2);
                    for copy in copies {
                        let count = copy.operands[2].virtual_register;
                        assert!(instructions.iter().any(|instruction| instruction.kind
                            == SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(u128::from(size))
                            }
                            && instruction.operands[0].virtual_register == count));
                        for operand in &copy.operands[3..] {
                            assert!(operand.early_clobber);
                            assert_eq!(
                                registers[operand.virtual_register.0 as usize].origin,
                                VirtualRegisterOrigin::InstructionScratch {
                                    instruction: copy.id,
                                    operand: operand.operand
                                }
                            );
                        }
                    }
                    let read = memory
                        .iter()
                        .find(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                        .unwrap();
                    assert_eq!((read.byte_offset, read.byte_count), (17, size));
                    let write = memory
                        .iter()
                        .find(|access| {
                            matches!(access.role, SelectedMemoryAccessRole::WriteLocal { .. })
                        })
                        .unwrap();
                    assert_eq!((write.byte_offset, write.byte_count), (0, size));
                    assert_eq!(memory.len(), 3);
                }
            }
        }
    }
}
pub(crate) mod emission {
    use super::VirtualRegisterId;
    use crate::SelectedInstructionError;
    use crate::selection::constraints::store_registers;
    use crate::selection::edge_transfers::invalid;
    use register_model::PreparedRegisterConstraintCatalog;
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedInstruction, SelectedInstructionId,
        SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
        SelectedMemoryAccessRole, SelectedSelectionConstraints, VirtualRegister,
        VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{EdgeId, PlaceId};

    pub(crate) struct Emission<'state> {
        pub(crate) function: usize,
        pub(crate) registers: &'state mut Vec<VirtualRegister>,
        pub(crate) memory: &'state mut Vec<SelectedMemoryAccess>,
        pub(crate) instructions: &'state mut Vec<SelectedInstruction>,
        pub(crate) next_instruction: &'state mut usize,
        pub(crate) edge: EdgeId,
        pub(crate) environment: &'state register_environment::ValidatedTargetRegisterEnvironment,
        pub(crate) constraints: &'state SelectedSelectionConstraints,
        pub(crate) catalog: &'state PreparedRegisterConstraintCatalog,
    }

    impl Emission<'_> {
        pub(crate) fn instruction_id(
            &mut self,
        ) -> Result<SelectedInstructionId, SelectedInstructionError> {
            let function = self.function;
            let id = SelectedInstructionId(
                (*self.next_instruction)
                    .try_into()
                    .map_err(|_| invalid(function))?,
            );
            *self.next_instruction += 1;
            Ok(id)
        }

        pub(crate) fn emit(
            &mut self,
            kind: SelectedInstructionKind,
            key: register_model::RegisterConstraintKey,
            inputs: &[VirtualRegisterId],
            place: PlaceId,
            byte_offset: u32,
            access: Option<(u32, SelectedMemoryAccessRole)>,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let function = self.function;
            let instruction_id = self.instruction_id()?;
            let output = VirtualRegisterId(
                self.registers
                    .len()
                    .try_into()
                    .map_err(|_| invalid(function))?,
            );
            let mut operands = inputs.to_vec();
            operands.push(output);
            let instruction = crate::selection::constraints::instruction(
                instruction_id,
                kind,
                key,
                &operands,
                provenance(self.edge),
                self.catalog,
            )?;
            let class = instruction
                .operands
                .last()
                .ok_or_else(|| invalid(function))?
                .class;
            self.registers.push(VirtualRegister {
                id: output,
                scalar_type: pointer_type(function)?,
                class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: instruction_id,
                    place,
                    byte_offset,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            if let Some((byte_count, role)) = access {
                self.memory.push(SelectedMemoryAccess {
                    instruction: instruction_id,
                    origin: SelectedMemoryAccessOrigin::Edge(self.edge),
                    place,
                    byte_offset,
                    byte_count,
                    role,
                });
            }
            self.instructions.push(instruction);
            Ok(output)
        }

        pub(crate) fn store_local_word(
            &mut self,
            slot: LocalStorageSlotId,
            byte_offset: u32,
            value: VirtualRegisterId,
            place: PlaceId,
        ) -> Result<(), SelectedInstructionError> {
            let function = self.function;
            let instruction_id = self.instruction_id()?;
            let key = self
                .constraints
                .keys
                .store64
                .ok_or_else(|| invalid(function))?;
            let row = crate::selection::constraints::row(self.catalog, key)?;
            let operands = store_registers(instruction_id, &[value], row, &mut *self.registers)?;
            self.instructions
                .push(crate::selection::constraints::instruction(
                    instruction_id,
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset,
                    },
                    key,
                    &operands,
                    provenance(self.edge),
                    self.catalog,
                )?);
            self.memory.push(SelectedMemoryAccess {
                instruction: instruction_id,
                origin: SelectedMemoryAccessOrigin::Edge(self.edge),
                place,
                byte_offset,
                byte_count: 8,
                role: SelectedMemoryAccessRole::WriteLocal { slot },
            });
            Ok(())
        }

        pub(crate) fn source_reads(
            &mut self,
            binding: &selected_instructions::SelectedStructuralBinding,
            byte_offset: u32,
        ) -> Result<(), SelectedInstructionError> {
            let function = self.function;
            let instruction = self
                .instructions
                .last()
                .ok_or_else(|| invalid(function))?
                .id;
            self.memory.extend(
                binding
                    .source_read_roles()
                    .map(|role| SelectedMemoryAccess {
                        instruction,
                        origin: SelectedMemoryAccessOrigin::Edge(self.edge),
                        place: binding.semantic.argument.place,
                        byte_offset,
                        byte_count: 8,
                        role,
                    }),
            );
            Ok(())
        }
    }

    pub(crate) fn pointer_type(
        function: usize,
    ) -> Result<semantic_vocabulary::ScalarType, SelectedInstructionError> {
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .map(semantic_vocabulary::ScalarType::Integer)
            .map_err(|_| invalid(function))
    }

    pub(crate) fn provenance(edge: EdgeId) -> selected_instructions::SelectedInstructionProvenance {
        selected_instructions::SelectedInstructionProvenance {
            edges: vec![edge],
            ..Default::default()
        }
    }
}
pub(crate) mod scalar_references {
    use super::emission::Emission;
    use crate::SelectedInstructionError;
    use crate::selection::edge_transfers::invalid;
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedAddressBase, SelectedInstructionKind,
        SelectedMemoryAccessRole, SelectedScalarReferenceSource, SelectedStructuralBinding,
        SelectedStructuralTransport, VirtualRegisterId,
    };
    use semantic_vocabulary::{IntegerValue, PlaceId};

    pub(crate) struct Snapshot {
        destination: LocalStorageSlotId,
        place: PlaceId,
        address: VirtualRegisterId,
        codec: VirtualRegisterId,
    }

    pub(crate) fn snapshot(
        emission: &mut Emission<'_>,
        bindings: &[SelectedStructuralBinding],
    ) -> Result<Vec<Snapshot>, SelectedInstructionError> {
        let function = emission.function;
        let error = || invalid(function);
        let mut snapshots = Vec::new();
        for binding in bindings {
            let SelectedStructuralTransport::ScalarReference {
                base,
                byte_offset,
                source,
                destination,
            } = binding.transport
            else {
                continue;
            };
            let place = binding.semantic.argument.place;
            let source_extent = match source {
                SelectedScalarReferenceSource::Descriptor => 16,
                SelectedScalarReferenceSource::Canonical { codec } => u32::from(
                    runtime_abi::ScalarStorageCodec::decode(codec)
                        .ok_or_else(error)?
                        .byte_size(),
                ),
            };
            let pointer = match base {
                SelectedAddressBase::Local(slot) => emission.emit(
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset,
                    },
                    emission.constraints.keys.frame_address.ok_or_else(error)?,
                    &[],
                    place,
                    byte_offset,
                    Some((
                        source_extent,
                        SelectedMemoryAccessRole::AddressLocal { slot },
                    )),
                )?,
                SelectedAddressBase::Register(register) => emission.emit(
                    if byte_offset == 0 {
                        SelectedInstructionKind::CopyI64
                    } else {
                        SelectedInstructionKind::AddressOffset { byte_offset }
                    },
                    if byte_offset == 0 {
                        emission.constraints.keys.copy_i64
                    } else {
                        emission.constraints.keys.address_offset.ok_or_else(error)?
                    },
                    &[register],
                    place,
                    byte_offset,
                    None,
                )?,
            };
            let (address, codec) = match source {
                SelectedScalarReferenceSource::Descriptor => {
                    let address = emission.emit(
                        SelectedInstructionKind::Load64 { byte_offset: 0 },
                        emission.constraints.keys.load64.ok_or_else(error)?,
                        &[pointer],
                        place,
                        byte_offset,
                        None,
                    )?;
                    emission.source_reads(binding, byte_offset)?;
                    let codec_offset = byte_offset.checked_add(8).ok_or_else(error)?;
                    let codec = emission.emit(
                        SelectedInstructionKind::Load64 { byte_offset: 8 },
                        emission.constraints.keys.load64.ok_or_else(error)?,
                        &[pointer],
                        place,
                        codec_offset,
                        None,
                    )?;
                    emission.source_reads(binding, codec_offset)?;
                    (address, codec)
                }
                SelectedScalarReferenceSource::Canonical { codec } => {
                    let codec = emission.emit(
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(u128::from(codec)),
                        },
                        emission.constraints.keys.materialize_i64,
                        &[],
                        place,
                        byte_offset,
                        None,
                    )?;
                    (pointer, codec)
                }
            };
            snapshots.push(Snapshot {
                destination,
                place: binding.semantic.parameter,
                address,
                codec,
            });
        }
        Ok(snapshots)
    }

    pub(crate) fn store(
        emission: &mut Emission<'_>,
        snapshots: &[Snapshot],
    ) -> Result<(), SelectedInstructionError> {
        for snapshot in snapshots {
            for (byte_offset, register) in [(0, snapshot.address), (8, snapshot.codec)] {
                emission.store_local_word(
                    snapshot.destination,
                    byte_offset,
                    register,
                    snapshot.place,
                )?;
            }
        }
        Ok(())
    }
}
pub(crate) use construction::prepare;

pub(crate) struct EdgeWork {
    pub edge: semantic_vocabulary::EdgeId,
    pub preparation: Vec<selected_instructions::SelectedInstruction>,
    pub cleanup: Vec<selected_instructions::SelectedInstruction>,
}

fn invalid(function: usize) -> SelectedInstructionError {
    SelectedInstructionError::FunctionProjectionMismatch { function }
}

fn instruction_count(function: &SelectedFunction) -> usize {
    function
        .blocks
        .iter()
        .map(|block| block.instructions.len() + 1)
        .sum()
}

fn stored_transport(
    transport: selected_instructions::SelectedStructuralTransport,
) -> Option<(
    VirtualRegisterId,
    u32,
    selected_instructions::LocalStorageSlotId,
    u32,
    u16,
)> {
    match transport {
        selected_instructions::SelectedStructuralTransport::Unused
        | selected_instructions::SelectedStructuralTransport::Address { .. }
        | selected_instructions::SelectedStructuralTransport::ScalarReference { .. } => None,
        selected_instructions::SelectedStructuralTransport::WholeValue {
            argument,
            source_byte_offset,
            destination,
            byte_size,
            alignment,
        } => Some((
            argument,
            source_byte_offset,
            destination,
            byte_size,
            alignment,
        )),
    }
}

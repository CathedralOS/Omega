//! Address transports lend a referent's address on the edge that binds it.
//!
//! An address join's slot receives one pointer, never referent bytes. The
//! bridge forms that pointer from the transport's base after every scalar and
//! descriptor snapshot: a frame-resident home through `FrameAddress`, a live
//! pointer register directly or through `AddressOffset`. Neither form reads
//! memory, so no destination write can disturb another binding's source. The
//! pointer is then stored whole into the join's own block slot.
use super::VirtualRegisterId;
use crate::SelectedInstructionError;
use crate::selection::edge_transfers::descriptors::provenance;
use crate::selection::edge_transfers::invalid;
use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    FrameStorageSlotId, SelectedAddressBase, SelectedBlock, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedSelectionConstraints,
    SelectedStructuralBinding, SelectedStructuralTransport, SelectedSuccessor, VirtualRegister,
    VirtualRegisterOrigin,
};

/// One address transport: its base, displacement, lent span and join slot.
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
        | SelectedStructuralTransport::Descriptor { .. } => None,
    }
}

/// Whether forming this address needs its own instruction and register.
fn computed(base: SelectedAddressBase, byte_offset: u32) -> bool {
    matches!(base, SelectedAddressBase::Local(_)) || byte_offset != 0
}

/// The bridge instructions and fresh registers the address transports need.
pub(super) fn counts(bindings: &[SelectedStructuralBinding]) -> (usize, usize) {
    bindings
        .iter()
        .filter_map(|binding| address(binding.transport))
        .fold((0, 0), |(instructions, registers), (base, offset, _, _)| {
            let extra = usize::from(computed(base, offset));
            (instructions + 1 + extra, registers + extra)
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn store(
    function_index: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: semantic_vocabulary::EdgeId,
    bindings: &[SelectedStructuralBinding],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let error = || invalid(function_index);
    let next_id = |next_instruction: &mut usize| {
        let id = SelectedInstructionId((*next_instruction).try_into().map_err(|_| error())?);
        *next_instruction += 1;
        Ok::<_, SelectedInstructionError>(id)
    };
    for binding in bindings {
        let Some((base, byte_offset, byte_count, destination)) = address(binding.transport) else {
            continue;
        };
        let pointer = if computed(base, byte_offset) {
            let id = next_id(next_instruction)?;
            let (kind, key, inputs) = match base {
                SelectedAddressBase::Local(slot) => (
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset,
                    },
                    constraints.keys.frame_address.ok_or_else(error)?,
                    Vec::new(),
                ),
                SelectedAddressBase::Register(register) => (
                    SelectedInstructionKind::AddressOffset { byte_offset },
                    constraints.keys.address_offset.ok_or_else(error)?,
                    vec![register],
                ),
            };
            let output = VirtualRegisterId(registers.len().try_into().map_err(|_| error())?);
            let mut operands = inputs;
            operands.push(output);
            let instruction = super::super::constraints::instruction(
                id,
                kind,
                key,
                &operands,
                provenance(edge),
                catalog,
            )?;
            registers.push(VirtualRegister {
                id: output,
                scalar_type: pointer_type(function_index)?,
                class: instruction.operands.last().ok_or_else(error)?.class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: id,
                    place: binding.semantic.argument.place,
                    byte_offset,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            if let SelectedAddressBase::Local(slot) = base {
                memory.push(SelectedMemoryAccess {
                    instruction: id,
                    origin: SelectedMemoryAccessOrigin::Edge(edge),
                    place: binding.semantic.argument.place,
                    byte_offset,
                    byte_count,
                    role: SelectedMemoryAccessRole::AddressLocal { slot },
                });
            }
            instructions.push(instruction);
            output
        } else {
            let SelectedAddressBase::Register(register) = base else {
                return Err(error());
            };
            register
        };
        let id = next_id(next_instruction)?;
        instructions.push(super::super::constraints::instruction(
            id,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(destination),
                byte_offset: 0,
            },
            constraints.keys.store64.ok_or_else(error)?,
            &[pointer],
            provenance(edge),
            catalog,
        )?);
        memory.push(SelectedMemoryAccess {
            instruction: id,
            origin: SelectedMemoryAccessOrigin::Edge(edge),
            place: binding.semantic.parameter,
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::WriteLocal { slot: destination },
        });
    }
    Ok(())
}

/// Replay the address suffix of one bridge exactly as `store` emits it: its
/// instructions start at `instruction_position` within the bridge and its
/// fresh registers at `register_start`. Every destination must be the join's
/// own 8-byte block slot, a register base must predate the bridge, and a
/// frame base must name a declared slot wide enough for the lent span.
#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    function: usize,
    prepared: &SelectedFunction,
    bridge: &SelectedBlock,
    continuation: &SelectedSuccessor,
    instruction_position: usize,
    instruction_start: usize,
    register_start: usize,
    original_register_count: usize,
    constraints: &SelectedSelectionConstraints,
) -> Result<Vec<SelectedMemoryAccess>, SelectedInstructionError> {
    let error = || invalid(function);
    let expected_provenance = provenance(continuation.psi_edge);
    let mut accesses = Vec::new();
    let mut position = instruction_position;
    let mut register_index = register_start;
    for binding in &continuation.structural_bindings {
        let Some((base, byte_offset, byte_count, destination)) = address(binding.transport) else {
            continue;
        };
        if byte_count == 0
            || destination
                != (selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                    block: continuation.source_target,
                    place: binding.semantic.parameter,
                })
            || prepared
                .local_storage_slots
                .iter()
                .filter(|slot| slot.id == destination && slot.byte_size == 8 && slot.alignment == 8)
                .count()
                != 1
        {
            return Err(error());
        }
        let pointer = if computed(base, byte_offset) {
            let instruction = bridge.instructions.get(position).ok_or_else(error)?;
            let register = prepared
                .virtual_registers
                .get(register_index)
                .ok_or_else(error)?;
            let (kind, key, inputs) = match base {
                SelectedAddressBase::Local(slot) => {
                    let declared = prepared
                        .local_storage_slots
                        .iter()
                        .filter(|declared| declared.id == slot)
                        .collect::<Vec<_>>();
                    let [declared] = declared.as_slice() else {
                        return Err(error());
                    };
                    if byte_offset
                        .checked_add(byte_count)
                        .is_none_or(|end| end > declared.byte_size)
                    {
                        return Err(error());
                    }
                    (
                        SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Local(slot),
                            byte_offset,
                        },
                        constraints.keys.frame_address,
                        Vec::new(),
                    )
                }
                SelectedAddressBase::Register(input) => {
                    if input.0 as usize >= original_register_count {
                        return Err(error());
                    }
                    (
                        SelectedInstructionKind::AddressOffset { byte_offset },
                        constraints.keys.address_offset,
                        vec![input],
                    )
                }
            };
            if instruction.id.0 as usize != instruction_start + position
                || instruction.kind != kind
                || Some(instruction.constraint) != key
                || instruction.provenance != expected_provenance
                || instruction
                    .operands
                    .iter()
                    .map(|operand| operand.virtual_register)
                    .ne(inputs.iter().copied().chain([register.id]))
                || register.id.0 as usize != register_index
                || register.scalar_type != pointer_type(function)?
                || Some(register.class) != instruction.operands.last().map(|operand| operand.class)
                || register.definition_site.is_some()
                || register.entry_fixed_view.is_some()
                || register.origin
                    != (VirtualRegisterOrigin::AbiTransport {
                        instruction: instruction.id,
                        place: binding.semantic.argument.place,
                        byte_offset,
                    })
            {
                return Err(error());
            }
            if let SelectedAddressBase::Local(slot) = base {
                accesses.push(SelectedMemoryAccess {
                    instruction: instruction.id,
                    origin: SelectedMemoryAccessOrigin::Edge(continuation.psi_edge),
                    place: binding.semantic.argument.place,
                    byte_offset,
                    byte_count,
                    role: SelectedMemoryAccessRole::AddressLocal { slot },
                });
            }
            position += 1;
            register_index += 1;
            register.id
        } else {
            let SelectedAddressBase::Register(register) = base else {
                return Err(error());
            };
            if register.0 as usize >= original_register_count {
                return Err(error());
            }
            register
        };
        let store = bridge.instructions.get(position).ok_or_else(error)?;
        if store.id.0 as usize != instruction_start + position
            || store.kind
                != (SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(destination),
                    byte_offset: 0,
                })
            || Some(store.constraint) != constraints.keys.store64
            || store.provenance != expected_provenance
            || store
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .ne([pointer])
        {
            return Err(error());
        }
        accesses.push(SelectedMemoryAccess {
            instruction: store.id,
            origin: SelectedMemoryAccessOrigin::Edge(continuation.psi_edge),
            place: binding.semantic.parameter,
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::WriteLocal { slot: destination },
        });
        position += 1;
    }
    if position != bridge.instructions.len() {
        return Err(error());
    }
    Ok(accesses)
}

fn pointer_type(
    function: usize,
) -> Result<semantic_vocabulary::ScalarType, SelectedInstructionError> {
    semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
        .map(semantic_vocabulary::ScalarType::Integer)
        .map_err(|_| invalid(function))
}

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
    FrameStorageSlotId, SelectedAddressBase, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole, SelectedSelectionConstraints, SelectedStructuralBinding,
    SelectedStructuralTransport, VirtualRegister, VirtualRegisterOrigin,
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
        SelectedStructuralTransport::Unused | SelectedStructuralTransport::WholeValue { .. } => {
            None
        }
    }
}

/// Whether forming this address needs its own instruction and register.
fn computed(base: SelectedAddressBase, byte_offset: u32) -> bool {
    matches!(base, SelectedAddressBase::Local(_)) || byte_offset != 0
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

fn pointer_type(
    function: usize,
) -> Result<semantic_vocabulary::ScalarType, SelectedInstructionError> {
    semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
        .map(semantic_vocabulary::ScalarType::Integer)
        .map_err(|_| invalid(function))
}

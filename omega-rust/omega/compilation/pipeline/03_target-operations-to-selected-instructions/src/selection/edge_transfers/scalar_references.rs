//! Snapshot both words before any edge destination is installed.
//! Loans preserve backing addresses; no scalar contents are copied or written.

use super::descriptors::provenance;
use crate::SelectedInstructionError;
use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedAddressBase, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedScalarReferenceSource,
    SelectedSelectionConstraints, SelectedStructuralBinding, SelectedStructuralTransport,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{EdgeId, IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarType};

pub(super) struct Snapshot {
    destination: LocalStorageSlotId,
    place: PlaceId,
    address: VirtualRegisterId,
    codec: VirtualRegisterId,
}

struct Emission<'state> {
    function: usize,
    registers: &'state mut Vec<VirtualRegister>,
    memory: &'state mut Vec<SelectedMemoryAccess>,
    instructions: &'state mut Vec<SelectedInstruction>,
    next_instruction: &'state mut usize,
    edge: EdgeId,
    catalog: &'state ValidatedRegisterConstraintCatalog,
}

impl Emission<'_> {
    fn source_reads(
        &mut self,
        binding: &SelectedStructuralBinding,
        byte_offset: u32,
    ) -> Result<(), SelectedInstructionError> {
        let instruction = self
            .instructions
            .last()
            .ok_or_else(|| super::invalid(self.function))?
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

    fn emit(
        &mut self,
        kind: SelectedInstructionKind,
        key: register_model::RegisterConstraintKey,
        inputs: &[VirtualRegisterId],
        place: PlaceId,
        byte_offset: u32,
        access: Option<(u32, SelectedMemoryAccessRole)>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let error = || super::invalid(self.function);
        let instruction_id =
            SelectedInstructionId((*self.next_instruction).try_into().map_err(|_| error())?);
        *self.next_instruction += 1;
        let output = VirtualRegisterId(self.registers.len().try_into().map_err(|_| error())?);
        let mut operands = inputs.to_vec();
        operands.push(output);
        let instruction = super::super::constraints::instruction(
            instruction_id,
            kind,
            key,
            &operands,
            provenance(self.edge),
            self.catalog,
        )?;
        self.registers.push(VirtualRegister {
            id: output,
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| error())?,
            ),
            class: instruction.operands.last().ok_or_else(error)?.class,
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
}

#[allow(clippy::too_many_arguments)]
pub(super) fn snapshot(
    function: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: EdgeId,
    bindings: &[SelectedStructuralBinding],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<Vec<Snapshot>, SelectedInstructionError> {
    let error = || super::invalid(function);
    let mut emission = Emission {
        function,
        registers,
        memory,
        instructions,
        next_instruction,
        edge,
        catalog,
    };
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
                constraints.keys.frame_address.ok_or_else(error)?,
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
                    constraints.keys.copy_i64
                } else {
                    constraints.keys.address_offset.ok_or_else(error)?
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
                    constraints.keys.load64.ok_or_else(error)?,
                    &[pointer],
                    place,
                    byte_offset,
                    None,
                )?;
                emission.source_reads(binding, byte_offset)?;
                let codec_offset = byte_offset.checked_add(8).ok_or_else(error)?;
                let codec = emission.emit(
                    SelectedInstructionKind::Load64 { byte_offset: 8 },
                    constraints.keys.load64.ok_or_else(error)?,
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
                    constraints.keys.materialize_i64,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn store(
    function: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: EdgeId,
    snapshots: &[Snapshot],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let error = || super::invalid(function);
    let key = constraints.keys.store64.ok_or_else(error)?;
    let row = super::super::constraints::row(catalog, key)?;
    for snapshot in snapshots {
        for (byte_offset, register) in [(0, snapshot.address), (8, snapshot.codec)] {
            let instruction_id =
                SelectedInstructionId((*next_instruction).try_into().map_err(|_| error())?);
            *next_instruction += 1;
            let operands = super::super::constraints::store_registers(
                instruction_id,
                &[register],
                row,
                registers,
            )?;
            instructions.push(super::super::constraints::instruction(
                instruction_id,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(snapshot.destination),
                    byte_offset,
                },
                key,
                &operands,
                provenance(edge),
                catalog,
            )?);
            memory.push(SelectedMemoryAccess {
                instruction: instruction_id,
                origin: SelectedMemoryAccessOrigin::Edge(edge),
                place: snapshot.place,
                byte_offset,
                byte_count: 8,
                role: SelectedMemoryAccessRole::WriteLocal {
                    slot: snapshot.destination,
                },
            });
        }
    }
    Ok(())
}

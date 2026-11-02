//! Shared emission constructors for one admitted runtime-value spill: the
//! address/load reload pair and the definition's store sequence, each built by
//! one constructor so the proposal and the replay's own reconstruction emit
//! identical register and instruction streams. The admission derivation these
//! consume is [`super::derive_runtime_spill`].

use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};

use super::{RuntimeSpillDerivation, RuntimeSpillError, VictimLineage};

pub(super) fn fresh(next: &mut u32) -> Result<u32, RuntimeSpillError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(result)
}

/// The private address computation and load inserted for one admitted use, at
/// an instruction operand or at a terminator operand — plus, for a
/// foreign-class victim, the carrier register the load fills and the
/// `from_bits` conversion restoring the reload's own class. Proposal and
/// replay build the identical sequence from this one constructor; the consumer
/// operand keeps its own access, class, and any fixed ABI view while only the
/// referenced register changes.
pub(super) struct Reload {
    pub address_register: VirtualRegister,
    /// The `Load64` result holding the victim's raw payload — present only
    /// when the victim is foreign-class and `convert` consumes it. A direct
    /// spill's load defines the reload register instead.
    pub carrier_register: Option<VirtualRegister>,
    pub reload_register: VirtualRegister,
    pub address: SelectedInstruction,
    pub load: SelectedInstruction,
    /// `from_bits` restoring the loaded payload into the victim's class.
    pub convert: Option<SelectedInstruction>,
}

impl Reload {
    /// Produced registers in declaration order, then the emitted instructions
    /// in stream order — the single sequence proposal and replay share.
    pub(super) fn into_streams(self) -> (Vec<VirtualRegister>, Vec<SelectedInstruction>) {
        (
            [
                Some(self.address_register),
                self.carrier_register,
                Some(self.reload_register),
            ]
            .into_iter()
            .flatten()
            .collect(),
            [Some(self.address), Some(self.load), self.convert]
                .into_iter()
                .flatten()
                .collect(),
        )
    }
}

pub(super) fn reload(
    admitted: &RuntimeSpillDerivation<'_>,
    slot: LocalStorageSlotId,
    register: VirtualRegisterId,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<Reload, RuntimeSpillError> {
    let conversion = admitted.bits_conversion;
    let address_instruction = SelectedInstructionId(fresh(next_instruction)?);
    let load_instruction = SelectedInstructionId(fresh(next_instruction)?);
    let convert_instruction = if conversion.is_some() {
        Some(SelectedInstructionId(fresh(next_instruction)?))
    } else {
        None
    };
    let address_register = VirtualRegisterId(fresh(next_register)?);
    let carrier_id = if conversion.is_some() {
        Some(VirtualRegisterId(fresh(next_register)?))
    } else {
        None
    };
    let reload_register = VirtualRegisterId(fresh(next_register)?);
    // The register the load defines: the reload itself when the victim rides
    // the slot rows directly, or the carrier holding its raw bits.
    let loaded = carrier_id.unwrap_or(reload_register);
    // The instruction whose result restates the victim — the load for a
    // direct spill, the `from_bits` conversion for a transported one.
    let restating = convert_instruction.unwrap_or(load_instruction);
    Ok(Reload {
        address_register: VirtualRegister {
            id: address_register,
            scalar_type: admitted.address_scalar_type,
            class: admitted.carrier_class,
            origin: VirtualRegisterOrigin::SpillAddress {
                instruction: address_instruction,
                register,
            },
            definition_site: None,
            entry_fixed_view: None,
        },
        carrier_register: carrier_id.map(|id| VirtualRegister {
            id,
            scalar_type: admitted.address_scalar_type,
            class: admitted.carrier_class,
            origin: match admitted.lineage {
                VictimLineage::Scalar(source_value) => VirtualRegisterOrigin::InstructionResult {
                    instruction: load_instruction,
                    source_value,
                },
                VictimLineage::Structural { place, byte_offset } => {
                    VirtualRegisterOrigin::StructuralObservation {
                        instruction: load_instruction,
                        place,
                        byte_offset,
                    }
                }
            },
            definition_site: admitted.victim.definition_site,
            entry_fixed_view: None,
        }),
        reload_register: VirtualRegister {
            id: reload_register,
            scalar_type: admitted.victim.scalar_type,
            class: admitted.victim.class,
            origin: match admitted.lineage {
                VictimLineage::Scalar(source_value) => VirtualRegisterOrigin::InstructionResult {
                    instruction: restating,
                    source_value,
                },
                // A structural victim restates no source value; the reload
                // re-observes the victim's own place and byte offset from
                // private storage — offset zero for a boundary live-in's
                // whole-place pointer, or the instruction-defined register's
                // declared coordinate, so a reload serving a case-payload
                // argument still names the exact field the transport asks for.
                VictimLineage::Structural { place, byte_offset } => {
                    VirtualRegisterOrigin::StructuralObservation {
                        instruction: restating,
                        place,
                        byte_offset,
                    }
                }
            },
            definition_site: admitted.victim.definition_site,
            entry_fixed_view: None,
        },
        address: crate::rewrites::instruction(
            address_instruction,
            SelectedInstructionKind::FrameAddress {
                slot: frame(slot),
                byte_offset: 0,
            },
            admitted.address,
            &[address_register],
        ),
        load: crate::rewrites::instruction(
            load_instruction,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            admitted.load,
            &[address_register, loaded],
        ),
        convert: conversion.map(|conversion| {
            crate::rewrites::instruction(
                convert_instruction.unwrap_or(restating),
                conversion.from_kind,
                conversion.from_bits,
                &[loaded, reload_register],
            )
        }),
    })
}

/// The sequence landing one definition's value in the slot: a victim in the
/// carrier class is the lone `Store64`, while a foreign-class victim's
/// payload first crosses `to_bits` into the carrier register the store then
/// writes. Proposal and replay build the identical sequence from this one
/// constructor, so the produced register and instruction streams stay in the
/// declaration order both sides share.
pub(super) struct Store {
    /// The `to_bits` result the store writes — present only for a converted
    /// victim.
    pub carrier_register: Option<VirtualRegister>,
    /// The target's frame-address temporary, independent of the stored payload.
    address_scratch: Option<VirtualRegister>,
    /// `to_bits` exposing the stored register's payload in the carrier class.
    pub convert: Option<SelectedInstruction>,
    pub store: SelectedInstruction,
}

impl Store {
    /// Produced registers in declaration order, then the emitted instructions
    /// in stream order — conversion ahead of the `Store64`.
    pub(super) fn into_streams(self) -> (Vec<VirtualRegister>, Vec<SelectedInstruction>) {
        (
            self.carrier_register
                .into_iter()
                .chain(self.address_scratch)
                .collect(),
            [self.convert, Some(self.store)]
                .into_iter()
                .flatten()
                .collect(),
        )
    }
}

pub(super) fn store(
    admitted: &RuntimeSpillDerivation<'_>,
    slot: LocalStorageSlotId,
    stored: VirtualRegisterId,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<Store, RuntimeSpillError> {
    let conversion = admitted.bits_conversion;
    let convert_instruction = if conversion.is_some() {
        Some(SelectedInstructionId(fresh(next_instruction)?))
    } else {
        None
    };
    let store_instruction = SelectedInstructionId(fresh(next_instruction)?);
    let carrier_id = if conversion.is_some() {
        Some(VirtualRegisterId(fresh(next_register)?))
    } else {
        None
    };
    // The register the slot write reads: the stored value itself, or the
    // carrier holding its raw bits after `to_bits`.
    let stored_register = carrier_id.unwrap_or(stored);
    let address_scratch = admitted
        .store
        .operands
        .get(1)
        .map(|operand| {
            Ok::<_, RuntimeSpillError>(VirtualRegister {
                id: VirtualRegisterId(fresh(next_register)?),
                scalar_type: admitted.address_scalar_type,
                class: operand.class,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: store_instruction,
                    operand: operand.operand,
                },
                definition_site: None,
                entry_fixed_view: None,
            })
        })
        .transpose()?;
    let store_registers = std::iter::once(stored_register)
        .chain(address_scratch.as_ref().map(|register| register.id))
        .collect::<Vec<_>>();
    Ok(Store {
        carrier_register: carrier_id.map(|id| VirtualRegister {
            id,
            scalar_type: admitted.address_scalar_type,
            class: admitted.carrier_class,
            origin: match admitted.lineage {
                VictimLineage::Scalar(source_value) => VirtualRegisterOrigin::InstructionResult {
                    instruction: convert_instruction.unwrap_or(store_instruction),
                    source_value,
                },
                VictimLineage::Structural { place, byte_offset } => {
                    VirtualRegisterOrigin::StructuralObservation {
                        instruction: convert_instruction.unwrap_or(store_instruction),
                        place,
                        byte_offset,
                    }
                }
            },
            definition_site: admitted.victim.definition_site,
            entry_fixed_view: None,
        }),
        address_scratch,
        convert: conversion.map(|conversion| {
            crate::rewrites::instruction(
                convert_instruction.unwrap_or(store_instruction),
                conversion.to_kind,
                conversion.to_bits,
                &[stored, stored_register],
            )
        }),
        store: crate::rewrites::instruction(
            store_instruction,
            SelectedInstructionKind::Store64 {
                slot: frame(slot),
                byte_offset: 0,
            },
            admitted.store,
            &store_registers,
        ),
    })
}

pub(super) fn frame(slot: LocalStorageSlotId) -> FrameStorageSlotId {
    FrameStorageSlotId::Local(slot)
}

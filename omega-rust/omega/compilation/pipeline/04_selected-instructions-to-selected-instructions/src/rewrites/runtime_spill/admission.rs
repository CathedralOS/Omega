use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterClassId, RegisterInstructionConstraint};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::ScalarType;

use super::{
    BitsConversion, RuntimeSpillError, StorageDefinition, StructuralArgumentUse,
    VictimLineage, slot,
};
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub use_blocks: Vec<usize>,
    /// Per block: one reload register serves consecutive flexible uses until
    /// an instruction that can destroy register content — a clobber or an
    /// implicit definition — closes it and the next use opens a fresh pair.
    /// `false` keeps each use on a private reload pair. A use recorded in
    /// `dedicated_tie_uses` keeps a private pair even where this holds.
    pub shared_reload: Vec<bool>,
    /// Per block: `(instruction index, operand ordinal)` of each victim use
    /// whose tied `Def` is early-clobber while an unpinned victim-reading
    /// co-operand could share the block's open reload register — the write
    /// may then land in that register ahead of the co-operand's read. The
    /// tied use takes a dedicated pair so the write's tied home is a
    /// register no other operand reads. Consulted only where `shared_reload`
    /// holds: a private-pair block already leaves every use unshared.
    pub dedicated_tie_uses: Vec<std::collections::BTreeSet<(usize, u16)>>,
    /// Per block: the source instruction indices that close the still-open
    /// shared reload — every instruction redefining the victim, whose later
    /// `Def` or `UseDef` operand makes the held value stale the way a unit
    /// writer makes it unsafe, plus every clobbering or implicitly-defining
    /// instruction
    /// the span policy did not cross. Under
    /// `RuntimeSpillSpanPolicy::UnitWriteCrossing` a unit-writing instruction
    /// stays off this set while an allocatable view of the victim's class
    /// avoids every unit written inside the span so far — the surviving home
    /// the produced interval then demands of the allocator. Under
    /// `UnitWriteBounded`, the historical shape, every unit writer is a
    /// member. Proposal, replay, and the slot-reuse analysis all read this
    /// one decision.
    pub span_closes: Vec<std::collections::BTreeSet<usize>>,
    // Establishment retains its exact address operand; only later uses reload.
    pub retained_uses: std::collections::BTreeSet<(SelectedInstructionId, u16)>,
    pub victim: &'source VirtualRegister,
    /// What the spill restates. A scalar victim keeps its source `ValueId`;
    /// a structural victim — an ABI live-in's incoming pointer, a field
    /// observation load's result, or an instruction-made transport register —
    /// is identified by the declared place and byte offset it restates and
    /// names no source value at all, so value bindings cannot transport it
    /// and its reload registers carry an observation origin, not a result
    /// one.
    pub lineage: VictimLineage,
    pub address_scalar_type: ScalarType,
    pub definitions: Vec<StorageDefinition>,
    /// The slot every emitted store and reload pair names. Each storage
    /// window — the definitions one reload position may observe and the
    /// positions they reach — independently takes the first declared `Spill`
    /// slot the last-writer replay proves disjoint from its accesses; a
    /// window no candidate admits falls back to the victim's private slot.
    /// `declares_private` decides whether the rewrite appends that
    /// declaration: `false` means every window landed on storage already
    /// charged to earlier victims, so frame demand stays unchanged and the
    /// bytes are never counted a second time.
    pub assignment: slot::Assignment,
    /// Stored structural-transport arguments naming the victim, one per
    /// binding. The snapshot's chunk loads are the real uses and were admitted
    /// as ordinary instruction operands; `loads` holds those load identities
    /// in byte order so proposal and replay can retarget the binding's
    /// `argument` field to the single register the loads name after rewriting.
    pub structural_uses: Vec<StructuralArgumentUse>,
    pub first_instruction: u32,
    pub first_register: u32,
    pub address: &'source RegisterInstructionConstraint,
    pub load: &'source RegisterInstructionConstraint,
    pub store: &'source RegisterInstructionConstraint,
    /// The class the private slot's access stream moves end to end — the
    /// victim's own class when it rides the rows directly, else the single
    /// class the frame-address, load, and store rows share, reached through
    /// the declared conversion pair below.
    pub carrier_class: RegisterClassId,
    /// `Some` when the victim's class cannot sit in the slot rows: its raw
    /// payload crosses the pair instead, so each store becomes `to_bits` then
    /// `Store64` and each reload appends `from_bits` after its `Load64`.
    pub bits_conversion: Option<BitsConversion<'source>>,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    span_policy: super::RuntimeSpillSpanPolicy,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RuntimeSpillError> { 
    let derived = super::derive_runtime_spill(
        source,
        function_index,
        register,
        environment,
        span_policy,
        budget,
    )?;
    Ok(Admission {
        function: derived.function,
        use_blocks: derived.use_blocks,
        shared_reload: derived.shared_reload,
        dedicated_tie_uses: derived.dedicated_tie_uses,
        span_closes: derived.span_closes,
        retained_uses: derived.retained_uses,
        victim: derived.victim,
        lineage: derived.lineage,
        address_scalar_type: derived.address_scalar_type,
        definitions: derived.definitions,
        assignment: derived.assignment,
        structural_uses: derived.structural_uses,
        first_instruction: derived.first_instruction,
        first_register: derived.first_register,
        address: derived.address,
        load: derived.load,
        store: derived.store,
        carrier_class: derived.carrier_class,
        bits_conversion: derived.bits_conversion,
    })
}

/// The declared bit-preserving pair bridging a foreign-class victim to the
/// slot's carrier class. Only an IEEE-float payload has such a pair today;
/// each row must be the exact two-operand `Use`/`Def` bridge between the two
/// classes with no hidden effects — a conversion that wrote or clobbered a
/// unit could destroy the still-open reload it sits inside. A missing key, an
/// absent row, or a row that does not bridge the two classes is a victim

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
    admitted: &Admission<'_>,
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
    admitted: &Admission<'_>,
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

/// Target rows supply the complete operand/effect interface, not guessed ISA conventions.

pub(super) fn frame(slot: LocalStorageSlotId) -> FrameStorageSlotId {
    FrameStorageSlotId::Local(slot)
}

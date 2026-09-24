//! Independent validation of the runtime-value spill.
//!
//! The validator never calls [`super::admission`]: it re-derives the spill's
//! legality from the source records — the named register's roster row, origin,
//! scalar payload and definition-site custody; the exact edge or boundary
//! definitions the contract demands; the use scan over instruction operands,
//! terminator operands, outgoing edge transports and case payloads; the
//! dominance the definition anchor must hold over every use block; the frame
//! rows' declared operand interface, the carrier class and any bit-preserving
//! conversion pair; the span policy's open/close decision and the structural
//! snapshot's chunk decomposition; the slot-reuse analysis over its own
//! derived positions; and the measured-step budget — then rebuilds the
//! expected fresh registers, stores and reload pairs itself and requires the
//! proposal to carry exactly them before restoring the complete source by
//! content. A producer admission error therefore fails validation even when
//! the proposal is exactly what that producer emitted.
//!
//! The shared `control`, `require_dominated_uses`, `slot::shared_slot`,
//! `stored_transport_size`, `next_chunk` and `surviving_home_exists` helpers
//! stay shared: the audit reads the same contract, not the producer's record.

use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockOrigin, SelectedBoundarySettlementPayload,
    SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedLocalStorageSlot,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
    SelectedStructuralTransport, SelectedSuccessorRole, SelectedTerminator, SelectedValueTransport,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    EdgeId, IeeeFloatFormat, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, ValueId,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    BitsConversion, RuntimeSpillError, RuntimeSpillReceipt, RuntimeSpillSpanPolicy,
    StorageDefinition, StoragePosition, StructuralArgumentUse, ValidatedRuntimeSpill,
    VictimLineage, control, control_mut, control_successors_mut, next_chunk,
    require_dominated_uses, slot, stored_transport_size, surviving_home_exists,
};
use crate::ValidatedSelectedAnalysis;

/// The validator's own reconstruction of the spill the contract permits: the
/// admitted victim, its proven definitions, the use blocks the spill serves,
/// the span decision, the resolved slot, the frame rows and conversion pair,
/// and the first fresh instruction and register identities the insertions
/// consume. It shares no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    use_blocks: Vec<usize>,
    /// Per block: one reload register serves consecutive flexible uses until
    /// an instruction that can destroy register content — a clobber or an
    /// implicit definition — closes it and the next use opens a fresh pair.
    /// `false` keeps each use on a private reload pair. A use recorded in
    /// `dedicated_tie_uses` keeps a private pair even where this holds.
    shared_reload: Vec<bool>,
    /// Per block: `(instruction index, operand ordinal)` of each victim use
    /// whose tied `Def` is early-clobber while an unpinned victim-reading
    /// co-operand could share the block's open reload register: the tied use
    /// takes a dedicated pair so the write's tied home is a register no
    /// other operand reads.
    dedicated_tie_uses: Vec<std::collections::BTreeSet<(usize, u16)>>,
    /// Per block: the source instruction indices that close the still-open
    /// shared reload — every instruction redefining the victim, plus every
    /// clobbering or implicitly-defining instruction the span policy did not
    /// cross.
    span_closes: Vec<std::collections::BTreeSet<usize>>,
    victim: &'source VirtualRegister,
    /// What the spill restates, derived from the victim's origin: a scalar
    /// keeps its source `ValueId`; a structural register keeps its declared
    /// place and byte offset and names no source value.
    lineage: VictimLineage,
    address_scalar_type: ScalarType,
    definitions: Vec<StorageDefinition>,
    /// The physical slot the rewrite addresses — the appended private slot,
    /// or an already-declared `Spill` slot whose storage windows the
    /// last-writer replay proved disjoint from this victim's.
    slot: LocalStorageSlotId,
    /// `true` when `slot` is the victim's own private slot and the rewrite
    /// must append its `local_storage_slots` declaration.
    fresh_slot: bool,
    /// Stored structural-transport arguments naming the victim, one per
    /// binding, with the snapshot chunk-load identities in byte order.
    structural_uses: Vec<StructuralArgumentUse>,
    first_instruction: u32,
    first_register: u32,
    address: &'source RegisterInstructionConstraint,
    load: &'source RegisterInstructionConstraint,
    store: &'source RegisterInstructionConstraint,
    /// The class the private slot's access stream moves end to end.
    carrier_class: RegisterClassId,
    /// `Some` when the victim's class cannot sit in the slot rows: its raw
    /// payload crosses the declared conversion pair instead.
    bits_conversion: Option<BitsConversion<'source>>,
}

/// The expected fresh registers and instruction stream for one admitted use:
/// the private `FrameAddress`/`Load64` pair plus, for a foreign-class victim,
/// the carrier register the load fills and the `from_bits` conversion
/// restoring the reload's own class. The validator builds this pair from its
/// own reconstruction so a wrong emission inside the producer's constructor
/// cannot launder through both sides.
struct ExpectedReload {
    address_register: VirtualRegister,
    /// The `Load64` result holding the victim's raw payload — present only
    /// when the victim is foreign-class and `convert` consumes it. A direct
    /// spill's load defines the reload register instead.
    carrier_register: Option<VirtualRegister>,
    reload_register: VirtualRegister,
    address: SelectedInstruction,
    load: SelectedInstruction,
    /// `from_bits` restoring the loaded payload into the victim's class.
    convert: Option<SelectedInstruction>,
}

impl ExpectedReload {
    /// Expected registers in declaration order, then the emitted instructions
    /// in stream order — the single sequence the replay consumes.
    fn into_streams(self) -> (Vec<VirtualRegister>, Vec<SelectedInstruction>) {
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

/// The expected sequence landing one definition's value in the slot: a victim
/// in the carrier class is the lone `Store64`, while a foreign-class victim's
/// payload first crosses `to_bits` into the carrier register the store then
/// writes.
struct ExpectedStore {
    /// The `to_bits` result the store writes — present only for a converted
    /// victim.
    carrier_register: Option<VirtualRegister>,
    /// `to_bits` exposing the stored register's payload in the carrier class.
    convert: Option<SelectedInstruction>,
    store: SelectedInstruction,
}

impl ExpectedStore {
    /// Expected registers in declaration order, then the emitted instructions
    /// in stream order — conversion ahead of the `Store64`.
    fn into_streams(self) -> (Vec<VirtualRegister>, Vec<SelectedInstruction>) {
        (
            self.carrier_register.into_iter().collect(),
            [self.convert, Some(self.store)]
                .into_iter()
                .flatten()
                .collect(),
        )
    }
}

fn fresh(next: &mut u32) -> Result<u32, RuntimeSpillError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(result)
}

/// The instruction the audit expects for one emitted row: the target row's
/// complete operand/effect interface, not guessed ISA conventions.
fn expected_instruction(
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    row: &RegisterInstructionConstraint,
    registers: &[VirtualRegisterId],
) -> SelectedInstruction {
    SelectedInstruction {
        id,
        kind,
        constraint: row.key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: *register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: Default::default(),
    }
}

/// The expected address/load pair for one admitted use, built from the
/// validator's own reconstruction: the victim's exact type, class, lineage
/// and definition-site custody, the resolved slot, the frame rows' operand
/// interface, and the declared conversion pair when the victim cannot ride
/// the slot rows directly.
fn expected_reload(
    reconstructed: &Reconstructed<'_>,
    register: VirtualRegisterId,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<ExpectedReload, RuntimeSpillError> {
    let conversion = reconstructed.bits_conversion;
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
    Ok(ExpectedReload {
        address_register: VirtualRegister {
            id: address_register,
            scalar_type: reconstructed.address_scalar_type,
            class: reconstructed.carrier_class,
            origin: VirtualRegisterOrigin::SpillAddress {
                instruction: address_instruction,
                register,
            },
            definition_site: None,
            entry_fixed_view: None,
        },
        carrier_register: carrier_id.map(|id| VirtualRegister {
            id,
            scalar_type: reconstructed.address_scalar_type,
            class: reconstructed.carrier_class,
            origin: match reconstructed.lineage {
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
            definition_site: reconstructed.victim.definition_site,
            entry_fixed_view: None,
        }),
        reload_register: VirtualRegister {
            id: reload_register,
            scalar_type: reconstructed.victim.scalar_type,
            class: reconstructed.victim.class,
            origin: match reconstructed.lineage {
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
            definition_site: reconstructed.victim.definition_site,
            entry_fixed_view: None,
        },
        address: expected_instruction(
            address_instruction,
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(reconstructed.slot),
                byte_offset: 0,
            },
            reconstructed.address,
            &[address_register],
        ),
        load: expected_instruction(
            load_instruction,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            reconstructed.load,
            &[address_register, loaded],
        ),
        convert: conversion.map(|conversion| {
            expected_instruction(
                convert_instruction.unwrap_or(restating),
                conversion.from_kind,
                conversion.from_bits,
                &[loaded, reload_register],
            )
        }),
    })
}

/// The expected store sequence for one definition, built from the validator's
/// own reconstruction: the `Store64` reading the proven stored register
/// directly, or the `to_bits` conversion exposing the payload in the carrier
/// class ahead of it.
fn expected_store(
    reconstructed: &Reconstructed<'_>,
    stored: VirtualRegisterId,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<ExpectedStore, RuntimeSpillError> {
    let conversion = reconstructed.bits_conversion;
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
    Ok(ExpectedStore {
        carrier_register: carrier_id.map(|id| VirtualRegister {
            id,
            scalar_type: reconstructed.address_scalar_type,
            class: reconstructed.carrier_class,
            origin: match reconstructed.lineage {
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
            definition_site: reconstructed.victim.definition_site,
            entry_fixed_view: None,
        }),
        convert: conversion.map(|conversion| {
            expected_instruction(
                convert_instruction.unwrap_or(store_instruction),
                conversion.to_kind,
                conversion.to_bits,
                &[stored, stored_register],
            )
        }),
        store: expected_instruction(
            store_instruction,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(reconstructed.slot),
                byte_offset: 0,
            },
            reconstructed.store,
            &[stored_register],
        ),
    })
}

/// Reconstruct the legality of spilling `register` into private storage from
/// the source records: the victim must carry a supported origin — an
/// instruction result, an incoming block or function parameter, a structural
/// parameter's live-in pointer, the hidden aggregate-result destination, or
/// an instruction-defined structural coordinate — with a scalar payload,
/// definition-site custody matching its origin, and no non-boundary entry
/// pin; its slot must not already be declared; every definition must resolve
/// to an exact site — the producing instruction, one dedicated edge
/// definition per predecessor, or the function-entry boundary with no
/// re-entry edge; every operand, terminator, transport and case-payload use
/// must be flexible, untied and non-early-clobber in the victim's own class
/// (a `Def` becomes a tracked redefinition, a `UseDef` the read-modify-write
/// store); the frame rows must expose the declared operand interface with no
/// hidden effects; the definition anchor must dominate every use block; and
/// the span, structural-snapshot and slot-reuse decisions follow the recorded
/// facts. The audit charges the family's measured steps against the budget.
/// Nothing in it reads the producer's admission decision, so a producer-side
/// legality error fails here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    span_policy: RuntimeSpillSpanPolicy,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, RuntimeSpillError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RuntimeSpillError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RuntimeSpillError::SourceMismatch)?;
    let victim = function
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64)
        .map_err(|_| RuntimeSpillError::UnsupportedValue)?;
    // Every register the representation's own entry-liveness replay
    // recognizes — a scalar or structural parameter, or the hidden
    // aggregate-result destination — has the function-entry boundary as its
    // definition anchor. Its ABI live-in pin is its own boundary constraint:
    // rewriting shrinks that register's interval to the entry-to-store
    // window, so the view still pins exactly where the value must arrive. A
    // fixed view on any other origin stays a rejection.
    let entry_boundary = function.is_entry_register(victim);
    let (lineage, definition, block_index) = match victim.origin {
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            let block_index = function
                .blocks
                .iter()
                .position(|block| {
                    block
                        .instructions
                        .iter()
                        .any(|candidate| candidate.id == instruction)
                })
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            (
                VictimLineage::Scalar(source_value),
                Some(instruction),
                block_index,
            )
        }
        VirtualRegisterOrigin::BlockParameter {
            source_value,
            block,
            parameter_index,
        } => {
            let block_index = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == block)
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            if block == function.entry_block
                || !matches!(victim.definition_site,
                Some(ValueDefinitionSite::BlockParameter { block: semantic_block, position })
                    if semantic_block == function.blocks[block_index].source_block()
                        && position as usize == parameter_index)
            {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            (VictimLineage::Scalar(source_value), None, block_index)
        }
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            // The entry block is the definition anchor: the parameter arrives
            // live-in, so its store opens that block and dominates every use
            // without any instruction or incoming edge to lean on.
            let block_index = function
                .blocks
                .iter()
                .position(|block| block.id == function.entry_block)
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            if !matches!(victim.definition_site,
                Some(ValueDefinitionSite::FunctionParameter(position))
                    if position as usize == parameter_index)
            {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            (VictimLineage::Scalar(source_value), None, block_index)
        }
        VirtualRegisterOrigin::StructuralParameter {
            place,
            parameter_index,
        } => {
            // A structural parameter is the same boundary live-in: the ABI
            // hands in the pointer or fragment register pinned to its entry
            // view. Its provenance is the structural contract's own
            // parameter row — the declared place at this index — and it
            // carries no source definition site.
            let block_index = function
                .blocks
                .iter()
                .position(|block| block.id == function.entry_block)
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            if victim.definition_site.is_some()
                || !function
                    .structural
                    .as_ref()
                    .and_then(|contract| contract.parameters.get(parameter_index))
                    .is_some_and(|parameter| {
                        parameter.semantic.place == place && parameter.target.place == place
                    })
            {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            (
                VictimLineage::Structural {
                    place,
                    byte_offset: 0,
                },
                None,
                block_index,
            )
        }
        VirtualRegisterOrigin::AbiTransport { place, .. } if entry_boundary => {
            // The hidden aggregate-result destination is the only transport
            // register that is a boundary live-in rather than an
            // instruction-made address: `is_entry_register` has already
            // proved instruction zero, offset zero, the pinned view, no site,
            // and the declared result place.
            let block_index = function
                .blocks
                .iter()
                .position(|block| block.id == function.entry_block)
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            (
                VictimLineage::Structural {
                    place,
                    byte_offset: 0,
                },
                None,
                block_index,
            )
        }
        VirtualRegisterOrigin::StructuralObservation {
            instruction,
            place,
            byte_offset,
        }
        | VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        } => {
            // An instruction-defined structural register — a field
            // observation's load result, a retained transport pointer, or a
            // snapshot chunk word — is defined by its producing instruction
            // exactly like a scalar result, but restates no `ValueId`: its
            // provenance is the place and byte offset the origin declares,
            // which the reload's observation origin carries on.
            let block_index = function
                .blocks
                .iter()
                .position(|block| {
                    block
                        .instructions
                        .iter()
                        .any(|candidate| candidate.id == instruction)
                })
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            (
                VictimLineage::Structural { place, byte_offset },
                Some(instruction),
                block_index,
            )
        }
        _ => return Err(RuntimeSpillError::UnsupportedValue),
    };
    // This preserves a full GPR in its own eight-byte slot, not a source
    // referent. Narrow values keep their exact type and all resident bits;
    // neither signed widening nor a wider read of source storage is needed.
    // The integer carrier is payload metadata the reload register retains:
    // an address-carrier value round-trips its bits through private storage
    // exactly like a fixed one — a materialized address constant is what
    // rematerialization (not this rewrite) still refuses to invent.
    let scalar_payload = match victim.scalar_type {
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => true,
        ScalarType::Integer(integer) => matches!(integer.bits(), 8 | 16 | 32 | 64),
    };
    // Semantic lineage alone does not locate physical storage definitions.
    // Instruction results and incoming parameters establish those separately.
    // Structural registers carry no source definition site at all — the
    // boundary is a live-in's definition and an observation or transport
    // register's own instruction is its definition — while every other
    // admitted origin must keep its declared site.
    if !scalar_payload
        || (victim.entry_fixed_view.is_some() && !entry_boundary)
        || !(matches!(
            victim.definition_site,
            Some(ValueDefinitionSite::FunctionParameter(_))
                | Some(ValueDefinitionSite::BlockParameter { .. })
                | Some(ValueDefinitionSite::Node { .. })
        ) || (victim.definition_site.is_none()
            && matches!(
                victim.origin,
                VirtualRegisterOrigin::StructuralParameter { .. }
                    | VirtualRegisterOrigin::AbiTransport { .. }
                    | VirtualRegisterOrigin::StructuralObservation { .. }
            )))
    {
        return Err(RuntimeSpillError::UnsupportedValue);
    }
    let private_slot = LocalStorageSlotId::Spill { register };
    if function
        .local_storage_slots
        .iter()
        .any(|storage| storage.id == private_slot)
    {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    let mut definitions = if let Some(instruction) = definition {
        vec![StorageDefinition {
            block_index,
            position: StoragePosition::AfterInstruction(instruction),
            register,
        }]
    } else if entry_boundary {
        entry_definitions(function, block_index, victim)?
    } else {
        // Only a block parameter reaches this arm: structural live-ins took
        // the boundary branch, and they carry no source value to check the
        // edge bindings against.
        let VictimLineage::Scalar(source_value) = lineage else {
            return Err(RuntimeSpillError::UnsupportedValue);
        };
        parameter_definitions(function, block_index, victim, source_value)?
    };
    // Per block: instruction indices holding a writing operand on the victim
    // beyond the origin definition — redefinitions. Each writes a new value
    // into the register, so each gains its own following store and each ends
    // the still-open shared reload: the value that register held no longer
    // restates the victim.
    let mut redefinitions: Vec<std::collections::BTreeSet<usize>> = function
        .blocks
        .iter()
        .map(|_| std::collections::BTreeSet::new())
        .collect();
    // The same per-block granularity for instructions whose recorded writer
    // is a `UseDef` operand: its store reads the operand's emitted register,
    // so a second victim-writing operand in the same instruction — in either
    // order — leaves the slot without a defined last writer and rejects.
    let mut usedef_writes: Vec<std::collections::BTreeSet<usize>> = function
        .blocks
        .iter()
        .map(|_| std::collections::BTreeSet::new())
        .collect();
    // Per block: `(instruction index, operand ordinal)` of each victim use
    // whose tied `Def` is early-clobber while an unpinned co-operand could
    // read the same reload register. The flag alone is no hazard — the write
    // can land only in the home its tie names — so the tied use takes a
    // dedicated pair where the block's sharing answer leaves a co-reader on
    // that register.
    let mut dedicated_tie_uses: Vec<std::collections::BTreeSet<(usize, u16)>> = function
        .blocks
        .iter()
        .map(|_| std::collections::BTreeSet::new())
        .collect();
    let mut defined = definition.is_none();
    let mut uses = 0usize;
    let mut use_blocks = Vec::new();
    // Blocks where at least one admitted use can read a shared block-local
    // reload: an unpinned instruction or terminator operand, or an
    // edge-transport argument. ABI-pinned (fixed-view) uses keep a private
    // reload pinned for their own point instead.
    let mut flexible_uses = vec![false; function.blocks.len()];
    // Exact positions of admitted uses, for the physical slot-reuse check:
    // its last-writer replay must see the same load points the rewrite emits.
    let mut use_positions: Vec<slot::BlockUsePositions> = function
        .blocks
        .iter()
        .map(|_| slot::BlockUsePositions::default())
        .collect();
    // Stored structural-transport arguments naming the victim. Their snapshot
    // chunk loads are ordinary operand uses the instruction scan below admits;
    // these records only pin down which loads belong to each binding so the
    // rewrite can retarget the binding's argument to one reload register.
    let mut structural_uses: Vec<StructuralArgumentUse> = Vec::new();
    // An instruction-defined victim may be produced by an address-forming
    // instruction — `FrameAddress`, `AddressOffset`, or `ByteViewAddress` —
    // whose resolved address round-trips its bits through private storage
    // like any other result. One operand position cannot follow the reload,
    // though: the primitive-local establishment replay binds a `WritePlace`
    // store's address operand to the `AddressLocal` `FrameAddress` result
    // verbatim, so when such an access names this victim's own definition for
    // an (operation, place) pair, the `WritePlace` store for that same pair
    // must keep the victim register — substituting a reload register would
    // orphan the recorded establishment. Reads, snapshot chunk loads, calls,
    // and the store's value operand are content-bound elsewhere — by source
    // value, by result origin, or by the access's own coordinates — so only
    // the paired store's instruction is refused here.
    let establishing: std::collections::BTreeSet<(OperationId, PlaceId)> = definition
        .into_iter()
        .flat_map(|definition| {
            function.memory_accesses.iter().filter_map(move |access| {
                match (access.origin, access.role) {
                    (
                        SelectedMemoryAccessOrigin::Operation(operation),
                        SelectedMemoryAccessRole::AddressLocal { .. },
                    ) if access.instruction == definition => Some((operation, access.place)),
                    _ => None,
                }
            })
        })
        .collect();
    let established_stores: std::collections::BTreeSet<SelectedInstructionId> = if establishing
        .is_empty()
    {
        std::collections::BTreeSet::new()
    } else {
        function
            .memory_accesses
            .iter()
            .filter_map(|access| match (access.origin, access.role) {
                (
                    SelectedMemoryAccessOrigin::Operation(operation),
                    SelectedMemoryAccessRole::WritePlace,
                ) if establishing.contains(&(operation, access.place)) => Some(access.instruction),
                _ => None,
            })
            .collect()
    };
    for (current_block_index, block) in function.blocks.iter().enumerate() {
        let previous_uses = uses;
        let (terminal, successors) = control(&block.terminator);
        // A terminator operand use executes after every block instruction, so
        // the same private reload serves it from the end of the block; the
        // dominance check below already covers that position. As on body
        // instruction operands, a fixed view stays attached and pins the fresh
        // reload register to the same physical unit, so ABI-pinned returns and
        // hosted exits stay exact. Tied, early-clobber, and defining
        // references stay rejected.
        for operand in &terminal.operands {
            if operand.virtual_register != register {
                continue;
            }
            match operand.access {
                RegisterOperandAccess::Use
                    if operand.tied_to.is_none()
                        && !operand.early_clobber
                        && operand.class == victim.class =>
                {
                    // An output tied to this use would extend the reload's
                    // value identity.
                    if terminal
                        .operands
                        .iter()
                        .any(|other| other.tied_to == Some(operand.operand))
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    if operand.fixed_view.is_none() {
                        flexible_uses[current_block_index] = true;
                    }
                    use_positions[current_block_index].end_of_block = true;
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                }
                _ => return Err(RuntimeSpillError::UnsupportedUse),
            }
        }
        for successor in successors.into_iter().flatten() {
            for binding in &successor.bindings {
                let SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = binding.transport
                else {
                    continue;
                };
                // The parameter side is the destination's incoming definition,
                // never a use in this block. Only a block-parameter victim's
                // own incoming edges may carry it; parameter_definitions has
                // already checked every such arrival.
                if parameter == register
                    && (definition.is_some() || successor.block != function.blocks[block_index].id)
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                if argument != register {
                    continue;
                }
                // An edge-transport argument reads the victim at the end of
                // this block. Its semantic declaration must name the victim's
                // source value and exact type; a structural live-in has none,
                // so any binding naming it is an inconsistent plan, not a use
                // this rewrite can serve.
                if lineage != VictimLineage::Scalar(binding.semantic.argument)
                    || binding.semantic.scalar_type != victim.scalar_type
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                flexible_uses[current_block_index] = true;
                use_positions[current_block_index].end_of_block = true;
                uses = uses
                    .checked_add(1)
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
            }
            for binding in &successor.structural_bindings {
                // An address transport's base register is carried as edge
                // evidence this rewrite does not relocate; spilling it stays
                // closed.
                if matches!(binding.transport,
                    SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Register(argument),
                        ..
                    } if argument == register)
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                let Some(byte_size) = stored_transport_size(binding.transport) else {
                    continue;
                };
                let (SelectedStructuralTransport::Descriptor { argument, .. }
                | SelectedStructuralTransport::WholeValue { argument, .. }) = binding.transport
                else {
                    continue;
                };
                if argument != register {
                    continue;
                }
                // A stored structural argument is read only by the bridge's
                // own snapshot chunk loads — the real uses, admitted as
                // ordinary instruction operands below. The binding is the
                // continuation's transfer record, so it must sit on the
                // edge-transfer continuation of the edge's own bridge, the
                // same idiom `parameter_definitions` enforces for the value
                // bindings on a block parameter's incoming edges.
                if successor.role != SelectedSuccessorRole::EdgeTransferContinuation
                    || !matches!(block.origin, SelectedBlockOrigin::EdgeTransfer { edge, target }
                        if edge == successor.psi_edge && target == successor.source_target)
                {
                    return Err(RuntimeSpillError::UnsupportedControlFlow);
                }
                // On a block-parameter victim's own incoming edge the edge's
                // definition store runs inside this same bridge; a snapshot
                // of the same register there could not be ordered against
                // that store, so that naming stays rejected.
                if definition.is_none() && successor.block == function.blocks[block_index].id {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                structural_uses.push(StructuralArgumentUse {
                    block: current_block_index,
                    edge: successor.psi_edge,
                    place: binding.semantic.argument.place,
                    byte_size,
                    loads: Vec::new(),
                    dedicated_reload: false,
                });
            }
            if let Some(case) = &successor.structural_case {
                for payload in &case.payloads {
                    match payload.transport {
                        SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            // The parameter side is the destination's payload
                            // definition, never a use in this block — the same
                            // rule the value bindings above keep: only a
                            // block-parameter victim's own incoming edges may
                            // carry it, and parameter_definitions has already
                            // checked every such arrival.
                            if parameter == register
                                && (definition.is_some()
                                    || successor.block != function.blocks[block_index].id)
                            {
                                return Err(RuntimeSpillError::UnsupportedUse);
                            }
                            if argument != register {
                                continue;
                            }
                            // A case-payload register transport reads the
                            // victim at this edge, like the value bindings.
                            // Its declared payload type must equal the
                            // victim's exact type; a mismatched plan is not a
                            // use this rewrite can serve.
                            if payload.semantic.parameter.scalar_type != victim.scalar_type {
                                return Err(RuntimeSpillError::UnsupportedUse);
                            }
                            // A structural victim may serve the argument only
                            // while its restated coordinate is exactly the
                            // case's declared place at this payload's field
                            // offset: the rewritten argument's observation
                            // origin carries those same coordinates, so any
                            // mismatch would break the transport's invariant
                            // rather than restore it.
                            if let VictimLineage::Structural { place, byte_offset } = lineage
                                && (Some(place) != case.slot.structural_place()
                                    || byte_offset != payload.semantic.field_byte_offset)
                            {
                                return Err(RuntimeSpillError::UnsupportedUse);
                            }
                            flexible_uses[current_block_index] = true;
                            use_positions[current_block_index].end_of_block = true;
                            uses = uses
                                .checked_add(1)
                                .ok_or(RuntimeSpillError::IdentityOverflow)?;
                        }
                        SelectedCasePayloadTransport::Unmaterialized { parameter }
                            if parameter == register =>
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        _ => {}
                    }
                }
            }
        }
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            for operand in &instruction.operands {
                if operand.virtual_register != register {
                    continue;
                }
                match operand.access {
                    RegisterOperandAccess::Def
                        if !defined
                            && current_block_index == block_index
                            && Some(instruction.id) == definition =>
                    {
                        defined = true;
                    }
                    // A later `Def` on the victim redefines the register: the
                    // instruction writes a new value into it, so the slot
                    // tracks that write with one store immediately after —
                    // the same rule the origin definition keeps. The operand
                    // itself keeps the victim register, so its class must
                    // agree; a tie onto a victim use binds the write to that
                    // use's reload register, and an early-clobber flag stays
                    // allocation detail on the unrewritten operand, while a
                    // use on the same instruction still reads the pre-write
                    // value through its own reload. A write ahead of the
                    // origin in the origin's own block — or a second
                    // definition inside the origin instruction — stays
                    // rejected: the register cannot be redefined before it
                    // exists.
                    RegisterOperandAccess::Def
                        if (defined || current_block_index != block_index)
                            && Some(instruction.id) != definition
                            && operand.class == victim.class =>
                    {
                        if redefinitions[current_block_index].insert(instruction_index) {
                            definitions.push(StorageDefinition {
                                block_index: current_block_index,
                                position: StoragePosition::AfterInstruction(instruction.id),
                                register,
                            });
                        } else if usedef_writes[current_block_index].contains(&instruction_index) {
                            // A second plain write still stores the victim's
                            // post-instruction value, but a `UseDef` already
                            // claimed this instruction's store for the reload
                            // register its operand was redirected to — with
                            // two writers the slot has no defined last writer.
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                    }
                    RegisterOperandAccess::Use
                        if (defined || current_block_index != block_index)
                            && Some(instruction.id) != definition
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                            && operand.class == victim.class =>
                    {
                        // A `WritePlace` store paired with the `AddressLocal`
                        // access on the victim's own definition keeps the
                        // address operand's register identity verbatim; the
                        // reload cannot stand in for it there.
                        if established_stores.contains(&instruction.id) {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        // A fixed view on this operand stays attached to the
                        // rewritten use, so the fresh reload register is a
                        // precolored segment pinned to that physical view for
                        // exactly the load-to-use window — the split the
                        // operand always needed, created by recovery instead
                        // of refusing the victim. A write tied to this use is
                        // admitted only as the victim's own redefinition — the
                        // two-operand read-modify-write idiom: the tie binds
                        // the write to this reload register's home and the
                        // store after the instruction reads that register
                        // through the victim operand the write kept. A tie
                        // from any other write would clobber the still-open
                        // reload with no store to mirror it. An early-clobber
                        // flag on the tied write is itself no hazard — the
                        // write lands only in the reload home its tie names —
                        // but it may complete before an unrelated operand's
                        // read, so an unpinned use that could share the
                        // block's open register with an unpinned
                        // victim-reading co-operand is recorded for a
                        // dedicated pair: the write's tied home is then a
                        // register no other operand reads.
                        let mut tied_writes = instruction
                            .operands
                            .iter()
                            .filter(|other| other.tied_to == Some(operand.operand));
                        let tied = tied_writes.next();
                        if let Some(other) = tied
                            && (other.access != RegisterOperandAccess::Def
                                || other.virtual_register != register
                                || tied_writes.next().is_some())
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        if let Some(other) = tied
                            && other.early_clobber
                            && operand.fixed_view.is_none()
                            && instruction.operands.iter().any(|co| {
                                co.operand != operand.operand
                                    && co.virtual_register == register
                                    && matches!(
                                        co.access,
                                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                                    )
                                    && co.fixed_view.is_none()
                            })
                        {
                            dedicated_tie_uses[current_block_index]
                                .insert((instruction_index, operand.operand));
                            // The dedicated pair's own load reads the slot at
                            // this position even when the block's open pair
                            // serves the co-readers, so the slot-reuse replay
                            // must see it.
                            let dedicated = &mut use_positions[current_block_index].dedicated;
                            if dedicated.last() != Some(&instruction_index) {
                                dedicated.push(instruction_index);
                            }
                        }
                        if operand.fixed_view.is_none() {
                            flexible_uses[current_block_index] = true;
                            let positions = &mut use_positions[current_block_index].unpinned;
                            if positions.last() != Some(&instruction_index) {
                                positions.push(instruction_index);
                            }
                        } else {
                            let positions = &mut use_positions[current_block_index].pinned;
                            if positions.last() != Some(&instruction_index) {
                                positions.push(instruction_index);
                            }
                        }
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    }
                    // A `UseDef` operand reads the victim and rewrites it in
                    // place — the one-operand form of the tied pair admitted
                    // above. The operand moves to a reload register like any
                    // use, the instruction writes its result back into that
                    // register, and the store after the instruction reads the
                    // emitted operand's register rather than the victim, which
                    // the rewritten instruction never defines. The read takes
                    // the still-open shared pair and the write ends it, the
                    // same span rule a plain redefinition keeps. Its own tie
                    // would bind a second register to the reload's home, an
                    // operand tied to it would clobber the reload without a
                    // mirrored store, an early-clobber write could precede a
                    // co-operand's read of the shared pair, and a second
                    // victim-writing operand on the same instruction would
                    // leave the slot without a defined last writer — all of
                    // those stay rejected.
                    RegisterOperandAccess::UseDef
                        if (defined || current_block_index != block_index)
                            && Some(instruction.id) != definition
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                            && operand.class == victim.class =>
                    {
                        if established_stores.contains(&instruction.id)
                            || instruction
                                .operands
                                .iter()
                                .any(|other| other.tied_to == Some(operand.operand))
                            || !redefinitions[current_block_index].insert(instruction_index)
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        usedef_writes[current_block_index].insert(instruction_index);
                        definitions.push(StorageDefinition {
                            block_index: current_block_index,
                            position: StoragePosition::AfterUseDef {
                                instruction: instruction.id,
                                operand: operand.operand,
                            },
                            register,
                        });
                        if operand.fixed_view.is_none() {
                            flexible_uses[current_block_index] = true;
                            let positions = &mut use_positions[current_block_index].unpinned;
                            if positions.last() != Some(&instruction_index) {
                                positions.push(instruction_index);
                            }
                        } else {
                            let positions = &mut use_positions[current_block_index].pinned;
                            if positions.last() != Some(&instruction_index) {
                                positions.push(instruction_index);
                            }
                        }
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    }
                    _ => return Err(RuntimeSpillError::UnsupportedUse),
                }
            }
        }
        if uses != previous_uses {
            use_blocks.push(current_block_index);
        }
    }
    if !defined || uses == 0 {
        return Err(RuntimeSpillError::UnsupportedValue);
    }
    let keys = environment.selected_keys();
    let address = environment
        .constraint(
            keys.frame_address
                .ok_or(RuntimeSpillError::ConstraintMismatch)?,
        )
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    let load = environment
        .constraint(keys.load64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    let store = environment
        .constraint(keys.store64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    // Frame accesses retain the validated target row's stack-pointer reads.
    // Ordinary pointer loads have only their explicit address operand.
    if !load.implicit_uses.is_empty() {
        return Err(RuntimeSpillError::ConstraintMismatch);
    }
    for (row, accesses) in [
        (address, &[RegisterOperandAccess::Def][..]),
        (
            load,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def][..],
        ),
        (store, &[RegisterOperandAccess::Use][..]),
    ] {
        if row.operands.len() != accesses.len()
            || !row.implicit_defs.is_empty()
            || !row.clobbers.is_empty()
        {
            return Err(RuntimeSpillError::ConstraintMismatch);
        }
        for (ordinal, (operand, access)) in row.operands.iter().zip(accesses).enumerate() {
            if usize::from(operand.operand) != ordinal
                || operand.access != *access
                || operand.fixed_view.is_some()
                || operand.tied_to.is_some()
                || operand.early_clobber
            {
                return Err(RuntimeSpillError::ConstraintMismatch);
            }
        }
    }
    // The slot's access stream moves one class end to end: the computed
    // address and every stored or loaded payload share it. A victim in
    // another class cannot ride the rows directly, but its stored bytes are
    // raw payload — when that payload is an IEEE float and the target
    // declares the matching bit-preserving pair, `to_bits` ahead of each
    // store and `from_bits` behind each load carry it across while the slot
    // keeps its eight-byte geometry. A victim with no usable pair is a
    // candidate-local rejection, not a constraint fault, so recovery may
    // try the next candidate.
    let carrier_class = address.operands[0].class;
    if [address, load, store]
        .iter()
        .flat_map(|row| row.operands.iter())
        .any(|operand| operand.class != carrier_class)
    {
        return Err(RuntimeSpillError::ConstraintMismatch);
    }
    let bits_conversion = if victim.class == carrier_class {
        None
    } else {
        Some(bits_transport(environment, victim, carrier_class)?)
    };
    // The reuse check scans this function once per declared local slot.
    let slot_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .and_then(|span| function.local_storage_slots.len().checked_mul(span))
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        // A converted victim emits one more instruction and one more
        // register at every store and every reload, so its replay charges
        // six per use and three per definition instead of four and one.
        .and_then(|total| {
            total.checked_add(uses.checked_mul(if bits_conversion.is_some() { 6 } else { 4 })?)
        })
        // A stored binding whose chunk loads cannot name one register emits a
        // dedicated end-of-block reload pair of its own — one use's work per
        // binding, charged for every recorded binding whether or not the
        // audit below decides its loads share.
        .and_then(|total| {
            total.checked_add(
                structural_uses
                    .len()
                    .checked_mul(if bits_conversion.is_some() { 6 } else { 4 })?,
            )
        })
        .and_then(|total| {
            total.checked_add(
                definitions
                    .len()
                    .checked_mul(if bits_conversion.is_some() { 3 } else { 1 })?,
            )
        })
        // An entry-bound register's boundary definition scans every block's
        // successor list once for a re-entry edge — the same per-block shape
        // the two dominance passes below already pay for.
        .and_then(|total| {
            total.checked_add(if entry_boundary {
                function.blocks.len()
            } else {
                0
            })
        })
        .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
        // The open/close decision replays once over the victim function's own
        // instructions — the crossing simulation under `UnitWriteCrossing`,
        // the span-close collection under either policy.
        .and_then(|total| {
            total.checked_add(function.blocks.iter().try_fold(0usize, |sum, block| {
                sum.checked_add(block.instructions.len())?.checked_add(1)
            })?)
        })
        .and_then(|total| total.checked_add(slot_scan))
        // The structural-argument check groups the memory accesses once, then
        // each pending binding walks its own access group. The establishment
        // pairing above scans them twice more — once for the `AddressLocal`
        // records on the victim's definition, once for the paired `WritePlace`
        // stores.
        .and_then(|total| total.checked_add(function.memory_accesses.len().checked_mul(3)?))
        .and_then(|total| {
            total.checked_add(
                structural_uses
                    .len()
                    .checked_mul(function.memory_accesses.len())?,
            )
        })
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RuntimeSpillError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RuntimeSpillError::WorkBudgetExceeded);
    }
    require_dominated_uses(function, block_index, &use_blocks)?;
    let first_instruction = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain([control(&block.terminator).0.id.0])
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    let first_register = function
        .virtual_registers
        .iter()
        .map(|value| value.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    // A unit implicitly used anywhere in the function can be live through a
    // shared span without appearing in the span's own effects: backward unit
    // liveness keeps it resident at every interior point, so a view
    // containing it cannot host an interval reaching across. The frame rows
    // the rewrite inserts contribute their own implicit reads at interior
    // points inside a shared interval, so their units are excluded the same
    // way. A fixed-view operand or entry-bound register anywhere in the
    // function precolors an interval that may reach across a span, so every
    // unit such a view occupies is excluded too.
    let mut implicit_use_units = std::collections::BTreeSet::new();
    let mut pinned_views = function
        .virtual_registers
        .iter()
        .filter_map(|register| register.entry_fixed_view)
        .collect::<Vec<_>>();
    for instruction in function.blocks.iter().flat_map(|block| {
        block
            .instructions
            .iter()
            .chain(std::iter::once(control(&block.terminator).0))
    }) {
        implicit_use_units.extend(instruction.implicit_uses.iter().copied());
        pinned_views.extend(
            instruction
                .operands
                .iter()
                .filter_map(|operand| operand.fixed_view),
        );
    }
    implicit_use_units.extend(address.implicit_uses.iter().copied());
    implicit_use_units.extend(load.implicit_uses.iter().copied());
    implicit_use_units.extend(store.implicit_uses.iter().copied());
    // A transported victim's conversions may read units of their own; they
    // sit inside the same open intervals, so their implicit units are
    // excluded from any home the shared reload demands.
    if let Some(conversion) = &bits_conversion {
        implicit_use_units.extend(conversion.to_bits.implicit_uses.iter().copied());
        implicit_use_units.extend(conversion.from_bits.implicit_uses.iter().copied());
    }
    let pinned_units = pinned_views
        .iter()
        .flat_map(|view_id| {
            environment
                .physical()
                .model()
                .views
                .get(usize::from(view_id.0))
                .into_iter()
                .flat_map(|view| view.units.iter().chain(&view.write_units))
        })
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    // One reload register stays open for consecutive flexible uses only
    // while the instructions between them cannot destroy register content;
    // a clobber or implicit definition closes it at rewrite time, so under
    // `UnitWriteBounded` no produced interval ever reaches across a call and
    // demands a callee-saved home recovery may not have.
    let empty_writes = std::collections::BTreeSet::new();
    let shared_reload: Vec<bool> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, _block)| {
            flexible_uses[block_index]
                && surviving_home_exists(
                    environment,
                    victim.class,
                    &implicit_use_units,
                    &pinned_units,
                    &empty_writes,
                )
        })
        .collect();
    // The deferred half of the early-clobber admission: the recorded hazard
    // is real only where the block actually keeps one reload register open
    // across unpinned uses — the write may then land in that shared register
    // ahead of the co-operand's read. The tied use takes a dedicated pair of
    // its own there, so the write's tied home is a register no co-reader
    // shares. Under a private-pair block every use is unshared already and
    // the record goes unread; either way the flag alone never rejects.
    // Under `UnitWriteCrossing` a unit-writing instruction instead keeps the
    // open pair while an allocatable view of the victim's class avoids every
    // unit written inside the span so far — the clobber-set-reduced, most
    // often callee-saved, home the produced interval then demands. The
    // simulation replays the rewrite's own open/close decision: the pair
    // opens at each unpinned use and, while open, a unit-writing instruction
    // either joins the crossed set — accumulating its writes into the span —
    // or ends the span. A redefinition always ends the span: it writes the
    // victim itself, so the held register's value no longer restates it.
    // Where no view survives, the instruction still closes the span and the
    // produced shape degrades to the bounded one.
    let crossed_unit_writes: Vec<std::collections::BTreeSet<usize>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            let mut crossed = std::collections::BTreeSet::new();
            if span_policy == RuntimeSpillSpanPolicy::UnitWriteCrossing
                && shared_reload[block_index]
            {
                let mut open = false;
                let mut written = std::collections::BTreeSet::new();
                for (index, instruction) in block.instructions.iter().enumerate() {
                    if use_positions[block_index].unpinned.contains(&index) {
                        open = true;
                    }
                    if redefinitions[block_index].contains(&index) {
                        open = false;
                        written.clear();
                        continue;
                    }
                    if instruction.clobbers.is_empty() && instruction.implicit_defs.is_empty()
                        || !open
                    {
                        continue;
                    }
                    let mut trial = written.clone();
                    trial.extend(instruction.clobbers.iter().copied());
                    trial.extend(instruction.implicit_defs.iter().copied());
                    if surviving_home_exists(
                        environment,
                        victim.class,
                        &implicit_use_units,
                        &pinned_units,
                        &trial,
                    ) {
                        written = trial;
                        crossed.insert(index);
                    } else {
                        open = false;
                        written.clear();
                    }
                }
            }
            crossed
        })
        .collect();
    // The shared span-closing decision, computed once: every victim
    // redefinition — the held reload's value stops restating the register —
    // and every unit-writing instruction the policy did not cross. Proposal,
    // replay, and the slot-reuse last-writer walk all consume this set, so
    // the three of them can never disagree about where a span ends.
    let span_closes: Vec<std::collections::BTreeSet<usize>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .filter_map(|(index, instruction)| {
                    (redefinitions[block_index].contains(&index)
                        || ((!instruction.clobbers.is_empty()
                            || !instruction.implicit_defs.is_empty())
                            && !crossed_unit_writes[block_index].contains(&index)))
                    .then_some(index)
                })
                .collect()
        })
        .collect();
    // Every recorded binding must resolve to exactly the snapshot chunk-load
    // stream its transport describes — `ReadPlace` accesses for its edge and
    // source place in the transport's chunk decomposition, each in this
    // bridge block reading the victim. The binding's `argument` field then
    // names one register holding the victim's edge-time value: the single
    // register all the loads share when they can — a one-chunk snapshot
    // always has one, and a multi-chunk one does while every chunk operand
    // is unpinned and the block's shared reload stays open across them — or
    // a dedicated end-of-block pair of the binding's own when they cannot.
    // A span-closing instruction at or after the first chunk load — a victim
    // redefinition, or a unit writer the policy did not cross — forces the
    // dedicated pair as well: it either splits the loads' registers or leaves
    // them holding a pre-close value the edge no longer reads.
    if !structural_uses.is_empty() {
        let mut chunk_loads: std::collections::BTreeMap<
            (EdgeId, PlaceId),
            Vec<&selected_instructions::SelectedMemoryAccess>,
        > = std::collections::BTreeMap::new();
        for access in &function.memory_accesses {
            if let SelectedMemoryAccessOrigin::Edge(edge) = access.origin
                && access.role == SelectedMemoryAccessRole::ReadPlace
            {
                chunk_loads
                    .entry((edge, access.place))
                    .or_default()
                    .push(access);
            }
        }
        for accesses in chunk_loads.values_mut() {
            accesses.sort_by_key(|access| access.byte_offset);
        }
        let mut owners = std::collections::BTreeMap::new();
        for (owner, block) in function.blocks.iter().enumerate() {
            for (index, instruction) in block.instructions.iter().enumerate() {
                owners.insert(instruction.id, (owner, index));
            }
        }
        for pending in &mut structural_uses {
            let Some(accesses) = chunk_loads.get(&(pending.edge, pending.place)) else {
                return Err(RuntimeSpillError::UnsupportedUse);
            };
            let block = &function.blocks[pending.block];
            let mut positions = Vec::with_capacity(accesses.len());
            let mut pinned = false;
            let mut offset = 0u32;
            for access in accesses {
                let Some(width) = next_chunk(pending.byte_size, offset) else {
                    return Err(RuntimeSpillError::UnsupportedUse);
                };
                if access.byte_offset != offset || access.byte_count != u32::from(width) {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                let Some(&(owner, index)) = owners.get(&access.instruction) else {
                    return Err(RuntimeSpillError::UnsupportedUse);
                };
                if owner != pending.block {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                let instruction = &block.instructions[index];
                let expected_kind = match width {
                    8 => SelectedInstructionKind::Load64 {
                        byte_offset: offset,
                    },
                    4 => SelectedInstructionKind::Load32 {
                        byte_offset: offset,
                    },
                    2 => SelectedInstructionKind::Load16 {
                        byte_offset: offset,
                    },
                    _ => SelectedInstructionKind::Load8 {
                        byte_offset: offset,
                    },
                };
                let Some(address) = instruction.operands.first() else {
                    return Err(RuntimeSpillError::UnsupportedUse);
                };
                if instruction.kind != expected_kind
                    || address.virtual_register != register
                    || address.access != RegisterOperandAccess::Use
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                pinned |= address.fixed_view.is_some();
                positions.push(index);
                pending.loads.push(access.instruction);
                offset += u32::from(width);
            }
            if offset != pending.byte_size {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            // The loads all name one register only while every chunk operand
            // is unpinned, the block keeps a shared reload, and no
            // span-closing instruction — a victim redefinition, or a unit
            // writer the policy did not cross — sits at or after the first
            // load's position: one strictly between loads splits the pair
            // they read, and one at or after the last load leaves the named
            // register's value or interval behind what the edge reads.
            // Positions follow byte order, not necessarily instruction order,
            // so the span runs from the earliest load's index. Anything else
            // gives the binding a dedicated end-of-block pair — the same
            // idiom a value binding uses — and the slot-reuse replay must see
            // that load.
            let first = *positions.iter().min().unwrap();
            pending.dedicated_reload = span_closes[pending.block].range(first..).next().is_some()
                || (pending.loads.len() > 1 && (!shared_reload[pending.block] || pinned));
            if pending.dedicated_reload {
                use_positions[pending.block].end_of_block = true;
            }
        }
    }
    // Prefer reusing an already-declared spill slot when the last-writer
    // replay proves the incumbent's and this victim's storage windows never
    // interleave. Reuse declares nothing, so the shared slot is charged to the
    // frame once; otherwise the victim gets its private slot as before.
    let (slot, fresh_slot) = match slot::shared_slot(
        function,
        register,
        &definitions,
        &use_positions,
        &shared_reload,
        &span_closes,
    ) {
        Some(shared) => (shared, false),
        None => (private_slot, true),
    };
    Ok(Reconstructed {
        function,
        use_blocks,
        shared_reload,
        dedicated_tie_uses,
        span_closes,
        victim,
        lineage,
        address_scalar_type: ScalarType::Integer(unsigned),
        definitions,
        slot,
        fresh_slot,
        structural_uses,
        first_instruction,
        first_register,
        address,
        load,
        store,
        carrier_class,
        bits_conversion,
    })
}

/// The declared bit-preserving pair bridging a foreign-class victim to the
/// slot's carrier class, checked on the validator's own audit. Only an
/// IEEE-float payload has such a pair today; each row must be the exact
/// two-operand `Use`/`Def` bridge between the two classes with no hidden
/// effects — a conversion that wrote or clobbered a unit could destroy the
/// still-open reload it sits inside. A missing key, an absent row, or a row
/// that does not bridge the two classes is a victim limit rather than an
/// environment fault.
fn bits_transport<'source>(
    environment: &'source ValidatedTargetRegisterEnvironment,
    victim: &VirtualRegister,
    carrier_class: RegisterClassId,
) -> Result<BitsConversion<'source>, RuntimeSpillError> {
    let keys = environment.selected_keys();
    let (to_key, from_key, to_kind, from_kind) = match victim.scalar_type {
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => (
            keys.float32_to_bits,
            keys.bits_to_float32,
            SelectedInstructionKind::Float32ToBits,
            SelectedInstructionKind::BitsToFloat32,
        ),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => (
            keys.float64_to_bits,
            keys.bits_to_float64,
            SelectedInstructionKind::Float64ToBits,
            SelectedInstructionKind::BitsToFloat64,
        ),
        _ => return Err(RuntimeSpillError::UnsupportedValue),
    };
    let to_bits = environment
        .constraint(to_key.ok_or(RuntimeSpillError::UnsupportedValue)?)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    let from_bits = environment
        .constraint(from_key.ok_or(RuntimeSpillError::UnsupportedValue)?)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    for (row, classes) in [
        (to_bits, [victim.class, carrier_class]),
        (from_bits, [carrier_class, victim.class]),
    ] {
        if row.operands.len() != 2 || !row.implicit_defs.is_empty() || !row.clobbers.is_empty() {
            return Err(RuntimeSpillError::UnsupportedValue);
        }
        for (ordinal, (operand, class)) in row.operands.iter().zip(classes).enumerate() {
            if usize::from(operand.operand) != ordinal
                || operand.access
                    != if ordinal == 0 {
                        RegisterOperandAccess::Use
                    } else {
                        RegisterOperandAccess::Def
                    }
                || operand.class != class
                || operand.fixed_view.is_some()
                || operand.tied_to.is_some()
                || operand.early_clobber
            {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
        }
    }
    Ok(BitsConversion {
        to_bits,
        from_bits,
        to_kind,
        from_kind,
    })
}

/// The exact edge definitions a block-parameter victim needs: on every
/// predecessor the edge's own store reads the predecessor's copy output — or,
/// on a case-dispatch continuation, the bridge's field observation behind the
/// payload's own `Load32`/`Load64` — checked here on the validator's own
/// audit rather than trusted from the producer's record.
fn parameter_definitions(
    function: &SelectedFunction,
    destination: usize,
    victim: &VirtualRegister,
    source_value: ValueId,
) -> Result<Vec<StorageDefinition>, RuntimeSpillError> {
    let mut definitions = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for successor in control(&block.terminator).1.into_iter().flatten() {
            if successor.block != function.blocks[destination].id {
                continue;
            }
            // Every arrival must initialize the slot on its exact edge. Do not
            // place a store on a conditional predecessor shared by other paths.
            if successor.role != SelectedSuccessorRole::EdgeTransferContinuation
                || !matches!(block.terminator, SelectedTerminator::Jump { .. })
                || !matches!(block.origin, SelectedBlockOrigin::EdgeTransfer { edge, target }
                    if edge == successor.psi_edge && target == successor.source_target)
            {
                return Err(RuntimeSpillError::UnsupportedControlFlow);
            }
            if let Some(case) = &successor.structural_case {
                // A case edge initializes a destination parameter through its
                // payload transport: the stored register is the bridge's own
                // field observation, stored right after its load — the same
                // edge-exact idiom the value bindings below keep. These are
                // the checks `incoming_argument` applies to that transport.
                let mut payloads = case
                    .payloads
                    .iter()
                    .filter(|payload| payload.semantic.parameter.value == source_value);
                if let Some(payload) = payloads.next() {
                    if payloads.next().is_some()
                        || successor
                            .bindings
                            .iter()
                            .any(|binding| binding.semantic.parameter == source_value)
                        || payload.semantic.parameter.scalar_type != victim.scalar_type
                        || Some(payload.semantic.parameter.definition_site)
                            != victim.definition_site
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    let SelectedCasePayloadTransport::Registers {
                        argument,
                        parameter,
                        ..
                    } = payload.transport
                    else {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    };
                    if parameter != victim.id {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    let value = function
                        .virtual_registers
                        .iter()
                        .find(|value| value.id == argument)
                        .ok_or(RuntimeSpillError::UnsupportedValue)?;
                    if value.scalar_type != victim.scalar_type
                        || value.class != victim.class
                        || value.definition_site.is_some()
                    {
                        return Err(RuntimeSpillError::UnsupportedValue);
                    }
                    let VirtualRegisterOrigin::StructuralObservation {
                        instruction,
                        place,
                        byte_offset,
                    } = value.origin
                    else {
                        return Err(RuntimeSpillError::UnsupportedValue);
                    };
                    if Some(place) != case.slot.structural_place()
                        || byte_offset != payload.semantic.field_byte_offset
                    {
                        return Err(RuntimeSpillError::UnsupportedValue);
                    }
                    // The field load must be the idiom the case bridge itself
                    // emits: a 32-bit payload through `Load32`, a 64-bit one
                    // through `Load64`, each at the declared field offset.
                    let load_kind = match payload.semantic.parameter.scalar_type {
                        ScalarType::Integer(integer) if integer.bits() == 32 => {
                            SelectedInstructionKind::Load32 { byte_offset }
                        }
                        ScalarType::Integer(integer) if integer.bits() == 64 => {
                            SelectedInstructionKind::Load64 { byte_offset }
                        }
                        _ => return Err(RuntimeSpillError::UnsupportedValue),
                    };
                    let (owner, load) = physical_definition(function, argument)?;
                    if owner != block_index
                        || load.id != instruction
                        || load.kind != load_kind
                        || !load.operands.iter().any(|operand| {
                            operand.virtual_register == argument
                                && operand.access == RegisterOperandAccess::Def
                        })
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    definitions.push(StorageDefinition {
                        block_index,
                        position: StoragePosition::AfterInstruction(instruction),
                        register: argument,
                    });
                    continue;
                }
                // No payload declares the parameter's value: the edge may still
                // initialize it through an ordinary value binding.
            }
            let mut bindings = successor.bindings.iter().filter(|binding| {
                matches!(binding.transport,
                SelectedValueTransport::Registers { parameter, .. } if parameter == victim.id)
            });
            let binding = bindings.next().ok_or(RuntimeSpillError::UnsupportedUse)?;
            if bindings.next().is_some()
                || binding.semantic.parameter != source_value
                || binding.semantic.scalar_type != victim.scalar_type
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            let SelectedValueTransport::Registers { argument, .. } = binding.transport else {
                return Err(RuntimeSpillError::UnsupportedUse);
            };
            let value = function
                .virtual_registers
                .iter()
                .find(|value| value.id == argument)
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            if value.scalar_type != victim.scalar_type || value.class != victim.class {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            let VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: argument_value,
            } = value.origin
            else {
                return Err(RuntimeSpillError::UnsupportedValue);
            };
            if argument_value != binding.semantic.argument {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            let (owner, copy) = physical_definition(function, argument)?;
            if owner != block_index
                || copy.id != instruction
                || copy.kind != SelectedInstructionKind::CopyI64
                || !copy.operands.iter().any(|operand| {
                    operand.virtual_register == argument
                        && operand.access == RegisterOperandAccess::Def
                })
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            definitions.push(StorageDefinition {
                block_index,
                position: StoragePosition::AfterInstruction(instruction),
                register: argument,
            });
        }
    }
    if definitions.is_empty() {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    Ok(definitions)
}

/// An entry-bound register's storage definition is the function-entry
/// boundary: the value arrives live-in, so its store opens the entry block.
/// Any edge back to that block — a loop header reuse or a shared
/// continuation — would re-execute the store reading a register whose
/// interval already ended at the first store, so re-entry stays rejected
/// rather than replayed against.
fn entry_definitions(
    function: &SelectedFunction,
    entry: usize,
    victim: &VirtualRegister,
) -> Result<Vec<StorageDefinition>, RuntimeSpillError> {
    let entry_block = function.blocks[entry].id;
    for block in &function.blocks {
        if control(&block.terminator)
            .1
            .into_iter()
            .flatten()
            .any(|successor| successor.block == entry_block)
        {
            return Err(RuntimeSpillError::UnsupportedControlFlow);
        }
    }
    Ok(vec![StorageDefinition {
        block_index: entry,
        position: StoragePosition::BlockStart,
        register: victim.id,
    }])
}

/// The unique physical instruction defining `argument` anywhere in the
/// function, as `(owner block, instruction)`. Missing definitions mean the
/// transport names no real value; multiple mean it is not a single edge-exact
/// definition — the same split the two caller sites keep.
fn physical_definition(
    function: &SelectedFunction,
    argument: VirtualRegisterId,
) -> Result<(usize, &SelectedInstruction), RuntimeSpillError> {
    let mut definitions = function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(owner, block)| {
            block.instructions.iter().flat_map(move |candidate| {
                candidate
                    .operands
                    .iter()
                    .filter(move |operand| {
                        operand.virtual_register == argument
                            && operand.access != RegisterOperandAccess::Use
                    })
                    .map(move |_| (owner, candidate))
            })
        });
    let Some(definition) = definitions.next() else {
        return Err(RuntimeSpillError::UnsupportedValue);
    };
    if definitions.next().is_some() {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    Ok(definition)
}

/// Independently consume the proposed instruction stream, checking every private
/// access and rewritten operand against its original use. Stripping these exact
/// additions must restore the entire admitted source, including calls and fuel.
/// Replays under the bounded span policy the default rewrite produces.
pub fn validate_runtime_spill(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    validate_runtime_spill_with_span_policy(
        source,
        function_index,
        register,
        environment,
        budget,
        proposed,
        RuntimeSpillSpanPolicy::UnitWriteBounded,
    )
}

/// The same independent consumption under an explicit span policy: replay
/// must follow the producer's own open/close decision exactly, so a plan
/// produced under the other policy — a shared reload reaching across a
/// unit-writing instruction, or a private pair where the crossing span
/// stayed open — mismatches rather than validates.
pub fn validate_runtime_spill_with_span_policy(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
    span_policy: RuntimeSpillSpanPolicy,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        register,
        environment,
        span_policy,
        budget,
    )?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(RuntimeSpillError::ReplayMismatch)?;
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    if restored_function.boundary_settlements.len()
        != reconstructed.function.boundary_settlements.len()
    {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    let mut values = function
        .virtual_registers
        .iter()
        .skip(reconstructed.function.virtual_registers.len());
    let mut next_instruction = reconstructed.first_instruction;
    let mut next_register = reconstructed.first_register;
    for (block_index, source_block) in reconstructed.function.blocks.iter().enumerate() {
        if !reconstructed.use_blocks.contains(&block_index)
            && !reconstructed
                .definitions
                .iter()
                .any(|definition| definition.block_index == block_index)
        {
            continue;
        }
        let block = function
            .blocks
            .get(block_index)
            .ok_or(RuntimeSpillError::ReplayMismatch)?;
        let mut stream = block.instructions.iter();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        let mut consumed = 0u32;
        // Replay reconstructs the same block-local decision independently:
        // when the audit proved a surviving view, the first unpinned
        // instruction-operand use emits the pair and each later unpinned use
        // names that still-open reload register until an instruction that can
        // destroy register content — and is not crossed under the audited
        // span policy — closes it. A pinned operand or an unadmitted block
        // consumes a fresh pair at every use.
        let shared = reconstructed.shared_reload[block_index];
        let mut open_reload: Option<VirtualRegisterId> = None;
        // The register each restored instruction's first victim use must
        // name — replay computes the same binding-argument target the
        // proposal had to produce.
        let mut use_reloads = std::collections::BTreeMap::new();
        // A boundary definition's store opens the block; replay consumes it
        // from the stream before the first source instruction, in the same
        // order the proposal emitted it.
        for definition in reconstructed.definitions.iter().filter(|definition| {
            definition.block_index == block_index
                && matches!(definition.position, StoragePosition::BlockStart)
        }) {
            let store = expected_store(
                &reconstructed,
                definition.register,
                &mut next_instruction,
                &mut next_register,
            )?;
            let (registers, sequence) = store.into_streams();
            for register in &registers {
                if values.next() != Some(register) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            for instruction in &sequence {
                if stream.next() != Some(instruction) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            consumed = consumed
                .checked_add(
                    u32::try_from(sequence.len())
                        .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                )
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
        }
        for (instruction_index, original) in source_block.instructions.iter().enumerate() {
            boundaries.push(consumed);
            let mut restored = original.clone();
            // `UseDef` operands read the victim like uses: replay redirects
            // them to the same reload registers, and the operand's own write
            // is what the instruction's following store then reads. A plain
            // `Def` — including one tied to a victim use — keeps the victim
            // register its store replays.
            for operand in &mut restored.operands {
                if operand.virtual_register != register
                    || !matches!(
                        operand.access,
                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                    )
                {
                    continue;
                }
                // A use the audit recorded beside an early-clobber tied write
                // must name a pair of its own — the write's tied home is a
                // register the unpinned co-readers never share.
                let dedicated = reconstructed.dedicated_tie_uses[block_index]
                    .contains(&(instruction_index, operand.operand));
                let share = shared && operand.fixed_view.is_none() && !dedicated;
                let reloaded = match (share, open_reload) {
                    (true, Some(existing)) => existing,
                    _ => {
                        let reload = expected_reload(
                            &reconstructed,
                            register,
                            &mut next_instruction,
                            &mut next_register,
                        )?;
                        let reloaded = reload.reload_register.id;
                        let (registers, sequence) = reload.into_streams();
                        for register in &registers {
                            if values.next() != Some(register) {
                                return Err(RuntimeSpillError::ReplayMismatch);
                            }
                        }
                        for instruction in &sequence {
                            if stream.next() != Some(instruction) {
                                return Err(RuntimeSpillError::ReplayMismatch);
                            }
                        }
                        consumed = consumed
                            .checked_add(
                                u32::try_from(sequence.len())
                                    .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                            )
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                        if share {
                            open_reload = Some(reloaded);
                        }
                        reloaded
                    }
                };
                use_reloads.entry(original.id).or_insert(reloaded);
                operand.virtual_register = reloaded;
            }
            instruction_positions.push(consumed);
            if stream.next() != Some(&restored) {
                return Err(RuntimeSpillError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(1)
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            // The same span-closing instruction — a victim redefinition, or
            // a unit writer the audited span policy did not cross — that
            // closed the proposal's open reload closes it here, so the next
            // unpinned use must name a fresh pair rather than the
            // pre-boundary register.
            if reconstructed.span_closes[block_index].contains(&instruction_index) {
                open_reload = None;
            }
            for definition in reconstructed.definitions.iter().filter(|definition| {
                definition.block_index == block_index
                    && match definition.position {
                        StoragePosition::AfterInstruction(anchor)
                        | StoragePosition::AfterUseDef {
                            instruction: anchor,
                            ..
                        } => anchor == original.id,
                        StoragePosition::BlockStart => false,
                    }
            }) {
                // The `UseDef` store reads the register its operand was
                // redirected to — replay resolves it on the rebuilt
                // instruction, exactly where the proposal's emitter read it.
                let stored = match definition.position {
                    StoragePosition::AfterUseDef { operand, .. } => restored
                        .operands
                        .iter()
                        .find(|candidate| candidate.operand == operand)
                        .map(|operand| operand.virtual_register)
                        .ok_or(RuntimeSpillError::ReplayMismatch)?,
                    _ => definition.register,
                };
                let store = expected_store(
                    &reconstructed,
                    stored,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                let (registers, sequence) = store.into_streams();
                for register in &registers {
                    if values.next() != Some(register) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                for instruction in &sequence {
                    if stream.next() != Some(instruction) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                consumed = consumed
                    .checked_add(
                        u32::try_from(sequence.len())
                            .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                    )
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
            }
        }
        // The rewrite appends terminator-operand reloads after the last body
        // instruction; replay consumes them in operand order, then requires
        // the proposed terminator to equal the source with exactly those
        // operands redirected to their reload registers.
        let mut expected_terminator = source_block.terminator.clone();
        for operand in &mut control_mut(&mut expected_terminator).operands {
            if operand.virtual_register != register || operand.access != RegisterOperandAccess::Use
            {
                continue;
            }
            let reload = expected_reload(
                &reconstructed,
                register,
                &mut next_instruction,
                &mut next_register,
            )?;
            let reloaded = reload.reload_register.id;
            let (registers, sequence) = reload.into_streams();
            for register in &registers {
                if values.next() != Some(register) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            for instruction in &sequence {
                if stream.next() != Some(instruction) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
            }
            consumed = consumed
                .checked_add(
                    u32::try_from(sequence.len())
                        .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                )
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            operand.virtual_register = reloaded;
        }
        // Binding-argument reloads follow the terminator-operand pairs in
        // successor, then binding, then case-payload order; the expected
        // terminator carries the moved argument on each matching transport
        // and nothing else.
        for successor in control_successors_mut(&mut expected_terminator)
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
                let reload = expected_reload(
                    &reconstructed,
                    register,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                let reloaded = reload.reload_register.id;
                let (registers, sequence) = reload.into_streams();
                for register in &registers {
                    if values.next() != Some(register) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                for instruction in &sequence {
                    if stream.next() != Some(instruction) {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                }
                consumed = consumed
                    .checked_add(
                        u32::try_from(sequence.len())
                            .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                    )
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
                let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport
                else {
                    unreachable!()
                };
                *argument = reloaded;
            }
            // A stored structural-transport argument whose chunk loads share
            // one register consumes no pair: the expected terminator only
            // retargets it to the register they were proven to name. Where
            // reconstruction recorded they cannot share it, the binding
            // instead consumes the dedicated end-of-block pair the rewrite
            // emitted — the same idiom a value binding's argument keeps.
            for binding in &mut successor.structural_bindings {
                let Some(byte_size) = stored_transport_size(binding.transport) else {
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
                let Some(pending) = reconstructed.structural_uses.iter().find(|pending| {
                    pending.block == block_index
                        && pending.edge == successor.psi_edge
                        && pending.place == binding.semantic.argument.place
                        && pending.byte_size == byte_size
                }) else {
                    return Err(RuntimeSpillError::ReplayMismatch);
                };
                let reloaded = if pending.dedicated_reload {
                    let reload = expected_reload(
                        &reconstructed,
                        register,
                        &mut next_instruction,
                        &mut next_register,
                    )?;
                    let reloaded = reload.reload_register.id;
                    let (registers, sequence) = reload.into_streams();
                    for register in &registers {
                        if values.next() != Some(register) {
                            return Err(RuntimeSpillError::ReplayMismatch);
                        }
                    }
                    for instruction in &sequence {
                        if stream.next() != Some(instruction) {
                            return Err(RuntimeSpillError::ReplayMismatch);
                        }
                    }
                    consumed = consumed
                        .checked_add(
                            u32::try_from(sequence.len())
                                .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                        )
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    reloaded
                } else {
                    pending
                        .loads
                        .iter()
                        .try_fold(None, |found: Option<VirtualRegisterId>, load| {
                            let named = use_reloads
                                .get(load)
                                .copied()
                                .ok_or(RuntimeSpillError::ReplayMismatch)?;
                            match found {
                                None => Ok(Some(named)),
                                Some(existing) if existing == named => Ok(found),
                                _ => Err(RuntimeSpillError::ReplayMismatch),
                            }
                        })?
                        .ok_or(RuntimeSpillError::ReplayMismatch)?
                };
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
                    let reload = expected_reload(
                        &reconstructed,
                        register,
                        &mut next_instruction,
                        &mut next_register,
                    )?;
                    let reloaded = reload.reload_register.id;
                    let (registers, sequence) = reload.into_streams();
                    for register in &registers {
                        if values.next() != Some(register) {
                            return Err(RuntimeSpillError::ReplayMismatch);
                        }
                    }
                    for instruction in &sequence {
                        if stream.next() != Some(instruction) {
                            return Err(RuntimeSpillError::ReplayMismatch);
                        }
                    }
                    consumed = consumed
                        .checked_add(
                            u32::try_from(sequence.len())
                                .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
                        )
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    let SelectedCasePayloadTransport::Registers { argument, .. } =
                        &mut payload.transport
                    else {
                        unreachable!()
                    };
                    *argument = reloaded;
                }
            }
        }
        if block.terminator != expected_terminator {
            return Err(RuntimeSpillError::ReplayMismatch);
        }
        restored_function.blocks[block_index]
            .terminator
            .clone_from(&source_block.terminator);
        boundaries.push(consumed);
        instruction_positions.push(consumed);
        if stream.next().is_some() {
            return Err(RuntimeSpillError::ReplayMismatch);
        }
        for (actual, original) in restored_function
            .boundary_settlements
            .iter_mut()
            .zip(&reconstructed.function.boundary_settlements)
        {
            if original.block != source_block.id {
                continue;
            }
            let positions = if matches!(
                original.settlement,
                SelectedBoundarySettlementPayload::ClaimCompletion(_)
            ) {
                &boundaries
            } else {
                &instruction_positions
            };
            let expected = positions
                .get(original.instruction_index as usize)
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            if actual.block != source_block.id || actual.instruction_index != *expected {
                return Err(RuntimeSpillError::ReplayMismatch);
            }
            actual.instruction_index = original.instruction_index;
        }
        restored_function.blocks[block_index]
            .instructions
            .clone_from(&source_block.instructions);
    }
    if values.next().is_some() {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    // A fresh private slot must be the appended tail entry; a shared slot
    // declared nothing, so the storage list must equal the source exactly.
    if reconstructed.fresh_slot {
        if restored_function.local_storage_slots.pop()
            != Some(SelectedLocalStorageSlot {
                id: reconstructed.slot,
                byte_size: 8,
                alignment: 8,
            })
        {
            return Err(RuntimeSpillError::ReplayMismatch);
        }
    } else if restored_function.local_storage_slots != reconstructed.function.local_storage_slots {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    restored_function
        .virtual_registers
        .truncate(reconstructed.function.virtual_registers.len());
    if restored != *source.selected_plan() {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    Ok(ValidatedRuntimeSpill {
        receipt: RuntimeSpillReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

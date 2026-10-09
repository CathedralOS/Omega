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
//! The shared `control`, `require_dominated_uses`, `slot::assign`,
//! `stored_transport_size`, `next_chunk` and `surviving_home_exists` helpers
//! stay shared: the audit reads the same contract, not the producer's record.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBoundarySettlementPayload,
    SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedLocalStorageSlot,
    SelectedStructuralTransport, SelectedValueTransport, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::ScalarType;

use super::{
    BitsConversion, RuntimeSpillError, RuntimeSpillReceipt, RuntimeSpillSpanPolicy,
    StorageDefinition, StoragePosition, StructuralArgumentUse, ValidatedRuntimeSpill,
    VictimLineage, control_mut, control_successors_mut, slot, stored_transport_size,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

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
    // Establishment retains its exact address operand; only later uses reload.
    retained_uses: std::collections::BTreeSet<(SelectedInstructionId, u16)>,
    victim: &'source VirtualRegister,
    /// What the spill restates, derived from the victim's origin: a scalar
    /// keeps its source `ValueId`; a structural register keeps its declared
    /// place and byte offset and names no source value.
    lineage: VictimLineage,
    address_scalar_type: ScalarType,
    definitions: Vec<StorageDefinition>,
    /// The slot every emitted store and reload pair names, resolved at
    /// storage-window granularity — the audit's own replay of the interval
    /// assignment, not the producer's record.
    assignment: slot::Assignment,
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
    /// The target's frame-address temporary, independent of the stored payload.
    address_scratch: Option<VirtualRegister>,
    /// `to_bits` exposing the stored register's payload in the carrier class.
    convert: Option<SelectedInstruction>,
    store: SelectedInstruction,
}

impl ExpectedStore {
    /// Expected registers in declaration order, then the emitted instructions
    /// in stream order — conversion ahead of the `Store64`.
    fn into_streams(self) -> (Vec<VirtualRegister>, Vec<SelectedInstruction>) {
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

fn fresh(next: &mut u32) -> Result<u32, RuntimeSpillError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(result)
}

/// The instruction the audit expects for one emitted row: the target row's
/// complete operand/effect interface, not guessed ISA conventions.

/// The expected address/load pair for one admitted use, built from the
/// validator's own reconstruction: the victim's exact type, class, lineage
/// and definition-site custody, the resolved slot, the frame rows' operand
/// interface, and the declared conversion pair when the victim cannot ride
/// the slot rows directly.
fn expected_reload(
    reconstructed: &Reconstructed<'_>,
    slot: LocalStorageSlotId,
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
        address: crate::rewrites::instruction(
            address_instruction,
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            reconstructed.address,
            &[address_register],
        ),
        load: crate::rewrites::instruction(
            load_instruction,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            reconstructed.load,
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

/// The expected store sequence for one definition, built from the validator's
/// own reconstruction: the `Store64` reading the proven stored register
/// directly, or the `to_bits` conversion exposing the payload in the carrier
/// class ahead of it.
fn expected_store(
    reconstructed: &Reconstructed<'_>,
    slot: LocalStorageSlotId,
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
    let address_scratch = reconstructed
        .store
        .operands
        .get(1)
        .map(|operand| {
            Ok::<_, RuntimeSpillError>(VirtualRegister {
                id: VirtualRegisterId(fresh(next_register)?),
                scalar_type: reconstructed.address_scalar_type,
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
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            reconstructed.store,
            &store_registers,
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
/// definition per predecessor, or the function-entry boundary whose store
/// every re-entry edge re-executes; every operand, terminator, transport and
/// case-payload use must be flexible, untied and non-early-clobber in the
/// victim's own class (a `Def` becomes a tracked redefinition, a `UseDef`
/// the read-modify-write store); the frame rows must expose the declared
/// operand interface with no hidden effects; the definition anchor must
/// dominate every use block; and the span, structural-snapshot and slot-reuse
/// decisions follow the recorded facts. The audit charges the family's
/// measured steps against the budget. Nothing in it reads the producer's
/// admission decision, so a producer-side legality error fails here even
/// when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    span_policy: RuntimeSpillSpanPolicy,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, RuntimeSpillError> {
    let derived = super::derive_runtime_spill(
        source,
        function_index,
        register,
        environment,
        span_policy,
        budget,
    )?;
    Ok(super::runtime_spill_projection!(Reconstructed, derived))
}

/// The declared bit-preserving pair bridging a foreign-class victim to the
/// slot's carrier class, checked on the validator's own audit. Only an
/// IEEE-float payload has such a pair today; each row must be the exact
/// two-operand `Use`/`Def` bridge between the two classes with no hidden
/// effects — a conversion that wrote or clobbered a unit could destroy the
/// still-open reload it sits inside. A missing key, an absent row, or a row
/// that does not bridge the two classes is a victim limit rather than an
/// environment fault.
/// The exact edge definitions a block-parameter victim needs: on every
/// predecessor the edge's own store reads the predecessor's copy output — or,
/// on a case-dispatch continuation, the bridge's field observation behind the
/// payload's own `Load32`/`Load64` — checked here on the validator's own

fn consume_emitted<'a>(
    values: &mut impl Iterator<Item = &'a VirtualRegister>,
    stream: &mut impl Iterator<Item = &'a SelectedInstruction>,
    emitted: (Vec<VirtualRegister>, Vec<SelectedInstruction>),
    consumed: &mut u32,
) -> Result<(), RuntimeSpillError> {
    let (registers, sequence) = emitted;
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
    *consumed = consumed
        .checked_add(
            u32::try_from(sequence.len()).map_err(|_| RuntimeSpillError::IdentityOverflow)?,
        )
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(())
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
        for (definition_index, definition) in
            reconstructed
                .definitions
                .iter()
                .enumerate()
                .filter(|(_, definition)| {
                    definition.block_index == block_index
                        && matches!(definition.position, StoragePosition::BlockStart)
                })
        {
            let store = expected_store(
                &reconstructed,
                reconstructed.assignment.store_slot(definition_index),
                definition.register,
                &mut next_instruction,
                &mut next_register,
            )?;
            consume_emitted(
                &mut values,
                &mut stream,
                store.into_streams(),
                &mut consumed,
            )?;
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
                    || reconstructed
                        .retained_uses
                        .contains(&(original.id, operand.operand))
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
                            reconstructed
                                .assignment
                                .load_slot(block_index, instruction_index),
                            register,
                            &mut next_instruction,
                            &mut next_register,
                        )?;
                        let reloaded = reload.reload_register.id;
                        consume_emitted(
                            &mut values,
                            &mut stream,
                            reload.into_streams(),
                            &mut consumed,
                        )?;
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
            for (definition_index, definition) in reconstructed
                .definitions
                .iter()
                .enumerate()
                .filter(|(_, definition)| {
                    definition.block_index == block_index
                        && match definition.position {
                            StoragePosition::AfterInstruction(anchor)
                            | StoragePosition::AfterUseDef {
                                instruction: anchor,
                                ..
                            } => anchor == original.id,
                            StoragePosition::BlockStart => false,
                        }
                })
            {
                // The `UseDef` store reads the register its operand was
                // redirected to — replay resolves it on the rebuilt
                // instruction, exactly where the proposal's emitter read it.
                let stored = match definition.position {
                    StoragePosition::AfterUseDef { operand, .. } => restored
                        .operand(operand)
                        .map(|operand| operand.virtual_register)
                        .ok_or(RuntimeSpillError::ReplayMismatch)?,
                    _ => definition.register,
                };
                let store = expected_store(
                    &reconstructed,
                    reconstructed.assignment.store_slot(definition_index),
                    stored,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                consume_emitted(
                    &mut values,
                    &mut stream,
                    store.into_streams(),
                    &mut consumed,
                )?;
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
                reconstructed.assignment.end_of_block_slot(block_index),
                register,
                &mut next_instruction,
                &mut next_register,
            )?;
            let reloaded = reload.reload_register.id;
            consume_emitted(
                &mut values,
                &mut stream,
                reload.into_streams(),
                &mut consumed,
            )?;
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
                    reconstructed.assignment.end_of_block_slot(block_index),
                    register,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                let reloaded = reload.reload_register.id;
                consume_emitted(
                    &mut values,
                    &mut stream,
                    reload.into_streams(),
                    &mut consumed,
                )?;
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
                let SelectedStructuralTransport::WholeValue {
                    argument,
                    source_byte_offset,
                    ..
                } = &mut binding.transport
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
                        && pending.source_byte_offset == *source_byte_offset
                        && pending.byte_size == byte_size
                }) else {
                    return Err(RuntimeSpillError::ReplayMismatch);
                };
                let reloaded = if pending.dedicated_reload {
                    let reload = expected_reload(
                        &reconstructed,
                        reconstructed.assignment.end_of_block_slot(block_index),
                        register,
                        &mut next_instruction,
                        &mut next_register,
                    )?;
                    let reloaded = reload.reload_register.id;
                    consume_emitted(
                        &mut values,
                        &mut stream,
                        reload.into_streams(),
                        &mut consumed,
                    )?;
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
                        reconstructed.assignment.end_of_block_slot(block_index),
                        register,
                        &mut next_instruction,
                        &mut next_register,
                    )?;
                    let reloaded = reload.reload_register.id;
                    consume_emitted(
                        &mut values,
                        &mut stream,
                        reload.into_streams(),
                        &mut consumed,
                    )?;
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
    // A private slot declared by at least one window must be the appended tail
    // entry; a fully shared victim declared nothing, so the storage list must
    // equal the source exactly.
    if reconstructed.assignment.declares_private {
        if restored_function.local_storage_slots.pop()
            != Some(SelectedLocalStorageSlot {
                id: reconstructed.assignment.private,
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
    seal_runtime_spill_with_span_policy(source, proposed)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_runtime_spill_with_span_policy(
    source: &impl ValidatedSelectedAnalysis,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    Ok(sealed!(
        ValidatedRuntimeSpill,
        RuntimeSpillReceipt,
        source,
        proposed
    ))
}

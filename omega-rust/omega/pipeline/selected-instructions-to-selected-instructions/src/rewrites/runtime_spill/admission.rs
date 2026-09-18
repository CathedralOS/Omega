use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{
    RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnitId,
};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockOrigin, SelectedCasePayloadTransport,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
    SelectedStructuralTransport, SelectedSuccessorRole, SelectedTerminator, SelectedValueTransport,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    EdgeId, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, ValueId,
};

use super::RuntimeSpillError;
use super::slot;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub use_blocks: Vec<usize>,
    /// Per block: one reload register serves consecutive flexible uses until
    /// an instruction that can destroy register content — a clobber or an
    /// implicit definition — closes it and the next use opens a fresh pair.
    /// `false` keeps each use on a private reload pair.
    pub shared_reload: Vec<bool>,
    /// Per block: the source instruction indices whose clobber or implicit
    /// definition does NOT close the still-open shared reload. Under
    /// `RuntimeSpillSpanPolicy::UnitWriteCrossing` a unit-writing instruction
    /// stays crossed while an allocatable view of the victim's class avoids
    /// every unit written inside the span so far — the surviving home the
    /// produced interval then demands of the allocator. Always empty under
    /// `UnitWriteBounded`, the historical shape.
    pub crossed_unit_writes: Vec<std::collections::BTreeSet<usize>>,
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
    /// The physical slot the rewrite addresses. When `fresh_slot` is false this
    /// is an already-declared `Spill` slot whose storage windows the last-writer
    /// replay proved disjoint from this victim's; no storage entry is appended.
    pub slot: LocalStorageSlotId,
    /// `true` when `slot` is the victim's own private slot and the rewrite must
    /// append its `local_storage_slots` declaration. `false` means the slot was
    /// already declared by an earlier victim — frame demand is unchanged and
    /// the byte must not be counted a second time.
    pub fresh_slot: bool,
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
}

/// Source definition coordinates, not proposed spill instructions. An incoming
/// parameter has one exact edge definition per predecessor: the predecessor's
/// own copy output, or a case bridge's field observation behind its load. An
/// entry-bound register's single definition is the function-entry boundary
/// itself.
pub(super) struct StorageDefinition {
    pub block_index: usize,
    pub position: StoragePosition,
    pub register: VirtualRegisterId,
}

/// The identity a spilled register's reloads restate. A scalar victim keeps
/// its source `ValueId`; a structural victim is identified by the declared
/// place and byte offset it restates and names no source value at all —
/// value bindings can transport only the former, and the reload's register
/// origin follows suit. A boundary live-in keeps offset zero: the structural
/// parameter or hidden result destination names the whole place. An
/// instruction-defined structural register keeps its own coordinate, so a
/// reload replacing it as a case-payload argument still satisfies the exact
/// place and field offset that transport requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VictimLineage {
    Scalar(ValueId),
    Structural { place: PlaceId, byte_offset: u32 },
}

/// Where the definition's store lands inside `block_index`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StoragePosition {
    /// Immediately after the instruction that physically defines the stored
    /// register — the victim's own result or an edge's copy/observation output.
    AfterInstruction(SelectedInstructionId),
    /// Ahead of every block instruction: an entry-bound register's boundary
    /// definition. Its register is live-in, so position zero is the earliest
    /// point the stored value exists and the only one every use follows.
    BlockStart,
}

/// One stored structural-transport argument — a `Descriptor` or `WholeValue`
/// snapshot source — naming the victim. The binding sits on an edge-transfer
/// continuation in `block`; its snapshot loads carry `ReadPlace` memory
/// accesses for `edge`/`place` in `byte_size`'s chunk decomposition, and
/// `loads` is their instruction identities in byte order.
pub(super) struct StructuralArgumentUse {
    pub block: usize,
    pub edge: EdgeId,
    pub place: PlaceId,
    pub byte_size: u32,
    pub loads: Vec<SelectedInstructionId>,
}

/// The stored byte extent a structural transport snapshots — a descriptor's
/// fixed sixteen bytes, or a whole value's declared size. `Unused` transports
/// snapshot nothing.
pub(super) fn stored_transport_size(transport: SelectedStructuralTransport) -> Option<u32> {
    match transport {
        SelectedStructuralTransport::Unused => None,
        SelectedStructuralTransport::Descriptor { .. } => Some(16),
        SelectedStructuralTransport::WholeValue { byte_size, .. } => Some(u32::from(byte_size)),
    }
}

/// The chunk decomposition edge-transfer selection emits for one snapshot:
/// the largest of 8, 4, 2, 1 bytes at each remaining offset. Admission walks
/// the sorted access stream against this split rather than inferring widths.
/// `offset` stays at or below `byte_size`, so the subtraction cannot
/// underflow; once it equals `byte_size` no chunk remains and `None` is
/// returned for any further access.
fn next_chunk(byte_size: u32, offset: u32) -> Option<u8> {
    [8u32, 4, 2, 1]
        .into_iter()
        .find(|width| *width <= byte_size - offset)
        .map(|width| width as u8)
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    span_policy: super::RuntimeSpillSpanPolicy,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RuntimeSpillError> {
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
    let definitions = if let Some(instruction) = definition {
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
        let (terminal, successors) = super::control(&block.terminator);
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
                        // of refusing the victim. An output tied to this use
                        // would extend the reload's value identity.
                        if instruction
                            .operands
                            .iter()
                            .any(|other| other.tied_to == Some(operand.operand))
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
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
        .and_then(|total| total.checked_add(uses.checked_mul(4)?))
        .and_then(|total| total.checked_add(definitions.len()))
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
        // The unit-write crossing simulation replays the open/close decision
        // once over the victim function's own instructions.
        .and_then(|total| {
            total.checked_add(
                if span_policy == super::RuntimeSpillSpanPolicy::UnitWriteCrossing {
                    function.blocks.iter().try_fold(0usize, |sum, block| {
                        sum.checked_add(block.instructions.len())?.checked_add(1)
                    })?
                } else {
                    0
                },
            )
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
    super::require_dominated_uses(function, block_index, &use_blocks)?;
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
            // A victim in another register class cannot ride these rows; that
            // is a victim limit, so recovery may try the next candidate.
            if operand.class != victim.class {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
        }
    }
    let first_instruction = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain([super::control(&block.terminator).0.id.0])
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
            .chain(std::iter::once(super::control(&block.terminator).0))
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
    // Under `UnitWriteCrossing` a unit-writing instruction instead keeps the
    // open pair while an allocatable view of the victim's class avoids every
    // unit written inside the span so far — the clobber-set-reduced, most
    // often callee-saved, home the produced interval then demands. The
    // simulation replays the rewrite's own open/close decision: the pair
    // opens at each unpinned use and, while open, a unit-writing instruction
    // either joins the crossed set — accumulating its writes into the span —
    // or ends the span. Where no view survives, the instruction still closes
    // the span and the produced shape degrades to the bounded one.
    let crossed_unit_writes: Vec<std::collections::BTreeSet<usize>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            let mut crossed = std::collections::BTreeSet::new();
            if span_policy == super::RuntimeSpillSpanPolicy::UnitWriteCrossing
                && shared_reload[block_index]
            {
                let mut open = false;
                let mut written = std::collections::BTreeSet::new();
                for (index, instruction) in block.instructions.iter().enumerate() {
                    if use_positions[block_index].unpinned.contains(&index) {
                        open = true;
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
    // Every recorded binding must resolve to exactly the snapshot chunk-load
    // stream its transport describes — `ReadPlace` accesses for its edge and
    // source place in the transport's chunk decomposition, each in this
    // bridge block reading the victim — and all of those loads must name one
    // register after rewriting so the binding's `argument` field can move to
    // it. A one-chunk snapshot always has such a register; a multi-chunk
    // snapshot needs the block's shared open reload, so it is admitted only
    // where every chunk operand is unpinned and no unit-writing instruction
    // closes the span between the first and the last chunk load.
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
            if pending.loads.len() > 1 {
                // Positions follow byte order, not necessarily instruction
                // order; the span between the outermost chunk loads is what a
                // unit-writing instruction would have to sit inside to close
                // the shared reload between them. Under the crossing policy a
                // barrier the span survives does not close it.
                let first = *positions.iter().min().unwrap();
                let last = *positions.iter().max().unwrap();
                let closed = block.instructions[first..last].iter().enumerate().any(
                    |(offset, instruction)| {
                        (!instruction.clobbers.is_empty() || !instruction.implicit_defs.is_empty())
                            && !crossed_unit_writes[pending.block].contains(&(first + offset))
                    },
                );
                if !shared_reload[pending.block] || closed || pinned {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
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
        &crossed_unit_writes,
    ) {
        Some(shared) => (shared, false),
        None => (private_slot, true),
    };
    Ok(Admission {
        function,
        use_blocks,
        shared_reload,
        crossed_unit_writes,
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
    })
}

fn parameter_definitions(
    function: &SelectedFunction,
    destination: usize,
    victim: &VirtualRegister,
    source_value: ValueId,
) -> Result<Vec<StorageDefinition>, RuntimeSpillError> {
    let mut definitions = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for successor in super::control(&block.terminator).1.into_iter().flatten() {
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
        if super::control(&block.terminator)
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

pub(super) fn fresh(next: &mut u32) -> Result<u32, RuntimeSpillError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(result)
}

/// The private address computation and load inserted for one admitted use, at
/// an instruction operand or at a terminator operand. Proposal and replay build
/// the identical pair from this one constructor; the consumer operand keeps its
/// own access, class, and any fixed ABI view while only the referenced register
/// changes.
pub(super) struct Reload {
    pub address_register: VirtualRegister,
    pub reload_register: VirtualRegister,
    pub address: SelectedInstruction,
    pub load: SelectedInstruction,
}

pub(super) fn reload(
    admitted: &Admission<'_>,
    register: VirtualRegisterId,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<Reload, RuntimeSpillError> {
    let address_instruction = SelectedInstructionId(fresh(next_instruction)?);
    let load_instruction = SelectedInstructionId(fresh(next_instruction)?);
    let address_register = VirtualRegisterId(fresh(next_register)?);
    let reload_register = VirtualRegisterId(fresh(next_register)?);
    Ok(Reload {
        address_register: VirtualRegister {
            id: address_register,
            scalar_type: admitted.address_scalar_type,
            class: admitted.victim.class,
            origin: VirtualRegisterOrigin::SpillAddress {
                instruction: address_instruction,
                register,
            },
            definition_site: None,
            entry_fixed_view: None,
        },
        reload_register: VirtualRegister {
            id: reload_register,
            scalar_type: admitted.victim.scalar_type,
            class: admitted.victim.class,
            origin: match admitted.lineage {
                VictimLineage::Scalar(source_value) => VirtualRegisterOrigin::InstructionResult {
                    instruction: load_instruction,
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
                        instruction: load_instruction,
                        place,
                        byte_offset,
                    }
                }
            },
            definition_site: admitted.victim.definition_site,
            entry_fixed_view: None,
        },
        address: instruction(
            address_instruction,
            SelectedInstructionKind::FrameAddress {
                slot: frame(admitted.slot),
                byte_offset: 0,
            },
            admitted.address,
            &[address_register],
        ),
        load: instruction(
            load_instruction,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            admitted.load,
            &[address_register, reload_register],
        ),
    })
}

/// Target rows supply the complete operand/effect interface, not guessed ISA conventions.
pub(super) fn instruction(
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

pub(super) fn frame(slot: LocalStorageSlotId) -> FrameStorageSlotId {
    FrameStorageSlotId::Local(slot)
}

/// Whether one reload register can keep a legal home across a flexible-use
/// span. Under the bounded policy an open reload never spans an instruction
/// that can destroy register content — a clobber or implicit definition
/// closes it — so the produced interval only covers instructions that write
/// no unit at all; under the crossing policy the caller instead collects the
/// units every crossed instruction writes into `written_units`, and the
/// surviving view must avoid those too — the callee-saved candidates a call
/// crossing leaves. What remains is function-wide either way: a unit
/// implicitly used anywhere can be live through every interior point (the
/// caller collects those into `implicit_use_units`, including the frame rows
/// the rewrite inserts), a unit precolored by a fixed-view operand or an
/// entry-bound register (`pinned_units`) is likewise unavailable, and a
/// reserved unit is never allocatable. Where no view survives, every use
/// keeps a private reload pair — the shape the rewrite always produced.
fn surviving_home_exists(
    environment: &ValidatedTargetRegisterEnvironment,
    class: RegisterClassId,
    implicit_use_units: &std::collections::BTreeSet<RegisterUnitId>,
    pinned_units: &std::collections::BTreeSet<RegisterUnitId>,
    written_units: &std::collections::BTreeSet<RegisterUnitId>,
) -> bool {
    let reserved = environment.reservations().reserved_units();
    let physical = environment.physical().model();
    physical
        .classes
        .iter()
        .find(|row| row.id == class)
        .is_some_and(|row| {
            row.views.iter().any(|view_id| {
                physical
                    .views
                    .get(usize::from(view_id.0))
                    .is_some_and(|view| {
                        view.id == *view_id
                            && view.allocatable
                            && view.units.iter().chain(&view.write_units).all(|unit| {
                                !implicit_use_units.contains(unit)
                                    && !pinned_units.contains(unit)
                                    && !written_units.contains(unit)
                                    && reserved.binary_search(unit).is_err()
                            })
                    })
            })
        })
}

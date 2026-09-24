//! Independent validation of store-to-load forwarding.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! forwarding's legality from the source records — the named instruction's
//! load kind against its single read row, the byte extent the row's place and
//! range spell (or the fixed byte a resolved sequence index lands it on), the
//! backward walk that must find one covering store of one register on every
//! path, the crossed-edge and walked-span audits that keep that register and
//! the load's result identical, and the target's own copy row — then rebuilds
//! the function the contract demands and requires the proposal to equal it.
//! Restoring the load and its read row must reproduce the complete source by
//! content. A producer admission error therefore fails validation even when
//! the proposal is exactly what that producer emitted.

use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedMemoryAccess, SelectedMemoryAccessRole, SelectedOperand, SelectedStructuralTransport,
    SelectedSuccessor, SelectedValueTransport, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{PlaceId, ValueId};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::StructuralPlaceDeclaration;

use super::{
    StoredLoadForwardingError, StoredLoadForwardingReceipt, ValidatedStoredLoadForwarding,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::unexecuted::place_storage::{
    constant_index, extent_intersects, extent_reached_by, local_slot_is_place_storage,
    structural_place_declarations,
};

/// The validator's own reconstruction of the forwarding the contract
/// permits: the admitted load's coordinates and roster position, the
/// source instruction restore reinserts, and the instruction the contract
/// demands in its place. It shares no state with the producer's `admission`
/// record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    load_index: usize,
    /// Index of the load's single read row in `memory_accesses` — replay
    /// requires the proposed roster to drop exactly this row.
    load_access: usize,
    /// The register move or exact-width `ZeroExtend` the contract demands
    /// in place of the load.
    forwarded: SelectedInstruction,
    /// The walked body spans the audit cleared — each `(block, start, end)`
    /// in `function.blocks` instruction ordinals — for the measured-step
    /// contract.
    between: Vec<(usize, usize, usize)>,
    /// The count of successor edges the walk crossed.
    crossed: usize,
    /// The measured work the join-merge lookups performed — each edge
    /// inspected per candidate parameter — matching the producer's own
    /// accounting.
    merge_work: usize,
}

/// The bytes the load reads within one place root, re-decoded by the
/// validator from the load's roster row alone: an exact range, or a dynamic
/// extent when the load is the indexed byte load — its single read byte sits
/// at `byte_offset + index` for the runtime `index` the `ReadByteSequence`
/// row carries, so every position it can touch lies at or after
/// `byte_offset` with no static upper bound. When that `index` itself
/// resolves to a clean materialized constant, the audit collapses the extent
/// to the one byte `byte_offset + index` before the walk: the read byte's
/// position is then fixed, and every interference and source check below
/// decides on it.
struct Read {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    /// The indexed byte load's runtime index, deciding the read byte's
    /// position: `byte_offset + index`. `None` for an exact load, and for
    /// an indexed load whose index resolved — its read byte is the
    /// collapsed `byte_offset` then.
    sequence_index: Option<ValueId>,
}

impl Read {
    /// Exact rows intersect when their half-open byte intervals share a
    /// byte. A dynamic read extent is unbounded upward from `byte_offset`,
    /// so the exact row still reaches the read byte once its own extent ends
    /// past that offset — ending at or below it is the only provable
    /// disjointness.
    fn overlaps(&self, access: &SelectedMemoryAccess) -> bool {
        extent_intersects(
            self.byte_offset,
            self.byte_count,
            self.sequence_index.is_some(),
            access,
        )
    }

    /// Whether `access` can reach the read extent — the shared dynamic-reach
    /// and resolved-landing decision in `place_storage`.
    fn reaches(&self, access: &SelectedMemoryAccess, function: &SelectedFunction) -> bool {
        extent_reached_by(
            self.byte_offset,
            self.byte_count,
            self.sequence_index.is_some(),
            access,
            function,
        )
    }
}

/// Whether one roster row can disturb the read's bytes — the validator's
/// own interference decision over the validated access roster. Write rows
/// must target the same place root to overlap, and a materialized local
/// address for the place's own storage always blocks. A dynamic-extent write
/// on the read's place reaches only upward from its fixed offset: it blocks
/// exactly while that offset starts below the read's end, and a
/// byte-sequence write whose resolved index lands outside the read's extent
/// walks past wherever its payload base sits. When the read itself is a
/// dynamic extent the directions reverse: an exact-range write still reaches
/// the runtime-placed byte once its own extent ends past the payload base,
/// while every dynamic-extent row on the place meets it. A `WriteLocal` row
/// names an exact range on a slot: when the slot is the place's own storage,
/// range intersection decides and an intersecting row still has to be the
/// exact writer; when the slot only stages bytes naming the place, its bytes
/// are not the place's at any offset, so the row never blocks. Local-slot
/// storage for a different place and outgoing-area storage never alias a
/// referent place.
fn disturbs(
    read: &Read,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
    function: &SelectedFunction,
) -> bool {
    use SelectedMemoryAccessRole::*;
    match access.role {
        WritePlace => access.place == read.place && read.overlaps(access),
        WriteByteSpan { .. } | WriteByteSequence { .. } | WriteIndexedPrimitive { .. } => {
            access.place == read.place && read.reaches(access, function)
        }
        WriteLocal { slot } => {
            local_slot_is_place_storage(slot, read.place, structural_places)
                && read.overlaps(access)
        }
        // A materialized local address could reach the place's storage by a
        // route the roster does not bound, so it blocks whenever its slot is
        // the place's storage; a staged slot's address reaches only the
        // staged bytes.
        AddressLocal { slot } => local_slot_is_place_storage(slot, read.place, structural_places),
        ReadPlace
        | ReadByteSpan { .. }
        | ReadByteSequence { .. }
        | ReadElementView { .. }
        | ReadIndexedPrimitive { .. }
        | WriteOutgoing { .. }
        | AddressOutgoing { .. } => false,
    }
}

/// The register the two-use store shape carries at operand 1 — the value a
/// covering write stored, on the target's own `[use pointer, use value]`
/// store row.
fn stored_value(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
    {
        return Err(StoredLoadForwardingError::ConstraintMismatch);
    }
    let mut uses = instruction
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Use);
    let pointer = uses
        .next()
        .ok_or(StoredLoadForwardingError::AliasingWrite)?;
    let value = uses
        .next()
        .ok_or(StoredLoadForwardingError::AliasingWrite)?;
    if uses.next().is_some() || pointer.operand != 0 || value.operand != 1 {
        return Err(StoredLoadForwardingError::AliasingWrite);
    }
    Ok(value.virtual_register)
}

/// The interfering instruction must produce the read's bytes from one
/// register — the validator's own re-decision of the covering-write routes:
/// one roster row on the read's place naming the identical byte range — the
/// exact range for a fixed-offset read, the one fixed read byte for a
/// resolved indexed load — on the target's own operand surface. The routes
/// to the place's storage that qualify:
/// - a `Store` of the read's exact width carrying `WritePlace` — through the
///   referent pointer — or `WriteLocal` on the place's own parameter
///   storage, through that slot's materialized address;
/// - a `Store64` into `Local(slot)` carrying `WriteLocal` on that same
///   slot — directly into the place's own parameter storage. The slot store
///   is always eight bytes, so only `Load64` pairs with it;
/// - a `Store { 0, 1 }` through a fully computed view address carrying
///   `WriteByteSequence` — the byte-exact writer for a one-byte read. Its
///   row's `byte_offset` is the payload base the `index` extends, not the
///   written position, so the route decides on the byte it lands.
///
/// A wider, narrower, or shifted writer cannot produce the read's bytes
/// from one register without an extract the selected vocabulary does not
/// carry.
fn covering_register(
    instruction: &SelectedInstruction,
    read: &Read,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    // The writer's semantic identity: exactly one roster row on the read's
    // place naming the read's byte range — the exact range for a
    // fixed-offset read, the one fixed read byte for a resolved indexed
    // load. A `WriteByteSequence` row names a payload base instead, so its
    // route dispatches on the byte the write lands before the range check.
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(StoredLoadForwardingError::AliasingWrite);
    };
    if rows.next().is_some() || row.place != read.place {
        return Err(StoredLoadForwardingError::AliasingWrite);
    }
    if let SelectedMemoryAccessRole::WriteByteSequence { index, .. } = row.role {
        return sequence_register(instruction, read, row, index, function, environment);
    }
    if row.byte_offset != read.byte_offset || row.byte_count != read.byte_count {
        return Err(StoredLoadForwardingError::AliasingWrite);
    }
    match instruction.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            let route = match (read.sequence_index, row.role) {
                (None, SelectedMemoryAccessRole::WritePlace) => {
                    byte_offset == read.byte_offset && u32::from(byte_size) == read.byte_count
                }
                (None, SelectedMemoryAccessRole::WriteLocal { slot }) => {
                    byte_offset == read.byte_offset
                        && u32::from(byte_size) == read.byte_count
                        && local_slot_is_place_storage(
                            slot,
                            read.place,
                            structural_place_declarations(function),
                        )
                }
                _ => false,
            };
            if !route {
                return Err(StoredLoadForwardingError::AliasingWrite);
            }
            stored_value(instruction, environment)
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => {
            if byte_offset != read.byte_offset || read.byte_count != 8 {
                return Err(StoredLoadForwardingError::AliasingWrite);
            }
            // The roster row must name the same slot the instruction
            // writes, and that slot must be the read place's own storage.
            if row.role != (SelectedMemoryAccessRole::WriteLocal { slot })
                || !local_slot_is_place_storage(
                    slot,
                    read.place,
                    structural_place_declarations(function),
                )
            {
                return Err(StoredLoadForwardingError::AliasingWrite);
            }
            if environment.selected_keys().store64 != Some(instruction.constraint) {
                return Err(StoredLoadForwardingError::ConstraintMismatch);
            }
            let row = environment
                .constraint(instruction.constraint)
                .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
            if row.operands.len() != 1
                || row.operands[0].operand != 0
                || row.operands[0].access != RegisterOperandAccess::Use
            {
                return Err(StoredLoadForwardingError::ConstraintMismatch);
            }
            let mut uses = instruction
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use);
            let value = uses
                .next()
                .ok_or(StoredLoadForwardingError::AliasingWrite)?;
            if uses.next().is_some() || value.operand != 0 {
                return Err(StoredLoadForwardingError::AliasingWrite);
            }
            Ok(value.virtual_register)
        }
        _ => Err(StoredLoadForwardingError::AliasingWrite),
    }
}

/// The `WriteByteSequence` source route, re-decided by the validator: the
/// row claims the one byte at `byte_offset + index`, so the write sources
/// the read only when it lands on the read's byte. The encoded shape is the
/// one-byte store through a fully computed view address on the plain
/// `[use pointer, use value]` row, and the sequence row's contractual
/// one-byte count. The landing decision:
/// - a read byte whose own `index` resolved — the extent collapsed to a
///   fixed position, and a fixed-offset `Load8` reads the same fixed way —
///   is sourced exactly when the write's resolved `byte_offset + index`
///   names that one byte: a resolved index landing anywhere else writes a
///   byte the read never observes, and a runtime index lands the write
///   anywhere at or past the payload base, so it can never provably spell
///   the one fixed byte;
/// - a read byte still placed at runtime is sourced by equal payload base
///   and equal `index` value — the same position at runtime — or by
///   distinct index values when both resolve to constants whose
///   `byte_offset + index` sums agree. Anything else may land on a
///   different byte entirely.
fn sequence_register(
    instruction: &SelectedInstruction,
    read: &Read,
    row: &SelectedMemoryAccess,
    written: ValueId,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    if !matches!(
        instruction.kind,
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 1
        }
    ) || row.byte_count != 1
    {
        return Err(StoredLoadForwardingError::AliasingWrite);
    }
    let lands_on_read = match read.sequence_index {
        None => {
            read.byte_count == 1
                && constant_index(function, written)
                    .and_then(|landed| u64::from(row.byte_offset).checked_add(landed))
                    == Some(u64::from(read.byte_offset))
        }
        Some(index) if written == index => row.byte_offset == read.byte_offset,
        Some(index) => match (
            constant_index(function, written),
            constant_index(function, index),
        ) {
            (Some(written), Some(read_index)) => {
                u64::from(row.byte_offset).checked_add(written)
                    == u64::from(read.byte_offset).checked_add(read_index)
            }
            _ => false,
        },
    };
    if !lands_on_read {
        return Err(StoredLoadForwardingError::AliasingWrite);
    }
    stored_value(instruction, environment)
}

/// Calls, hosted effects, and terminator kinds are always barriers: they
/// can write or expose reachable storage regardless of their roster rows,
/// and a terminator kind never belongs in a block body.
fn barrier(instruction: &SelectedInstruction) -> Result<(), StoredLoadForwardingError> {
    use SelectedInstructionKind::*;
    match instruction.kind {
        CallUnit { .. }
        | CallScalar { .. }
        | CallAggregate { .. }
        | HostedReadByte { .. }
        | HostedWriteByteI32 { .. }
        | HostedExitProcessI32
        | ReturnScalar
        | ReturnAggregate { .. }
        | ReturnUnit
        | Jump
        | ConditionalBranchNonZero
        | ConditionalBranchU64LessThan
        | ConditionalBranchI64LessThan => Err(StoredLoadForwardingError::UnsupportedInstruction),
        _ => Ok(()),
    }
}

/// A walked instruction without a roster row must be unable to write any
/// semantic or place-backed storage: loads only observe, address forms only
/// compute, private-slot frame stores touch compiler-owned spill/boundary
/// slots, and pure register work has no memory side at all. A referent
/// store or any other frame slot without its row is an unaccounted write.
fn unaccounted(instruction: &SelectedInstruction) -> Result<(), StoredLoadForwardingError> {
    use SelectedInstructionKind::*;
    match instruction.kind {
        Store64 {
            slot:
                FrameStorageSlotId::Local(
                    LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
                ),
            ..
        }
        | Load8 { .. }
        | Load16 { .. }
        | Load32 { .. }
        | Load64 { .. }
        | Load8Indexed
        | LoadPacked { .. }
        | FrameAddress { .. }
        | AddressOffset { .. }
        | ByteViewAddress
        | CopyI64
        | MaterializeI64 { .. }
        | CompareI64
        | CompareI64Zero
        | CompareI64Immediate { .. }
        | ExactAddI64 { .. }
        | ExactSubtractI64 { .. }
        | ExactMultiplyI64 { .. }
        | ExactAddI64Immediate { .. }
        | ExactSubtractI64Immediate { .. }
        | WrappingAddI64
        | WrappingSubtractI64
        | WrappingMultiplyI64
        | ExactDivideU64 { .. }
        | ExactRemainderU64 { .. }
        | WrappingRemainderI64 { .. }
        | WrappingDivideI64 { .. }
        | ExactDivideI64 { .. }
        | ExactRemainderI64 { .. }
        | SaturatingAdd { .. }
        | SaturatingSubtract { .. }
        | SaturatingDivide { .. }
        | SaturatingRemainder { .. }
        | BitwiseAndI64
        | BitwiseOrI64
        | BitwiseNotI64
        | BitwiseXorI64
        | ZeroExtendU8
        | ZeroExtendU16
        | ZeroExtendU32
        | SignExtendI8
        | SignExtendI16
        | SignExtendI32
        | Float32ToBits
        | Float64ToBits
        | BitsToFloat32
        | BitsToFloat64
        | MaterializeBooleanEqual
        | MaterializeBooleanU64LessThan
        | MaterializeBooleanI64LessThan
        | MaterializeBooleanU64LessOrEqual
        | MaterializeBooleanI64LessOrEqual => Ok(()),
        Store { .. } | StorePacked { .. } | Store64 { .. } | CopyBytes => {
            Err(StoredLoadForwardingError::AliasingWrite)
        }
        _ => Err(StoredLoadForwardingError::UnsupportedInstruction),
    }
}

/// The load's sole register definition, re-derived by the validator. A
/// pinned, tied, or early-clobbered result cannot become a same-identity
/// copy through a clean copy row.
fn sole_result(
    instruction: &SelectedInstruction,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    let mut defs = instruction
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Def);
    let result = defs
        .next()
        .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
    if defs.next().is_some()
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
    {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    }
    Ok(result.virtual_register)
}

/// A crossed edge must perform no work the roster cannot see — the
/// validator's own edge audit. `Unused` transports are quiet; `Registers`
/// bindings and case payloads name the one register the edge defines, which
/// must not be the register the edge's own leg carries or the load's
/// result. The one permitted definition is the merged parameter the target
/// block resolved through — and only when the binding supplies exactly the
/// register this leg resolved to, which is the condition the merge itself
/// was reconstructed under. Structural destinations, case custody slots,
/// and custody discards reach the read's bytes only through the same place
/// root under place exclusivity.
fn edge_keeps(
    successor: &SelectedSuccessor,
    read: &Read,
    carried: Option<VirtualRegisterId>,
    defined: Option<VirtualRegisterId>,
    output: VirtualRegisterId,
) -> Result<(), StoredLoadForwardingError> {
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers {
            argument,
            parameter,
        } = binding.transport
        {
            if Some(parameter) == defined {
                if Some(argument) != carried {
                    return Err(StoredLoadForwardingError::UnsupportedUse);
                }
                continue;
            }
            if Some(parameter) == carried || parameter == output {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            }
        }
    }
    for binding in &successor.structural_bindings {
        // A lent address writes only the join's own carrier slot; the shared
        // referent itself is never written through it.
        let destination = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue { destination, .. }
            | SelectedStructuralTransport::Descriptor { destination, .. }
            | SelectedStructuralTransport::Address { destination, .. } => destination,
        };
        if destination.structural_place() == Some(read.place) {
            return Err(StoredLoadForwardingError::AliasingWrite);
        }
    }
    if let Some(case) = &successor.structural_case {
        if case.slot.structural_place() == Some(read.place)
            || case.trivial_affine_discards.contains(&read.place)
        {
            return Err(StoredLoadForwardingError::AliasingWrite);
        }
        for payload in &case.payloads {
            let parameter = match payload.transport {
                SelectedCasePayloadTransport::Unused => continue,
                SelectedCasePayloadTransport::Unmaterialized { parameter }
                | SelectedCasePayloadTransport::Registers { parameter, .. } => parameter,
            };
            if Some(parameter) == carried || Some(parameter) == defined || parameter == output {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            }
        }
    }
    Ok(())
}

/// Between the store and the load no instruction may redefine the register
/// that position carries or predefine the load's result; both stay
/// register-identical after the copy. A position whose block never
/// resolved carries nothing provable, so only the result still holds it.
fn span_keeps(
    instruction: &SelectedInstruction,
    carried: Option<VirtualRegisterId>,
    output: VirtualRegisterId,
) -> Result<(), StoredLoadForwardingError> {
    for operand in &instruction.operands {
        if operand.access != RegisterOperandAccess::Use
            && (Some(operand.virtual_register) == carried || operand.virtual_register == output)
        {
            return Err(StoredLoadForwardingError::UnsupportedUse);
        }
    }
    Ok(())
}

/// A divergent join still forwards when the block already merges the legs
/// on its edges, re-derived here from the source records alone: a
/// `BlockParameter` register every incoming edge binds from exactly that
/// leg's resolved register. The carried value is then the parameter
/// itself — it holds the stored bytes whichever path ran. Candidates are
/// the block's own parameter registers in declaration order; the first
/// every edge binds from its leg wins, and `work` charges one step per
/// edge inspected per candidate so the accounting matches the producer's.
fn merged_parameter(
    block: usize,
    predecessors: &[usize],
    resolved: &[Option<VirtualRegisterId>],
    function: &SelectedFunction,
    work: &mut usize,
) -> Option<VirtualRegisterId> {
    let block_id = function.blocks[block].id;
    for register in &function.virtual_registers {
        let VirtualRegisterOrigin::BlockParameter {
            block: parameter_block,
            ..
        } = register.origin
        else {
            continue;
        };
        if parameter_block != block_id {
            continue;
        }
        let mut binds = true;
        for predecessor in predecessors {
            for edge in terminator_successors(&function.blocks[*predecessor].terminator) {
                if edge.block != block_id {
                    continue;
                }
                *work += 1;
                let mut arguments =
                    edge.bindings
                        .iter()
                        .filter_map(|binding| match binding.transport {
                            SelectedValueTransport::Registers {
                                argument,
                                parameter,
                            } if parameter == register.id => Some(argument),
                            _ => None,
                        });
                let Some(argument) = arguments.next() else {
                    binds = false;
                    break;
                };
                if arguments.next().is_some() || Some(argument) != resolved[*predecessor] {
                    binds = false;
                    break;
                }
            }
            if !binds {
                break;
            }
        }
        if binds {
            return Some(register.id);
        }
    }
    None
}

/// Re-derive the forwarding's legality from the source records, without the
/// producer's `admission` routine. A legality error surfaces here even when
/// the proposal matches the edit the producer emitted.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    load: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, StoredLoadForwardingError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(StoredLoadForwardingError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(StoredLoadForwardingError::SourceMismatch)?;
    let (block_index, load_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == load)
                .map(|load_index| (block_index, load_index))
        })
        .ok_or(StoredLoadForwardingError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let forwarded_load = &block.instructions[load_index];
    let indexed = matches!(forwarded_load.kind, SelectedInstructionKind::Load8Indexed);
    let (encoded_offset, width): (u32, u32) = match forwarded_load.kind {
        SelectedInstructionKind::Load8 { byte_offset } => (byte_offset, 1),
        SelectedInstructionKind::Load16 { byte_offset } => (byte_offset, 2),
        SelectedInstructionKind::Load32 { byte_offset } => (byte_offset, 4),
        SelectedInstructionKind::Load64 { byte_offset } => (byte_offset, 8),
        // The indexed byte load encodes no offset of its own: the byte it
        // reads sits at the roster row's payload base plus the runtime
        // index the row carries.
        SelectedInstructionKind::Load8Indexed => (0, 1),
        _ => return Err(StoredLoadForwardingError::UnsupportedInstruction),
    };
    // The read's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction reads — the encoded range on a
    // `ReadPlace` row for a fixed-offset load, or the `ReadByteSequence`
    // row's one-byte dynamic extent for the indexed byte load.
    let mut rows = function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| access.instruction == load);
    let (load_access, read_row) = rows
        .next()
        .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
    if rows.next().is_some() {
        return Err(StoredLoadForwardingError::UnsupportedPair);
    }
    let sequence_index = match read_row.role {
        SelectedMemoryAccessRole::ReadPlace
            if !indexed
                && read_row.byte_offset == encoded_offset
                && read_row.byte_count == width =>
        {
            None
        }
        SelectedMemoryAccessRole::ReadByteSequence { index, .. }
            if indexed && read_row.byte_count == width =>
        {
            Some(index)
        }
        _ => return Err(StoredLoadForwardingError::UnsupportedPair),
    };
    let mut read = Read {
        place: read_row.place,
        byte_offset: read_row.byte_offset,
        byte_count: width,
        sequence_index,
    };
    // An indexed byte load whose own `index` resolves to a clean
    // materialized constant reads one fixed byte at `byte_offset + index`:
    // the read extent collapses to that exact byte, and every interference
    // and source check below decides on a fixed position.
    if let Some(index) = read.sequence_index
        && let Some(landed) = constant_index(function, index)
        && let Some(position) = u64::from(read.byte_offset).checked_add(landed)
        && let Ok(position) = u32::try_from(position)
    {
        read.byte_offset = position;
        read.byte_count = 1;
        read.sequence_index = None;
    }
    // The load's result must be defined only here; the copy keeps the
    // register.
    let output = sole_result(forwarded_load)?;
    let output_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == output)
        .ok_or(StoredLoadForwardingError::UnsupportedPair)?;
    let VirtualRegisterOrigin::InstructionResult {
        instruction: output_instruction,
        ..
    } = output_register.origin
    else {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    };
    if output_instruction != load {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    }
    // The load row must follow the target's load surface so the replacement
    // copy can reuse the result operand's class: [use pointer, def result]
    // for a fixed-offset load, [use pointer, use index, def result] for the
    // indexed byte load.
    let load_row = environment
        .constraint(forwarded_load.constraint)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    let load_shape = if indexed {
        load_row.operands.len() == 3
            && load_row.operands[0].operand == 0
            && load_row.operands[0].access == RegisterOperandAccess::Use
            && load_row.operands[1].operand == 1
            && load_row.operands[1].access == RegisterOperandAccess::Use
            && load_row.operands[2].operand == 2
            && load_row.operands[2].access == RegisterOperandAccess::Def
            && load_row.operands[2].class == output_register.class
    } else {
        load_row.operands.len() == 2
            && load_row.operands[0].operand == 0
            && load_row.operands[0].access == RegisterOperandAccess::Use
            && load_row.operands[1].operand == 1
            && load_row.operands[1].access == RegisterOperandAccess::Def
            && load_row.operands[1].class == output_register.class
    };
    if !load_shape {
        return Err(StoredLoadForwardingError::ConstraintMismatch);
    }
    // The validator's own backward walk: scan each reached block to the
    // first instruction that can disturb the read's bytes — a covering
    // store resolves the block to the register the write carried, anything
    // else rejects — and a block whose walked span stays clear defers to
    // every predecessor edge. A deferred block resolves once each
    // predecessor path settles on one register: the same store dominating
    // the join or each leg's own last writer of that register, and legs
    // that stored different registers still resolve when the block's own
    // parameter merges them — every incoming edge binding that parameter
    // from exactly the leg's register. A
    // deferred cycle resolves the same way once its arriving legs agree.
    // The entry block's implicit path, a block no edge reaches, and a
    // deferred region whose arriving legs disagree without an edge merge
    // or never settle each leave the load unproven.
    let structural_places = structural_place_declarations(function);
    let mut visited = vec![false; function.blocks.len()];
    let mut resolved = vec![None; function.blocks.len()];
    let mut deferred = Vec::new();
    let mut walked = Vec::new();
    let mut between = Vec::new();
    let mut crossed = Vec::new();
    let mut pending = vec![(block_index, load_index)];
    while let Some((cursor, cursor_end)) = pending.pop() {
        if visited[cursor] {
            continue;
        }
        visited[cursor] = true;
        walked.push(cursor);
        let current = &function.blocks[cursor];
        let mut found = None;
        for candidate_index in (0..cursor_end).rev() {
            let candidate = &current.instructions[candidate_index];
            barrier(candidate)?;
            let mut has_row = false;
            let mut interfered = false;
            for access in function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == candidate.id)
            {
                has_row = true;
                interfered |= disturbs(&read, access, structural_places, function);
            }
            if interfered {
                found = Some(covering_register(candidate, &read, function, environment)?);
                between.push((cursor, candidate_index + 1, cursor_end));
                break;
            }
            if !has_row {
                unaccounted(candidate)?;
            }
        }
        if let Some(value) = found {
            resolved[cursor] = Some(value);
            continue;
        }
        between.push((cursor, 0, cursor_end));
        // Reached the block's top. The writer must arrive on every path
        // in: the entry block has an implicit path no predecessor covers,
        // and a block no edge names leaves this path without a writer at
        // all.
        if current.id == function.entry_block {
            return Err(StoredLoadForwardingError::UnsupportedPair);
        }
        let mut predecessors = Vec::new();
        for (predecessor_index, predecessor) in function.blocks.iter().enumerate() {
            let edges: Vec<&SelectedSuccessor> = terminator_successors(&predecessor.terminator)
                .into_iter()
                .filter(|successor| successor.block == current.id)
                .collect();
            if edges.is_empty() {
                continue;
            }
            // The terminator instruction sits between the predecessor's
            // body and the crossed edge, so an interfering row on it
            // decides first. It never has a place-storage writer kind, so
            // it rejects the pair.
            let terminator = terminator_instruction(&predecessor.terminator);
            if function.memory_accesses.iter().any(|access| {
                access.instruction == terminator.id
                    && disturbs(&read, access, structural_places, function)
            }) {
                return Err(StoredLoadForwardingError::AliasingWrite);
            }
            crossed.extend(edges.iter().map(|edge| (predecessor_index, cursor, *edge)));
            predecessors.push(predecessor_index);
            if !visited[predecessor_index] {
                pending.push((predecessor_index, predecessor.instructions.len()));
            }
        }
        if predecessors.is_empty() {
            return Err(StoredLoadForwardingError::UnsupportedPair);
        }
        deferred.push((cursor, predecessors));
    }
    // Resolve the deferred region. A block resolves to the one register
    // every predecessor resolved to, so convergence through a shared
    // predecessor lands. Legs that settled on different registers still
    // meet when the block's own parameter already merges them — every
    // incoming edge binding that parameter from exactly this leg's
    // register — while divergent legs without the merge and unreached
    // regions leave the load's block unresolved. The first pass is acyclic
    // propagation only; a stall leaves the cyclic remainder to the
    // fixpoint below.
    let mut merged = vec![None; function.blocks.len()];
    let mut merge_work = 0usize;
    loop {
        if resolved[block_index].is_some() {
            break;
        }
        let mut progressed = false;
        for (block, predecessors) in &deferred {
            if resolved[*block].is_some() {
                continue;
            }
            let mut values = predecessors
                .iter()
                .map(|predecessor| resolved[*predecessor]);
            if let Some(Some(first)) = values.next()
                && values.all(|value| value == Some(first))
            {
                resolved[*block] = Some(first);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    // The still-open deferred blocks carry an optimistic candidate:
    // nothing in a deferred block's walked span writes the read's bytes, so
    // a cyclic region carries whatever its arriving legs agree on. Once
    // every predecessor has settled the block is decided: agreeing legs
    // propagate their register, divergent legs still meet through the
    // block's own merged parameter, and anything else is a conflict. While
    // a leg stays open the block takes the optimistic meet instead — a
    // resolved or already pinned predecessor contributes its register, an
    // open one contributes nothing, and conflicting settled registers mark
    // the block conflicted — and the candidates only descend, so the meet
    // converges. A block left open at the fixpoint belongs to a region no
    // resolved leg reaches: no finite path arrives through it, so it
    // constrains nothing.
    let mut open = vec![false; function.blocks.len()];
    for (block, _) in &deferred {
        open[*block] = resolved[*block].is_none();
    }
    let mut conflicted = vec![false; function.blocks.len()];
    loop {
        let mut progressed = false;
        for (block, predecessors) in &deferred {
            if !open[*block] || conflicted[*block] {
                continue;
            }
            if predecessors
                .iter()
                .all(|predecessor| resolved[*predecessor].is_some())
            {
                let mut legs = predecessors
                    .iter()
                    .map(|predecessor| resolved[*predecessor].unwrap());
                let first = legs.next().unwrap();
                if legs.all(|leg| leg == first) {
                    if resolved[*block] != Some(first) {
                        resolved[*block] = Some(first);
                        progressed = true;
                    }
                    merged[*block] = None;
                } else if let Some(parameter) =
                    merged_parameter(*block, predecessors, &resolved, function, &mut merge_work)
                {
                    if resolved[*block] != Some(parameter) {
                        resolved[*block] = Some(parameter);
                        progressed = true;
                    }
                    merged[*block] = Some(parameter);
                } else {
                    conflicted[*block] = true;
                    resolved[*block] = None;
                    merged[*block] = None;
                    progressed = true;
                }
                continue;
            }
            let mut candidate = None;
            let mut conflict = false;
            for predecessor in predecessors {
                if conflicted[*predecessor] {
                    conflict = true;
                    break;
                }
                if let Some(register) = resolved[*predecessor] {
                    match candidate {
                        None => candidate = Some(register),
                        Some(current) if current == register => {}
                        Some(_) => {
                            conflict = true;
                            break;
                        }
                    }
                }
            }
            if conflict {
                conflicted[*block] = true;
                resolved[*block] = None;
                merged[*block] = None;
                progressed = true;
            } else if resolved[*block] != candidate {
                resolved[*block] = candidate;
                merged[*block] = None;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    let Some(value) = resolved[block_index] else {
        return Err(StoredLoadForwardingError::UnsupportedPair);
    };
    // When the load's own block sits on a deferred cycle — walked
    // predecessors reach back to it — the span after the load runs between
    // the carried register and the next iteration's read. It must be as
    // clear as every other walked span: no barrier kind, no unaccounted
    // access. An interfering write there is the last writer on the looping
    // path, so it is admitted only when it is itself a source of the
    // carried register; the register check below then treats the tail like
    // the rest of the interval.
    let head_looped = {
        let mut deferred_block = vec![false; function.blocks.len()];
        for (block, _) in &deferred {
            deferred_block[*block] = true;
        }
        let mut seen = vec![false; function.blocks.len()];
        let mut stack: Vec<usize> = deferred
            .iter()
            .find(|(block, _)| *block == block_index)
            .map(|(_, predecessors)| {
                predecessors
                    .iter()
                    .copied()
                    .filter(|predecessor| deferred_block[*predecessor])
                    .collect()
            })
            .unwrap_or_default();
        let mut loops = false;
        while let Some(block) = stack.pop() {
            if block == block_index {
                loops = true;
                break;
            }
            if seen[block] {
                continue;
            }
            seen[block] = true;
            if let Some((_, predecessors)) = deferred.iter().find(|(entry, _)| *entry == block) {
                stack.extend(
                    predecessors
                        .iter()
                        .copied()
                        .filter(|predecessor| deferred_block[*predecessor]),
                );
            }
        }
        loops
    };
    if head_looped {
        let tail = &function.blocks[block_index].instructions[load_index + 1..];
        for candidate in tail {
            barrier(candidate)?;
            let mut has_row = false;
            let mut interfered = false;
            for access in function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == candidate.id)
            {
                has_row = true;
                interfered |= disturbs(&read, access, structural_places, function);
            }
            if interfered {
                // The interfering instruction is the last writer on the
                // looping path: the read still observes the carried
                // register only when this writer itself sourced it.
                if covering_register(candidate, &read, function, environment)? != value {
                    return Err(StoredLoadForwardingError::AliasingWrite);
                }
                continue;
            }
            if !has_row {
                unaccounted(candidate)?;
            }
        }
        between.push((
            block_index,
            load_index + 1,
            function.blocks[block_index].instructions.len(),
        ));
    }
    if value == output {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    }
    let value_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == value)
        .ok_or(StoredLoadForwardingError::UnsupportedPair)?;
    // Nothing on the walked path may redefine the register each leg
    // carries or predefine the load's result: the store's tail, each
    // crossed block's terminator and its successor-edge transports, the
    // intervening block bodies, and the load's own head all keep those
    // registers identical after the copy. Each position's own resolved
    // register is the carried one — a block that resolved through its
    // merged parameter carries the parameter itself, and the edge binding
    // defining it is the one transport the audit lets through. A looped
    // head adds its own tail span and terminator: on the cyclic path they
    // run between the load and the block's next top.
    for (predecessor, target, successor) in &crossed {
        edge_keeps(
            successor,
            &read,
            resolved[*predecessor],
            merged[*target],
            output,
        )?;
    }
    for (walked_block, start, end) in &between {
        for instruction in &function.blocks[*walked_block].instructions[*start..*end] {
            span_keeps(instruction, resolved[*walked_block], output)?;
        }
    }
    for walked_block in &walked {
        if *walked_block != block_index || head_looped {
            span_keeps(
                terminator_instruction(&function.blocks[*walked_block].terminator),
                resolved[*walked_block],
                output,
            )?;
        }
    }
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    if copy.operands.len() != 2
        || copy.operands[0].operand != 0
        || copy.operands[0].access != RegisterOperandAccess::Use
        || copy.operands[0].class != value_register.class
        || copy.operands[1].operand != 1
        || copy.operands[1].access != RegisterOperandAccess::Def
        || copy.operands[1].class != output_register.class
        || !copy.implicit_uses.is_empty()
        || !copy.implicit_defs.is_empty()
        || !copy.clobbers.is_empty()
        || copy.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(StoredLoadForwardingError::ConstraintMismatch);
    }
    // The replacement keeps the load's exact read width: a same-width
    // store and load round-trip the register's low bits in the target's own
    // byte order, so `ZeroExtend` reconstructs the loaded value without any
    // endianness assumption.
    let kind = match read.byte_count {
        8 => SelectedInstructionKind::CopyI64,
        4 => SelectedInstructionKind::ZeroExtendU32,
        2 => SelectedInstructionKind::ZeroExtendU16,
        _ => SelectedInstructionKind::ZeroExtendU8,
    };
    let forwarded = SelectedInstruction {
        id: load,
        kind,
        constraint: copy.key,
        operands: copy
            .operands
            .iter()
            .zip([value, output])
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: copy.implicit_uses.clone(),
        implicit_defs: copy.implicit_defs.clone(),
        clobbers: copy.clobbers.clone(),
        provenance: forwarded_load.provenance.clone(),
    };
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        load_index,
        load_access,
        forwarded,
        between,
        crossed: crossed.len(),
        merge_work,
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction across
/// the plan, one per walked interval position, crossed edge, and
/// join-merge edge inspection, and one per roster row.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, StoredLoadForwardingError> {
    let interval = reconstructed
        .between
        .iter()
        .try_fold(0usize, |distance, (_, start, end)| {
            distance.checked_add(end - start)
        })
        .and_then(|total| total.checked_add(reconstructed.crossed))
        .and_then(|total| total.checked_add(reconstructed.merge_work))
        .ok_or(StoredLoadForwardingError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(interval))
        .and_then(|total| total.checked_add(function.memory_accesses.len()))
        .ok_or(StoredLoadForwardingError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| StoredLoadForwardingError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own record:
/// the named load is the reconstructed register move or `ZeroExtend`, and
/// the roster drops exactly the load's read row. The producer's `rewrite`
/// is not consulted; both sides derive the same function from the source
/// alone.
fn expect(reconstructed: &Reconstructed<'_>) -> SelectedFunction {
    let mut expected = reconstructed.function.clone();
    expected.blocks[reconstructed.block_index].instructions[reconstructed.load_index] =
        reconstructed.forwarded.clone();
    expected.memory_accesses.remove(reconstructed.load_access);
    expected
}

/// Reinsert the load instruction and its read row: undoing the validator's
/// expected edit must restore the complete source by content — every other
/// function, block, instruction, register, call, settlement, and access
/// retained bit-identical.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, StoredLoadForwardingError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(StoredLoadForwardingError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(StoredLoadForwardingError::ReplayMismatch)?;
    let slot = block
        .instructions
        .get_mut(reconstructed.load_index)
        .ok_or(StoredLoadForwardingError::ReplayMismatch)?;
    *slot = reconstructed.function.blocks[reconstructed.block_index].instructions
        [reconstructed.load_index]
        .clone();
    if reconstructed.load_access > function.memory_accesses.len() {
        return Err(StoredLoadForwardingError::ReplayMismatch);
    }
    function.memory_accesses.insert(
        reconstructed.load_access,
        reconstructed.function.memory_accesses[reconstructed.load_access].clone(),
    );
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the forwarding's preconditions from the source, requires the proposal to
/// equal the function its own record produces, and restores the complete
/// source by content. The producer's admission routine is never consulted,
/// so a wrong legality decision fails here even when the proposal matches
/// the edit the producer emitted.
pub fn validate_stored_load_forwarding(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    load: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
    let reconstructed = reconstruct(source, function_index, load, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed,
    )? > budget.validation_steps()
    {
        return Err(StoredLoadForwardingError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)) {
        return Err(StoredLoadForwardingError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(StoredLoadForwardingError::ReplayMismatch);
    }
    Ok(ValidatedStoredLoadForwarding {
        receipt: StoredLoadForwardingReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

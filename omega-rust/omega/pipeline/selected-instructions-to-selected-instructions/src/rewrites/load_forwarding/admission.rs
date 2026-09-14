//! Shared admission for store-to-load forwarding: locate the named `Load64`,
//! walk the same block back to the exact eight-byte `Store` that last wrote
//! its place range, and prove no intervening instruction can disturb it.
//!
//! Interference is decided from the validated access roster. A row naming the
//! forwarded place blocks on any overlapping or dynamic-extent write and on
//! any materialized local address for that place; rows for other places and
//! every read role are safe under place exclusivity. Instructions without a
//! row are admitted only when their kind cannot write semantic storage:
//! loads, address formation, private-slot frame accesses, and pure register
//! work. Calls, hosted effects, and unaccounted writers reject.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    SelectedMemoryAccess, SelectedMemoryAccessRole, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::PlaceId;

use super::StoredLoadForwardingError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub load_index: usize,
    pub load_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    pub value: VirtualRegisterId,
    pub output: VirtualRegisterId,
    /// Index of the load's single `ReadPlace` row in `memory_accesses`; replay
    /// requires the proposed roster to drop exactly this row.
    pub load_access: usize,
    pub copy: &'source RegisterInstructionConstraint,
}

/// One exact byte range within one place root.
struct Forwarded {
    place: PlaceId,
    byte_offset: u32,
}

impl Forwarded {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + 8
            && u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count)
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    load: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, StoredLoadForwardingError> {
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
    let SelectedInstructionKind::Load64 { byte_offset } = forwarded_load.kind else {
        return Err(StoredLoadForwardingError::UnsupportedInstruction);
    };
    // The read's semantic identity: exactly one roster row, one place root,
    // and the same eight bytes the instruction encodes.
    let mut rows = function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| access.instruction == load);
    let (load_access, read) = rows
        .next()
        .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
    if rows.next().is_some()
        || read.role != SelectedMemoryAccessRole::ReadPlace
        || read.byte_offset != byte_offset
        || read.byte_count != 8
    {
        return Err(StoredLoadForwardingError::UnsupportedPair);
    }
    let forwarded = Forwarded {
        place: read.place,
        byte_offset: read.byte_offset,
    };
    // The load's result must be defined only here; the copy keeps the register.
    let output = single_def(forwarded_load)?;
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
    // The load row must follow the target's [use pointer, def result] shape so
    // the replacement copy can reuse the result operand's class.
    let load_row = environment
        .constraint(forwarded_load.constraint)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    if load_row.operands.len() != 2
        || load_row.operands[0].operand != 0
        || load_row.operands[0].access != RegisterOperandAccess::Use
        || load_row.operands[1].operand != 1
        || load_row.operands[1].access != RegisterOperandAccess::Def
        || load_row.operands[1].class != output_register.class
    {
        return Err(StoredLoadForwardingError::ConstraintMismatch);
    }
    // Walk back to the last writer of the forwarded range. The first
    // potentially interfering access decides: an exact full-range referent
    // store forwards; anything else rejects.
    let mut found = None;
    for candidate_index in (0..load_index).rev() {
        let candidate = &block.instructions[candidate_index];
        reject_barrier(candidate)?;
        let mut has_row = false;
        let mut interfered = false;
        for access in function
            .memory_accesses
            .iter()
            .filter(|access| access.instruction == candidate.id)
        {
            has_row = true;
            interfered |= interferes(&forwarded, access);
        }
        if interfered {
            let value = forwarding_source(candidate, &forwarded, function, environment)?;
            found = Some((candidate_index, value));
            break;
        }
        if !has_row {
            reject_unaccounted(candidate)?;
        }
    }
    let (store_index, value) = found.ok_or(StoredLoadForwardingError::UnsupportedPair)?;
    if value == output {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    }
    let value_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == value)
        .ok_or(StoredLoadForwardingError::UnsupportedPair)?;
    // Nothing between the store and the load may redefine the carried value or
    // predefine the load's result; both stay register-identical after the copy.
    for between in &block.instructions[store_index + 1..load_index] {
        for operand in &between.operands {
            if operand.access != RegisterOperandAccess::Use
                && (operand.virtual_register == value || operand.virtual_register == output)
            {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            }
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
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(load_index.checked_sub(store_index)?))
        .and_then(|total| total.checked_add(function.memory_accesses.len()))
        .ok_or(StoredLoadForwardingError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| StoredLoadForwardingError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(StoredLoadForwardingError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        load_index,
        load_id: load,
        provenance: forwarded_load.provenance.clone(),
        value,
        output,
        load_access,
        copy,
    })
}

/// The load's sole register definition. A pinned, tied, or early-clobbered
/// result cannot become a same-identity copy through a clean copy row.
fn single_def(
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

/// Whether one roster row can disturb the forwarded bytes. Writes must target
/// the same place root to overlap; dynamic extents and escaped place-backed
/// addresses always block. Local-slot and outgoing-area storage never aliases
/// a referent place.
fn interferes(forwarded: &Forwarded, access: &SelectedMemoryAccess) -> bool {
    match access.role {
        SelectedMemoryAccessRole::WritePlace => {
            access.place == forwarded.place && forwarded.intersects(access)
        }
        SelectedMemoryAccessRole::WriteByteSequence { .. } => access.place == forwarded.place,
        SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } => {
            slot.structural_place() == Some(forwarded.place)
        }
        SelectedMemoryAccessRole::ReadPlace
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// The found writer must be a `Store` of all eight bits at the identical row:
/// one `WritePlace` access, matching byte offset, and a clean `[pointer,
/// value]` operand shape on the target's own constraint row.
fn forwarding_source(
    instruction: &SelectedInstruction,
    forwarded: &Forwarded,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    let reject = || StoredLoadForwardingError::AliasingWrite;
    let SelectedInstructionKind::Store {
        byte_offset,
        byte_size: 8,
    } = instruction.kind
    else {
        return Err(reject());
    };
    if byte_offset != forwarded.byte_offset {
        return Err(reject());
    }
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(reject());
    };
    if rows.next().is_some()
        || row.role != SelectedMemoryAccessRole::WritePlace
        || row.place != forwarded.place
        || row.byte_offset != forwarded.byte_offset
        || row.byte_count != 8
    {
        return Err(reject());
    }
    let row_constraint = environment
        .constraint(instruction.constraint)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    if row_constraint.operands.len() != 2
        || row_constraint.operands[0].operand != 0
        || row_constraint.operands[0].access != RegisterOperandAccess::Use
        || row_constraint.operands[1].operand != 1
        || row_constraint.operands[1].access != RegisterOperandAccess::Use
    {
        return Err(StoredLoadForwardingError::ConstraintMismatch);
    }
    let mut uses = instruction
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Use);
    let pointer = uses.next().ok_or_else(reject)?;
    let value = uses.next().ok_or_else(reject)?;
    if uses.next().is_some() || pointer.operand != 0 || value.operand != 1 {
        return Err(reject());
    }
    Ok(value.virtual_register)
}

/// Calls, hosted effects, and terminator kinds are always barriers: they can
/// write or expose reachable storage regardless of their roster rows, and a
/// terminator kind never belongs in a block body.
fn reject_barrier(instruction: &SelectedInstruction) -> Result<(), StoredLoadForwardingError> {
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

/// An interval instruction without a roster row must be unable to write any
/// semantic or place-backed storage: loads only observe, address forms only
/// compute, private-slot frame stores touch compiler-owned spill/boundary
/// slots, and pure register work has no memory side at all. A referent store
/// or any other frame slot without its row is an unaccounted write.
fn reject_unaccounted(instruction: &SelectedInstruction) -> Result<(), StoredLoadForwardingError> {
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
        | ExactAddI64Immediate { .. }
        | ExactSubtractI64Immediate { .. }
        | SaturatingAddU64
        | WrappingAddI64
        | SaturatingSubtractU64
        | ExactDivideU64 { .. }
        | WrappingRemainderI64 { .. }
        | BitwiseAndI64
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
        Store { .. } | StorePacked { .. } | Store64 { .. } => {
            Err(StoredLoadForwardingError::AliasingWrite)
        }
        _ => Err(StoredLoadForwardingError::UnsupportedInstruction),
    }
}

/// The one-instruction proposal shape shared with replay: the target's own
/// copy row supplies the operand interface while the register identity, the
/// instruction identity, and the read's provenance stay with the result.
pub(super) fn forwarded(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.load_id,
        kind: SelectedInstructionKind::CopyI64,
        constraint: admitted.copy.key,
        operands: admitted
            .copy
            .operands
            .iter()
            .zip([admitted.value, admitted.output])
            .map(
                |(operand, register)| selected_instructions::SelectedOperand {
                    operand: operand.operand,
                    virtual_register: register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                },
            )
            .collect(),
        implicit_uses: admitted.copy.implicit_uses.clone(),
        implicit_defs: admitted.copy.implicit_defs.clone(),
        clobbers: admitted.copy.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
}

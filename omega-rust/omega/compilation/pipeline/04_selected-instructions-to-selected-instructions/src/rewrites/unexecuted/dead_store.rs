//! Borrow-aware dead-store elimination on the selected CFG.
//!
//! A `Store { byte_offset, byte_size }` writes the low exact-width bits of
//! its value operand through the referent pointer, and a `StorePacked`
//! writes its packed width the same way; either can be the dead store, the
//! packed form only when its early-clobber scratch `Def` occurs nowhere
//! else in the function — a surviving mention would lose its definition to
//! the removal. Both place stores can also write the place's own local
//! storage — its `StructuralParameter`/`StructuralBlockParameter` slot, or
//! the producing operation's `Structural` home, each the place's storage
//! under the same byte coordinates — through the slot's materialized
//! address, and a `Store64` writes that slot directly; a dead store
//! carrying the `WriteLocal` row on such a slot is removed the same way.
//! The byte-sequence store is another dead-store route: a `Store { 0, 1 }`
//! through a fully computed view address carries one `WriteByteSequence`
//! row, so its dead extent is dynamic — the written byte sits at the row's
//! `byte_offset + index`, unbounded upward from that fixed offset.
//! A `CopyBytes` is the last dead-store route and the only one carrying
//! several roster rows: its destination `WriteByteSpan` writes `length`
//! bytes at a fixed `byte_offset` — again an extent unbounded upward —
//! and its source `ReadByteSpan` rides on the same instruction, so the
//! removal drops the whole row set the copy owns. Both scratch defs of the
//! copy's operand row take the packed store's custody check, and the count
//! operand's register must carry the span row's `length` value.
//! A `Structural` slot the place's declaration does not charge to that
//! operation only stages bytes that name the place — its writes are dead
//! stores of the slot's own bytes instead: no place-named roster row reaches
//! them, so a `Store`, `StorePacked`, or `Store64` into such a slot dies
//! under a later write covering the same slot range.
//! A later write of the place's storage whose byte range
//! covers the first store's range entirely replaces those bytes. When no
//! intervening instruction can observe or partially overwrite the first
//! store's bytes, the earlier store is dead: every observer of the place
//! sees the covering write's value, so the first instruction and its roster
//! row can be removed without changing any reachable memory state.
//!
//! The alias decision is borrow-aware: it comes from the validated
//! `memory_accesses` roster, not from pointer-register equality. Each access
//! row names the semantic `PlaceId` and the exact byte range the instruction
//! touches. Two simultaneous accesses that can write cannot share one
//! referent under different place identities — exclusivity rejects
//! overlapping exclusive custody before selection — so a write row for a
//! different `PlaceId` cannot disturb the dead bytes, and a read row for a
//! different place cannot observe them. Only an access on the dead place
//! itself matters: an overlapping read observes the bytes, a partial write
//! leaves them observable, and a
//! materialized local address can reach the same storage by another route.
//! A dynamic-extent row reaches only upward from its fixed byte offset — a
//! span covers `length` bytes there and a sequence row touches
//! `offset + index` — so one whose offset starts below the dead range's
//! end still reaches its last bytes and interferes like an overlapping
//! row, while one starting at or past the end is provably disjoint and
//! walks past like any disjoint row. When the dead extent is itself
//! dynamic the directions mirror: an exact or local row still reaches the
//! dead byte once its own extent ends past the row's fixed offset, and a
//! dynamic-extent row on the dead place always meets it.
//! The covering write must be the first access on the dead place after the
//! removed store, and the only row reaching the dead range must contain it
//! entirely — the row need only cover, not equal, the removed write. Two
//! routes to the place's storage qualify: a `Store` or `StorePacked`
//! carrying `WritePlace`, and a write into the place's own local storage
//! carrying `WriteLocal` — its `StructuralParameter`/`StructuralBlockParameter`
//! slot, or the `Structural` slot of the operation the place's declaration
//! names as its producer — either a `Store64` naming that slot or a place
//! store through its materialized address. Any other `Structural`
//! operation slot only stages bytes that merely name the place — a call's
//! staged view descriptor — so its writes and addresses are not the dead
//! bytes at all.
//! A `CopyBytes` into the place covers too — the only covering write with
//! several roster rows — when its destination `WriteByteSpan` is the sole
//! row reaching the dead range and its count register resolves to a clean
//! `MaterializeI64`: the span then writes a compile-time `count` bytes at
//! its fixed offset, and coverage is containment on constants. The copy's
//! other rows — its source read among them — must stay quiet on the dead
//! range, since a read reaching it would observe the bytes first.
//! A byte-sequence store covers an exact dead range the same way once its
//! index is itself constant: the `index` value's sole `InstructionResult`
//! register must resolve to a clean `MaterializeI64` with no
//! edge-transport or case-payload redefinition, so the write lands on the
//! one fixed byte `byte_offset + index` — and a single dead byte is covered
//! exactly when that is it. A runtime index still lands the write anywhere
//! at or past the payload base and cannot provably rewrite one fixed byte.
//! A byte-sequence dead store is covered only by another byte-sequence
//! store whose `WriteByteSequence` row spells the same byte: the same
//! payload base and the same `index` value place it exactly, and distinct
//! index values still do when each resolves to a clean `MaterializeI64`
//! under the same carrier audit the exact-range route runs — the two
//! `byte_offset + index` sums then name one fixed position apiece, and
//! equal sums are the same byte. Any exact or local row would have to
//! contain a byte placed at runtime, and a sequence write whose index
//! stays runtime or lands elsewhere may land on a different byte
//! entirely.
//! A `CopyBytes` dead store follows the same pattern one level up: while
//! its `length` stays runtime the destination extent is unbounded upward
//! from `byte_offset`, so only another `CopyBytes` spelling the same
//! extent — the same `byte_offset` and the same `length` value — provably
//! rewrites every byte it could have written, and no exact, local, or
//! sequence write can bound the reach. Once the `length` value's carrier
//! resolves to a clean `MaterializeI64` under the same audit — sole
//! `InstructionResult` carrier, no edge-transport or case-payload
//! redefinition — the dead extent collapses to `length` bytes at
//! `byte_offset` and every exact-range covering route applies unchanged:
//! a wider `CopyBytes` whose own count resolves, an exact store
//! containing the range, or — when the collapsed range is the one byte —
//! a byte-sequence write landing on it all cover.
//!
//! Instructions inserted by private-slot rewrites (spill stores, reloads,
//! frame addresses over `Spill`/`Boundary` slots) carry no roster row; they
//! touch compiler-owned activation storage that no referent place aliases, so
//! they can neither observe nor overwrite the dead bytes. Every other
//! memory-capable instruction without a row, every call, and every hosted
//! effect conservatively blocks elimination.
//!
//! The walk is not confined to one block: reaching a block's end without
//! interference continues through every successor edge of its terminator —
//! a covering write further down still rewrites the dead bytes when each
//! path forward reaches one before any observer. A block is only ever
//! entered uncovered, so its first interfering access decides all paths
//! through it at once; a block scanned clear defers coverage to its
//! distinct successors — a fork's legs may cover through different writes
//! or reconverge on one — and a join at a crossed block is harmless because
//! coverage looks forward. Terminator roster rows decide before each
//! crossed edge's transports, which may not write or retire the dead
//! place's storage. A terminator without successors and an edge back into
//! the store's own block — the removed store can never be its own covering
//! write — each end the walk in rejection.
//!
//! Coverage need not be proven on every path. The walked region is closed
//! under successor edges, so a path that never reaches a covering write is
//! confined to clear blocks forever — no access, transport, or terminator
//! on it can observe the dead bytes — and the store is dead on that path
//! too. Legs whose paths disagree about coverage and writerless cycles
//! admit on the same argument: the bytes are either rewritten before any
//! observer or never observable again.
//!
//! Removing the store shortens its block's instruction vector, so boundary
//! settlements positioned after it shift one ordinal earlier. Settlement
//! positions inside the dead interval — after the removed store in its block,
//! anywhere in a fully crossed block, or at or before the covering store in
//! its block — are rejected outright: a boundary event in that interval could
//! observe the dead bytes.
//! Validation re-derives the same legality contract from the source
//! records on its own audit — the store's roster row and shape, the
//! forward walk to a covering write or a permanently clear region, the
//! crossed-edge and boundary-settlement checks — and never consults the
//! producer's admission routine. It then requires the proposal to equal
//! the function that audit produces and restores the complete source by
//! content, so a wrong legality decision fails validation even when the
//! proposal matches the emitted edit.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use validation::validate_dead_store_elimination;

/// An accepted dead-store elimination with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedDeadStoreElimination {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: DeadStoreEliminationReceipt,
}

validated_transformed_receipt!(ValidatedDeadStoreElimination, DeadStoreEliminationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeadStoreEliminationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(DeadStoreEliminationReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeadStoreEliminationError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedPair,
    InterveningAccess,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for DeadStoreEliminationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid dead-store elimination: {self:?}")
    }
}

impl std::error::Error for DeadStoreEliminationError {}

/// Whether the register's provenance names `value`: entry parameters, block
/// parameters, and instruction results all carry it as `source_value`.
pub(super) fn origin_carries(
    origin: selected_instructions::VirtualRegisterOrigin,
    value: semantic_vocabulary::ValueId,
) -> bool {
    match origin {
        selected_instructions::VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | selected_instructions::VirtualRegisterOrigin::BlockParameter { source_value, .. }
        | selected_instructions::VirtualRegisterOrigin::InstructionResult {
            source_value, ..
        } => source_value == value,
        _ => false,
    }
}

/// The dead `StoreZeroFill`'s constant fill count: the `count` operand is a
/// `Use` whose register holds a clean `MaterializeI64` definition that no
/// edge transport redefines — only then does `byte_offset + count` bound
/// the filled span exactly.
pub(super) fn zero_fill_count(
    instruction: &selected_instructions::SelectedInstruction,
    function: &selected_instructions::SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<u32, DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    if environment.selected_keys().zero_bytes != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let count = instruction
        .operands
        .iter()
        .find(|operand| {
            operand.operand == 1 && operand.access == register_model::RegisterOperandAccess::Use
        })
        .ok_or_else(reject)?
        .virtual_register;
    let written = crate::rewrites::condition_state::materialized_bits(function, count)
        .map_err(|_| reject())?;
    if crate::rewrites::place_storage::transport_defines(function, count) {
        return Err(reject());
    }
    u32::try_from(written).map_err(|_| reject())
}

/// The storage route the named dead-store instruction takes, decoded once for
/// the admission and the replay: encoded offset and size, whether the packed
/// form's scratch `Def` needs the no-other-mention audit, the direct local
/// slot on the `Store64` route, and the byte-sequence flag.
pub(super) fn dead_store_route(
    kind: &selected_instructions::SelectedInstructionKind,
) -> Result<
    (
        u32,
        u32,
        bool,
        Option<selected_instructions::LocalStorageSlotId>,
        bool,
    ),
    DeadStoreEliminationError,
> {
    Ok(match kind {
        selected_instructions::SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(DeadStoreEliminationError::UnsupportedInstruction);
            }
            (*byte_offset, u32::from(*byte_size), false, None, false)
        }
        selected_instructions::SelectedInstructionKind::StorePacked { byte_offset, width } => (
            *byte_offset,
            u32::from(width.byte_size()),
            true,
            None,
            false,
        ),
        selected_instructions::SelectedInstructionKind::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Local(slot),
            byte_offset,
        } => (*byte_offset, 8, false, Some(*slot), false),
        selected_instructions::SelectedInstructionKind::CopyBytes => (0, 0, false, None, true),
        _ => return Err(DeadStoreEliminationError::UnsupportedInstruction),
    })
}

/// A boundary settlement inside the dead interval is an event a boundary
/// could observe the still-current bytes through: the origin block's
/// positions after the store, every fully crossed block, and each covering
/// block through its cover bound — the audit both sides run identically.
pub(super) fn settlement_clear(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    block_index: usize,
    store_index: usize,
    killers: &[(usize, usize)],
    crossed: &[selected_instructions::SelectedBlockId],
) -> Result<(), DeadStoreEliminationError> {
    let origin_cover = killers
        .iter()
        .find(|(killer, _)| *killer == block_index)
        .map(|(_, cover)| *cover);
    for settlement in function.boundary_settlements.iter() {
        let position = settlement.instruction_index as usize;
        if settlement.block == block.id {
            if store_index < position && origin_cover.is_none_or(|cover| position <= cover) {
                return Err(DeadStoreEliminationError::InterveningAccess);
            }
        } else if crossed.contains(&settlement.block)
            || killers.iter().any(|(killer, cover)| {
                *killer != block_index
                    && function.blocks[*killer].id == settlement.block
                    && position <= *cover
            })
        {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
    }
    Ok(())
}

/// Whether `register`'s only occurrence in `function` is one `Def` operand —
/// the custody a dropped early-clobber scratch requires. The operand itself
/// is that one occurrence: any other operand position, terminator operand,
/// or successor transport naming the register would leave the elimination
/// removing a definition a surviving read or second definition still
/// observes.
pub(super) fn scratch_dies(
    function: &selected_instructions::SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    crate::rewrites::register_occurs_once(function, register)
}

/// The copy-bytes store shape both sides prove the same way: the target's
/// declared row, the instruction's own operand contract, and the scratch
/// registers' sole-definition custody.
pub(super) fn byte_span_row_shape(
    instruction: &selected_instructions::SelectedInstruction,
    write: &selected_instructions::SelectedMemoryAccess,
    function: &selected_instructions::SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    use register_model::RegisterOperandAccess;
    use selected_instructions::SelectedMemoryAccessRole;

    if environment.selected_keys().copy_bytes != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if row.operands.len() != 5
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[2].operand != 2
        || row.operands[2].access != RegisterOperandAccess::Use
        || row.operands[3].operand != 3
        || row.operands[3].access != RegisterOperandAccess::Def
        || !row.operands[3].early_clobber
        || row.operands[4].operand != 4
        || row.operands[4].access != RegisterOperandAccess::Def
        || !row.operands[4].early_clobber
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    if instruction.operands.len() != 5
        || instruction.operands[3].operand != 3
        || instruction.operands[3].access != RegisterOperandAccess::Def
        || instruction.operands[4].operand != 4
        || instruction.operands[4].access != RegisterOperandAccess::Def
        || !scratch_dies(function, instruction.operands[3].virtual_register)
        || !scratch_dies(function, instruction.operands[4].virtual_register)
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let SelectedMemoryAccessRole::WriteByteSpan { length, .. } = write.role else {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    };
    let count = instruction
        .operands
        .iter()
        .find(|operand| operand.operand == 2 && operand.access == RegisterOperandAccess::Use)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?
        .virtual_register;
    let register = function
        .virtual_register(count)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if !origin_carries(register.origin, length) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}
/// Queue a crossed block's distinct successors on the walk frontier,
/// charging the block's id when the cursor left the store's own block.
/// Each side's edge audit stays its own: admission proves the transport
/// observes nothing, replay proves the transport keeps the bytes.
pub(super) fn crossed_successors(
    function: &selected_instructions::SelectedFunction,
    edges: &[&selected_instructions::SelectedSuccessor],
    block_index: usize,
    cursor: usize,
    cursor_id: selected_instructions::SelectedBlockId,
    queued: &mut [bool],
    frontier: &mut Vec<(usize, usize)>,
    audit: impl Fn(&&selected_instructions::SelectedSuccessor) -> Result<(), DeadStoreEliminationError>,
) -> Result<Option<selected_instructions::SelectedBlockId>, DeadStoreEliminationError> {
    for edge in edges {
        audit(edge)?;
    }
    for edge in edges {
        let next = function
            .block_index(edge.block)
            .ok_or(DeadStoreEliminationError::SourceMismatch)?;
        // An edge back into the dead store's own block re-executes the
        // removed store from its top — it can never be its own covering
        // write — so the looped path stays unproven.
        if next == block_index {
            return Err(DeadStoreEliminationError::UnsupportedPair);
        }
        if !queued[next] {
            queued[next] = true;
            frontier.push((next, 0));
        }
    }
    Ok((cursor != block_index).then_some(cursor_id))
}

/// The copy's covering row must be the span carrying the reach `length`
/// names: a single disturbing `WriteByteSpan` row of contractual zero count
/// on the declared `copy_bytes` instruction, whose operand-2 register
/// carries `length` — or, when the dead extent never resolved, the
/// identical span at `byte_offset`.
pub(super) fn covering_span_row(
    instruction: &selected_instructions::SelectedInstruction,
    byte_offset: u32,
    byte_count: u32,
    same_extent: Option<semantic_vocabulary::ValueId>,
    function: &selected_instructions::SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    disturbs: impl Fn(&selected_instructions::SelectedMemoryAccess) -> bool,
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    let mut covering = None;
    for access in function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id)
    {
        if !disturbs(access) {
            continue;
        }
        if covering.is_some() {
            return Err(reject());
        }
        covering = Some(access);
    }
    let Some(row) = covering else {
        return Err(reject());
    };
    // The span row names the dynamic extent the copy writes; its byte count
    // is contractual zero — the runtime `length` is the authoritative reach.
    let selected_instructions::SelectedMemoryAccessRole::WriteByteSpan { length, .. } = row.role
    else {
        return Err(reject());
    };
    if row.byte_count != 0 {
        return Err(reject());
    }
    // The target's declared `copy_bytes` row pins operand 2 as the count
    // use, so the register named there is the extent the instruction writes.
    if environment.selected_keys().copy_bytes != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let count = instruction
        .operands
        .iter()
        .find(|operand| {
            operand.operand == 2 && operand.access == register_model::RegisterOperandAccess::Use
        })
        .ok_or_else(reject)?
        .virtual_register;
    // Row-instruction agreement for a dynamic extent: the count register
    // must carry the `length` value the row claims, so a resolved constant
    // — or a same-extent dead span naming the value — really is the reach
    // the instruction writes.
    let register = function.virtual_register(count).ok_or_else(reject)?;
    if !origin_carries(register.origin, length) {
        return Err(reject());
    }
    // The same-extent route: the dead copy's `length` never resolved, so
    // its reach is unbounded upward and only a span writing the identical
    // extent — the same `byte_offset` and the same `length` value — can
    // provably rewrite all of it.
    if let Some(dead_length) = same_extent {
        return if row.byte_offset == byte_offset && length == dead_length {
            Ok(())
        } else {
            Err(reject())
        };
    }
    let written = crate::rewrites::condition_state::materialized_bits(function, count)
        .map_err(|_| reject())?;
    if crate::rewrites::place_storage::transport_defines(function, count) {
        return Err(reject());
    }
    if row.byte_offset > byte_offset {
        return Err(reject());
    }
    let needed = u64::from(byte_offset) + u64::from(byte_count) - u64::from(row.byte_offset);
    if written < needed {
        return Err(reject());
    }
    Ok(())
}

//! Shared admission for dead-store elimination: locate the named `Store`,
//! `StorePacked`, own-storage `Store64`, byte-sequence `Store { 0, 1 }`, or
//! `CopyBytes` destination span, prove its write row, then walk forward to
//! the first access on the dead place and require it to be a write of the
//! dead place's storage whose own row covers the dead range entirely.
//!
//! Interference is decided from the validated access roster. A row naming the
//! dead place interferes when it can observe the stored bytes or leave them
//! observable: any overlapping read, any write that is not
//! the exact covering write, or a materialized local address. A
//! dynamic-extent row interferes the same way while its fixed offset starts
//! below the dead range's end — its reach is unbounded upward, so only a row
//! beginning at or past that end is provably disjoint. A sequence row whose
//! `index` resolves to a clean `MaterializeI64` touches exactly the byte
//! `byte_offset + index` instead, so it interferes only by landing inside
//! the dead extent — a landing anywhere off it walks past. When the dead
//! extent is itself dynamic the directions mirror the store-motion walk: an
//! exact or local row still reaches the dead byte once its own extent ends
//! past the row's fixed offset, and a dynamic-extent row on the dead place
//! always meets it — unless the dead store's own extent decider resolved
//! the same way, collapsing the extent to the fixed bytes the constant
//! names before the walk. A `CopyBytes` dead store carries that second
//! dynamic shape: its destination `WriteByteSpan` claims `length` bytes at
//! its fixed `byte_offset`, so the may-written set is again unbounded
//! upward from `byte_offset`; a resolved `length` collapses it to the exact
//! range `[byte_offset, byte_offset + length)` and every covering route
//! applies unchanged, while an unresolved one is covered only by another
//! `CopyBytes` whose destination span spells the same extent — the same
//! `byte_offset` and the same `length` value — since nothing else can bound
//! an unbounded reach. Rows for other
//! places are safe under place exclusivity. A `WriteLocal` on the dead
//! place's own storage interferes exactly like a `WritePlace` on that place:
//! an overlapping row decides coverage below, a disjoint row walks past. The
//! place's own storage is its `StructuralParameter`/`StructuralBlockParameter`
//! slot or the producing operation's `Structural` home — the place's
//! declaration names that producer. Any other `Structural` slot only stages
//! bytes that name the place (a call's staged view descriptor) under its own
//! slot coordinates, so its writes and materialized address never touch the
//! dead bytes at all.
//! The staging slot is itself a dead-store subject: a `WriteLocal` on it
//! writes bytes that live under the slot's own coordinates, which no
//! place-named row reaches — reads of the staged bytes would have no honest
//! roster role, and the consuming operation sits behind the call barrier —
//! so only a row naming that very slot can observe or rewrite them. The dead
//! store carrying that row dies under a later write of the same slot's
//! covering range exactly like a place-storage write does.
//! Instructions without a row are admitted only when their kind cannot reach
//! semantic storage: private-slot frame accesses and pure register work.
//! Calls, hosted effects, and unaccounted writers reject.
//!
//! The walk is not confined to one block: reaching a block's end without
//! interference continues through its terminator's successor edges — every
//! edge out of a walked block is crossed, not only a single-successor
//! chain's, since a covering write further down still rewrites the dead
//! bytes when each path forward reaches one before any observer. A block is
//! only ever entered uncovered, so its first interfering access decides
//! every path through it at once: a covering write resolves the block for
//! all of them, anything else rejects. A block scanned clear defers coverage
//! to its distinct successors — a fork's legs may cover through different
//! writes or reconverge on one — and a join at a crossed block stays
//! harmless because coverage looks forward. The terminator's roster rows
//! decide first; each crossed edge's transports may not write or retire the
//! dead place's storage. A terminator with no successors lets the bytes
//! escape to the boundary, and an edge back into the store's own block
//! re-executes the removed store — which cannot be its own covering write —
//! so each ends the walk in rejection.
//!
//! A walked region needs no all-paths coverage proof to still eliminate:
//! the region is closed under successors, so a path that never reaches a
//! covering write is confined to clear blocks forever — no interfering
//! access, barrier, quiet-violating transport, or boundary escape exists on
//! it — and can never observe the dead bytes. Fork legs that disagree about
//! coverage, self-loops, and writerless cycles therefore admit: the store is
//! dead on every path, whether the path's bytes are rewritten or merely
//! never read again. Boundary settlements inside the walked region still
//! reject, since a boundary event positioned where the bytes remain current
//! could observe them.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedStructuralTransport, SelectedSuccessor, terminator_instruction,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{member_position, terminator_successors};
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, extent_intersects, extent_reached_by,
    local_slot_is_place_storage, local_store_shape, packed_store_row_shape, place_store_row_shape,
    slot_is_subject_storage, staging_slot, structural_place_declarations,
    write_role_covers_subject,
};
use crate::rewrites::window_hazards::{UnrosteredReach, unrostered_reach};

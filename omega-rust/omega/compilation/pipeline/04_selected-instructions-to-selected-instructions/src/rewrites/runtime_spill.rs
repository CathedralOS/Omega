//! Private, bit-preserving runtime-value storage. Every admitted use reads its
//! own storage through a reload pair; when admission proves some physical view
//! of the victim's class could host an interval at all — no implicitly used,
//! pinned, or reserved unit occupies every view — the first unpinned
//! instruction-operand use in a block emits the pair and every later unpinned
//! use names that same still-open reload register. How far that pair reaches is
//! the span policy: under `UnitWriteBounded` an instruction that can destroy
//! register content — a clobber or an implicit definition — closes the open
//! pair, so the next unpinned use opens a fresh one and no produced interval
//! ever reaches across a call. Under `UnitWriteCrossing` the open pair instead
//! survives such an instruction while an allocatable view of the victim's class
//! avoids every unit written inside the span — exactly the clobber-set-reduced,
//! most often callee-saved, home the produced interval then demands of the
//! allocator. ABI-pinned uses keep a private pair so the pin attaches
//! to the load-to-use window alone, and blocks with no surviving view keep the
//! per-use shape entirely.
//! The contract vocabulary — lineage, storage positions, transport sizing,
//! the span and surviving-home predicates, control projection — is shared;
//! the legality audit itself is not: the proposal's admission and the
//! validator's reconstruction each derive it from the source records alone,
//! so a defect inside either one fails rather than propagating. Proposal
//! inserts the private accesses while independent replay consumes them and
//! restores source.
//!
//! Instruction-result definitions must dominate every flexible use. Incoming parameters
//! instead require a dedicated edge definition on every predecessor: the edge
//! transfer's current output register (a copy or an already-admitted reload),
//! or — on a case-dispatch continuation — the
//! bridge's field observation behind the payload's own `Load32`/`Load64`.
//! Each is stored immediately, before later transfers create more pressure.
//! A later `Def` operand on the victim redefines the register, so the slot
//! tracks it the same way: one store after every definition keeps the slot a
//! mirror of the register's last write, a use on the redefining instruction
//! still reads the pre-write value through its own reload, and the
//! redefinition closes any still-open shared reload since the value it held
//! no longer restates the register. Two operand forms read and write the
//! victim in one instruction — the read-modify-write family. A `Def` tied to
//! a victim use is the two-operand form: the use reloads the pre-write value
//! as usual, the tie binds the write to that reload register's home, and the
//! redefinition's store reads the victim operand the write kept. A `UseDef`
//! operand is the one-operand form: the operand moves to a reload register,
//! the instruction writes its result there, and the store after the
//! instruction reads the emitted operand's register rather than the victim it
//! no longer defines. Either form must be the instruction's only victim write
//! — a second one leaves the slot without a defined last writer — and a
//! use-side tie onto a write to another register or a `UseDef` carrying its
//! own tie stay rejected. An early-clobber flag on the tied `Def` is itself
//! no hazard — the write can land only in the reload home its tie names — but
//! it may complete before an unrelated operand's read, so where the block
//! shares one open reload and an unpinned victim-reading co-operand could
//! read that same register, the tied use takes a dedicated pair of its own:
//! the write's tied home is then a register no other operand reads. A pinned
//! use or a private-pair block already leaves the write's register unshared,
//! so the flag alone never rejects. The origin
//! definition still dominates every use, so on each path the last executed
//! store is the register's reaching write.
//! Original parameter bindings remain exact. Replacing the destination's uses
//! makes that parameter dead, so fresh liveness no longer requires its edge
//! home tie. Their destination must likewise dominate every use.
//! An entry-bound register's definition is the function-entry boundary
//! itself: a scalar entry parameter, a structural parameter's incoming
//! pointer, or the hidden aggregate-result destination each arrives live-in,
//! so its single store opens the entry block. An edge back to that block
//! re-executes the store, and the register it reads still holds the boundary
//! value on every arrival: a consistent plan never rewrites an entry
//! register — no `Def`/`UseDef` operand may redefine a boundary live-in, and
//! no edge transport may name one as a destination — while the store's own
//! read keeps the register live at the boundary, so the transformed liveness
//! carries its interval across every re-entry path. A parameter-side
//! transport naming the register on such an edge instead substitutes the
//! edge's own arrival, which the same store writes. Its ABI live-in view
//! keeps pinning the register's home wherever that interval reaches. A
//! structural live-in's provenance is the contract's parameter row or
//! declared result place rather than a source site — it restates no
//! `ValueId`, so its reload registers carry a structural-observation origin
//! and no value binding may name it.
//! Instruction-defined structural registers follow the result definition
//! rule: a field observation's load result, a retained transport pointer
//! copy, or a snapshot chunk word carries a declared place and byte offset
//! rather than a `ValueId`, so its store lands after the defining instruction
//! and its reload registers carry that same coordinate in their observation
//! origin — exact enough that a reload can still serve a case-payload
//! argument whose transport demands the declared field. The definition's own
//! instruction may be an address producer — `FrameAddress`, `AddressOffset`,
//! or `ByteViewAddress` — since a resolved address round-trips its bits like
//! any other result; the single exception is the primitive-local
//! establishment idiom, where the `WritePlace` store or zero fill paired with
//! the `AddressLocal` access on that definition keeps the victim register in
//! its address operand verbatim, and the coordinate stays rejected where a
//! transport names a different place or field offset.
//!
//! A victim whose class cannot sit in the slot's access rows — an
//! ABI-resident IEEE-float register on a target whose frame transport is
//! GPR — still spills: its stored bytes are raw payload, so each store is
//! the target's own `Float*ToBits` conversion into the rows' shared carrier
//! class followed by the `Store64`, and each reload appends the matching
//! `BitsToFloat*` behind its `Load64`. The eight-byte private slot, the
//! slot-sharing analysis, and the boundary/redefinition ordering are
//! unchanged; the conversions are inert two-operand bridges — a row with an
//! implicit definition or clobber could destroy the still-open reload it
//! sits inside, so an impure or missing pair leaves the victim a
//! candidate-local rejection rather than a constraint fault.
//!
//! Terminator instruction operands are ordinary uses at one more position:
//! each reloads at the end of its block, after the last body instruction and
//! any definition store. A fixed view on such an operand (an ABI return or
//! exit register) stays on the rewritten operand, pinning the fresh reload
//! register to the same physical unit. The same holds for a fixed view on a
//! body instruction operand (an ABI call argument or result site): its reload
//! pair is inserted immediately before the consumer, so the precolored
//! segment covers exactly the load-to-use window. A `Registers` value-binding argument
//! on an outgoing successor reads at that same end-of-block position, after
//! the terminator instruction executes: its reload pair follows any
//! terminator-operand pairs in successor then binding order, and the binding
//! keeps its semantic declaration while moving to the fresh reload register.
//! A `Registers` case-payload argument reads at that same position: its pairs
//! follow the edge's binding pairs in payload order, and the payload's
//! declared type must equal the victim's exact type. The parameter side of a
//! binding or payload is not a use at all — it is the destination's
//! definition, admitted only when it is the block-parameter victim's own
//! incoming edge; every other naming stays rejected.
//!
//! A stored structural transport — a `Descriptor` or `WholeValue` snapshot
//! argument on an edge-transfer continuation — reads the victim only through
//! the bridge's own snapshot chunk loads, which admission treats as ordinary
//! body instruction uses: each reload pair sits before its load, inside the
//! bridge body rather than at the end-of-block position. The binding's
//! `argument` field then moves to the single register those loads name after
//! rewriting — no pair of its own is emitted — when they all share it: a
//! one-chunk snapshot always resolves to one register, and a multi-chunk
//! snapshot does while every chunk operand is unpinned and the block's shared
//! reload stays open from the first load through the edge. Where the loads
//! cannot share — a pinned chunk operand, a block keeping no shared reload,
//! or a span-closing instruction at or after the first load, which would
//! leave the loads' registers divergent or carrying a pre-close value the
//! edge no longer holds — the binding instead gains a dedicated end-of-block
//! reload pair of its own, the same idiom a value binding uses, and
//! `argument` names that fresh register. Admission requires the exact
//! chunk-load stream the transport's byte decomposition describes, recorded
//! as `ReadPlace` accesses for the edge and source place in this block. A
//! stored argument on the victim's own incoming edge or on a
//! non-continuation successor stays rejected.
//! Cyclic functions stay admitted: a back edge
//! reaching the destination is just one more incoming edge, and it must run
//! the same dedicated edge-copy definition whose store initializes the slot
//! before the destination — and therefore every dominated use — executes.
//!
//! Each retained rewrite shares unchanged selected functions. Replay still
//! restores and compares the complete source by content, so separately allocated
//! equivalent inputs work and corruption of an unrelated function rejects.
//!
//! A victim's accesses resolve to storage windows before any slot is
//! considered: definitions whose stores may serve one reload position share a
//! window because that load can read only one slot, while a redefinition's
//! new stores open a fresh window the earlier value's loads never join. Each
//! window independently shares an already-declared spill slot when the
//! candidate's every access follows this rewrite's own idiom — a zero-offset
//! `Store64`, or a zero-offset `FrameAddress` feeding only zero-offset
//! `Load64`s — and a last-writer replay over the function proves the
//! incumbent's accesses and the window's cannot interleave: no incumbent load
//! may observe one of the window's stores, and every window reload must be
//! reached by its own stores alone. Sharing appends no `local_storage_slots`
//! entry, so reused bytes stay charged to the frame exactly once; any other
//! naming of the candidate, an escaped address, or an interleaved window
//! falls back to the victim's private slot, appended once however many
//! windows share it.

mod admission;
mod case_payload_address;
mod rewrite;
mod slot;
mod unit_effects;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterClassId, RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedStructuralTransport, VirtualRegister,
    VirtualRegisterId,
};
use semantic_vocabulary::{EdgeId, FuelScheduleIdentity, PlaceId, ScalarType, ValueId};

pub use rewrite::{spill_selected_runtime_value, spill_selected_runtime_value_with_span_policy};

/// How far a block's still-open shared reload interval may reach. The
/// entrypoint fixes this once per rewrite so the proposal and the
/// validator's own reconstruction apply the same policy rule to the same
/// recorded unit writes — each computes its open/close decision itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSpillSpanPolicy {
    /// A clobber or implicit definition always closes the open pair: every
    /// produced interval stays inside a unit-free span and never demands a
    /// call-surviving home.
    UnitWriteBounded,
    /// A unit-writing instruction — most often a `CallUnit` — keeps the open
    /// pair while an allocatable view of the victim's class avoids every unit
    /// written inside the span so far: the surviving homes a call crossing
    /// demands. When no such view exists the instruction still closes the
    /// span, so the produced shape degrades to the bounded one.
    UnitWriteCrossing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeSpill {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RuntimeSpillReceipt,
}

validated_transformed_receipt!(ValidatedRuntimeSpill, RuntimeSpillReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSpillReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(RuntimeSpillReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSpillError {
    SourceMismatch,
    UnsupportedValue,
    UnsupportedControlFlow,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RuntimeSpillError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime-value spill: {self:?}")
    }
}

impl std::error::Error for RuntimeSpillError {}

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
pub(crate) enum VictimLineage {
    Scalar(ValueId),
    Structural { place: PlaceId, byte_offset: u32 },
}

/// Source definition coordinates, not proposed spill instructions. An incoming
/// parameter has one exact edge definition per predecessor: the predecessor's
/// own copy output, or a case bridge's field observation behind its load. An
/// entry-bound register's single definition is the function-entry boundary
/// itself.
pub(crate) struct StorageDefinition {
    pub block_index: usize,
    pub position: StoragePosition,
    /// The register the emitted store reads. `AfterUseDef` ignores this field:
    /// the operand was rewritten to a reload register and the store must read
    /// that emitted operand's register, resolved from the produced (or
    /// replayed) instruction at emission time.
    pub register: VirtualRegisterId,
}

/// Where the definition's store lands inside `block_index`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoragePosition {
    /// Immediately after the instruction that physically defines the stored
    /// register — the victim's own result or an edge's copy/observation output.
    AfterInstruction(SelectedInstructionId),
    /// Immediately after the instruction carrying `operand` — a `UseDef`
    /// operand on the victim. The read-modify-write leaves the new value in
    /// the register the operand was redirected to, so the store reads that
    /// emitted operand's register rather than the victim itself.
    AfterUseDef {
        instruction: SelectedInstructionId,
        operand: u16,
    },
    /// Ahead of every block instruction: an entry-bound register's boundary
    /// definition. Its register is live-in, so position zero is the earliest
    /// point the stored value exists and the only one every use follows.
    BlockStart,
}

/// One stored structural-transport argument — a `Descriptor` or `WholeValue`
/// snapshot source — naming the victim. The binding sits on an edge-transfer
/// continuation in `block`; its snapshot loads carry `ReadPlace` memory
/// accesses for `edge`/`place` in `byte_size`'s chunk decomposition, and
/// `loads` is their instruction identities in byte order. `dedicated_reload`
/// records that the loads cannot all name one register — pinned operands,
/// a block keeping no shared reload, or a span-closing instruction at or
/// after the first load — so the binding reads the victim's edge-time value
/// through its own end-of-block pair instead of following theirs.
pub(crate) struct StructuralArgumentUse {
    pub block: usize,
    pub edge: EdgeId,
    pub place: PlaceId,
    pub source_byte_offset: u32,
    pub byte_size: u32,
    pub source_read_roles: Vec<selected_instructions::SelectedMemoryAccessRole>,
    pub loads: Vec<SelectedInstructionId>,
    pub dedicated_reload: bool,
}

/// The declared bit-preserving pair bridging a foreign-class victim to the
/// slot's carrier class. `to_bits` re-exposes the payload in the carrier
/// class ahead of each store; `from_bits` restores the victim's class behind
/// each load. `to_kind`/`from_kind` are the selected instruction kinds those
/// rows realize.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BitsConversion<'source> {
    pub to_bits: &'source RegisterInstructionConstraint,
    pub from_bits: &'source RegisterInstructionConstraint,
    pub to_kind: SelectedInstructionKind,
    pub from_kind: SelectedInstructionKind,
}

/// The stored byte extent a structural transport snapshots — a descriptor's
/// fixed sixteen bytes, or a whole value's declared size. `Unused` transports
/// snapshot nothing.
pub(crate) fn stored_transport_size(transport: SelectedStructuralTransport) -> Option<u32> {
    match transport {
        // An address transport stores a pointer it forms; it snapshots nothing.
        SelectedStructuralTransport::Unused
        | SelectedStructuralTransport::Address { .. }
        | SelectedStructuralTransport::ScalarReference {
            source: selected_instructions::SelectedScalarReferenceSource::Canonical { .. },
            ..
        } => None,
        SelectedStructuralTransport::ScalarReference {
            source: selected_instructions::SelectedScalarReferenceSource::Descriptor,
            ..
        } => Some(16),
        SelectedStructuralTransport::WholeValue { byte_size, .. } => Some(u32::from(byte_size)),
    }
}

/// The chunk decomposition edge-transfer selection emits for one snapshot:
/// the largest of 8, 4, 2, 1 bytes at each remaining offset. The audit walks
/// the sorted access stream against this split rather than inferring widths.
/// `offset` stays at or below `byte_size`, so the subtraction cannot
/// underflow; once it equals `byte_size` no chunk remains and `None` is
/// returned for any further access.
pub(crate) fn next_chunk(byte_size: u32, offset: u32) -> Option<u8> {
    [8u32, 4, 2, 1]
        .into_iter()
        .find(|width| *width <= byte_size - offset)
        .map(|width| width as u8)
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
pub(crate) fn surviving_home_exists(
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

/// Project every selected terminator without conflating edge transport with
/// instruction operands. Both can keep a virtual value live across blocks.
/// Shared with the runtime-rematerialization recovery rewrite.
pub(crate) fn control(
    terminator: &selected_instructions::SelectedTerminator,
) -> (
    &selected_instructions::SelectedInstruction,
    [Option<&selected_instructions::SelectedSuccessor>; 2],
) {
    use selected_instructions::SelectedTerminator;
    match terminator {
        selected_instructions::terminator_exit_triple_pattern!(instruction, ..) => {
            (instruction, [None, None])
        }
        SelectedTerminator::Jump {
            instruction,
            successor,
        } => (instruction, [Some(successor), None]),
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => (instruction, [Some(when_nonzero), Some(when_zero)]),
        selected_instructions::conditional_branch_less_pair_pattern!(
            instruction,
            when_less,
            when_not_less,
            ..
        ) => (instruction, [Some(when_less), Some(when_not_less)]),
    }
}

/// The same projection for operand substitution. Successor edges and their
/// transports stay untouched; only the instruction's operand registers change.
/// Shared with the runtime-rematerialization recovery rewrite.
/// Fresh identity cursors for rewrite-inserted rows: one past the highest
/// instruction id anywhere in the function (terminators included) and one
/// past the highest virtual register id. Shared with the runtime-split and
/// runtime-rematerialization recovery rewrites, which attach their own
/// identity-overflow error.
pub(crate) fn next_instruction_id(function: &SelectedFunction) -> Option<u32> {
    function
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
}

/// The register twin of [`next_instruction_id`].
pub(crate) fn next_register_id(function: &SelectedFunction) -> Option<u32> {
    function
        .virtual_registers
        .iter()
        .map(|value| value.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
}

pub(crate) fn control_mut(
    terminator: &mut selected_instructions::SelectedTerminator,
) -> &mut selected_instructions::SelectedInstruction {
    selected_instructions::terminator_instruction_mut(terminator)
}

/// The same projection for successor transports. The terminator instruction is
/// untouched; only the outgoing successors' value bindings can move to fresh
/// reload registers. Order matches `control`: polarity zero before one.
pub(crate) fn control_successors_mut(
    terminator: &mut selected_instructions::SelectedTerminator,
) -> [Option<&mut selected_instructions::SelectedSuccessor>; 2] {
    use selected_instructions::SelectedTerminator;
    match terminator {
        selected_instructions::terminator_exit_triple_pattern!(..) => [None, None],
        SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => [Some(when_nonzero), Some(when_zero)],
        selected_instructions::conditional_branch_less_pair_pattern!(
            when_less,
            when_not_less,
            ..
        ) => [Some(when_less), Some(when_not_less)],
    }
}

/// Removing the initialization anchor must disconnect every use from entry.
/// Together with ordinary reachability, this proves dominance without assuming
/// that the block vector is topological or that a use is an edge-copy idiom.
/// Shared with the runtime-rematerialization recovery rewrite.
pub(crate) fn require_dominated_uses(
    function: &selected_instructions::SelectedFunction,
    anchor: usize,
    use_blocks: &[usize],
) -> Result<(), RuntimeSpillError> {
    let entry = function
        .block_index(function.entry_block)
        .ok_or(RuntimeSpillError::SourceMismatch)?;
    for bypass_anchor in [false, true] {
        let mut reached = vec![false; function.blocks.len()];
        let mut pending = vec![entry];
        while let Some(block_index) = pending.pop() {
            if reached[block_index] || (bypass_anchor && block_index == anchor) {
                continue;
            }
            reached[block_index] = true;
            for successor in control(&function.blocks[block_index].terminator)
                .1
                .into_iter()
                .flatten()
            {
                let destination = function
                    .block_index(successor.block)
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                pending.push(destination);
            }
        }
        if (!bypass_anchor && !reached[anchor])
            || use_blocks
                .iter()
                .any(|block_index| reached[*block_index] == bypass_anchor)
        {
            return Err(RuntimeSpillError::UnsupportedUse);
        }
    }
    Ok(())
}

/// The complete runtime-spill derivation the proposal's admission and the
/// validator's independent replay both compute: every gate and every
/// derived position, identical step for step. The family module owns it
/// once; each side keeps only its own record's name.
pub(super) struct RuntimeSpillDerivation<'source> {
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

pub(super) fn derive_runtime_spill<'source>(
    source: &'source impl crate::ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    span_policy: RuntimeSpillSpanPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<RuntimeSpillDerivation<'source>, RuntimeSpillError> {
    use optimization_unit::ValueDefinitionSite;
    use register_model::RegisterOperandAccess;
    use selected_instructions::{
        LocalStorageSlotId, SelectedBlockOrigin, SelectedCasePayloadTransport,
        SelectedInstructionKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
        SelectedSuccessorRole, SelectedValueTransport, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{EdgeId, IntegerSign, IntegerType, OperationId};
    use slot::{bits_transport, entry_definitions};
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            RuntimeSpillError::SourceMismatch
        })?;
    let victim = function
        .virtual_register(register)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64)
        .map_err(|_| RuntimeSpillError::UnsupportedValue)?;
    // Every register the representation's own entry-liveness replay
    // recognizes — a scalar or structural parameter, or the hidden
    // aggregate-result destination — has the function-entry boundary as its
    // definition anchor. Its ABI live-in pin is its own boundary constraint:
    // the boundary store's own read keeps the register live where it must
    // arrive — across any edge back to the entry block too — so the view
    // keeps pinning the register's home wherever that interval reaches. A
    // fixed view on any other origin stays a rejection.
    let entry_boundary = function.is_entry_register(victim);
    let (lineage, definition, block_index) = match victim.origin {
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            let block_index = slot::instruction_block(function, instruction)
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
                .block_index(block)
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
                .block_index(function.entry_block)
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
                .block_index(function.entry_block)
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
                .block_index(function.entry_block)
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
            let block_index = slot::instruction_block(function, instruction)
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
        entry_definitions(block_index, victim)
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
    // an (operation, place) pair, the `WritePlace` store — or the zero fill
    // of a large zeroed home — for that same pair must keep the victim
    // register; substituting a reload register would orphan the recorded
    // establishment. Reads, snapshot chunk loads, calls,
    // and the store's value operand are content-bound elsewhere — by source
    // value, by result origin, or by the access's own coordinates — so only
    // the paired store's address use is retained through rewriting.
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
    let mut retained_uses = std::collections::BTreeSet::new();
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
                // own incoming edges may carry it — parameter_definitions has
                // already checked every such arrival — or an entry-bound
                // victim's edge back to the boundary, where the boundary
                // store writes whatever the edge delivered.
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
                    SelectedStructuralTransport::ScalarReference {
                        base: selected_instructions::SelectedAddressBase::Register(argument),
                        ..
                    }
                    | SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Register(argument),
                        ..
                    } if argument == register)
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                let Some(byte_size) = stored_transport_size(binding.transport) else {
                    continue;
                };
                let SelectedStructuralTransport::WholeValue {
                    argument,
                    source_byte_offset,
                    ..
                } = binding.transport
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
                // that store, so that naming stays rejected. An entry-bound
                // victim's store instead opens the destination block itself —
                // after every bridge — so an edge back to the boundary may
                // carry the snapshot.
                if definition.is_none()
                    && !entry_boundary
                    && successor.block == function.blocks[block_index].id
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                structural_uses.push(StructuralArgumentUse {
                    block: current_block_index,
                    edge: successor.psi_edge,
                    place: binding.semantic.argument.place,
                    source_byte_offset,
                    byte_size,
                    source_read_roles: binding.source_read_roles().collect(),
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
                            // rule the value bindings above keep: a
                            // block-parameter victim's incoming edges carry
                            // it under parameter_definitions' checks, and an
                            // entry-bound victim's re-entry edge may name it
                            // the same way.
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
                                && (case.source.place() != Some(place)
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
                        // A `WritePlace` store or zero fill paired with the
                        // `AddressLocal` access on the victim's own definition
                        // keeps the address operand's register identity
                        // verbatim; the reload cannot stand in for it there.
                        // Operand 0 is the destination pointer of both kinds.
                        if established_stores.contains(&instruction.id) {
                            if operand.operand != 0
                                || !matches!(
                                    instruction.kind,
                                    SelectedInstructionKind::Store { .. }
                                        | SelectedInstructionKind::ZeroBytes
                                )
                            {
                                return Err(RuntimeSpillError::UnsupportedUse);
                            }
                            retained_uses.insert((instruction.id, operand.operand));
                            continue;
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
                        use_positions[current_block_index].record_use(
                            instruction_index,
                            operand.fixed_view.is_some(),
                            &mut flexible_uses[current_block_index],
                        );
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
                        use_positions[current_block_index].record_use(
                            instruction_index,
                            operand.fixed_view.is_some(),
                            &mut flexible_uses[current_block_index],
                        );
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
        (
            store,
            if store.operands.len() == 2 {
                &[RegisterOperandAccess::Use, RegisterOperandAccess::Def][..]
            } else {
                &[RegisterOperandAccess::Use][..]
            },
        ),
    ] {
        if row.operands.len() != accesses.len() || !row.implicit_defs.is_empty() {
            return Err(RuntimeSpillError::ConstraintMismatch);
        }
        for (ordinal, (operand, access)) in row.operands.iter().zip(accesses).enumerate() {
            if usize::from(operand.operand) != ordinal
                || operand.access != *access
                || operand.fixed_view.is_some()
                || operand.tied_to.is_some()
                || operand.early_clobber != (row.key == store.key && ordinal == 1)
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
    // The frame access stream cannot destroy a unit one of its own rows
    // requires, such as the stack pointer. Other declared clobbers are
    // admitted against source liveness at their actual insertion points.
    let access_rows = [address, load, store];
    if access_rows.iter().any(|writer| {
        writer.clobbers.iter().any(|unit| {
            access_rows
                .iter()
                .any(|reader| reader.implicit_uses.contains(unit))
        })
    }) {
        return Err(RuntimeSpillError::ConstraintMismatch);
    }
    let bits_conversion = if victim.class == carrier_class {
        None
    } else {
        Some(bits_transport(environment, victim, carrier_class)?)
    };
    let function_scan =
        crate::rewrites::function_scan(function).ok_or(RuntimeSpillError::IdentityOverflow)?;
    let mut steps = crate::rewrites::rewrite_scan(plan, function)
        // A converted victim emits one more instruction and one more
        // register at every store and every reload, so its replay charges
        // six per use and three per definition instead of four and one.
        // A target-declared frame-address scratch adds one register per store.
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
            total.checked_add(definitions.len().checked_mul(
                (if bits_conversion.is_some() { 3 } else { 1 }) + store.operands.len() - 1,
            )?)
        })
        .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
        // The open/close decision replays once over the victim function's own
        // instructions — the crossing simulation under `UnitWriteCrossing`,
        // the span-close collection under either policy.
        .and_then(|total| total.checked_add(function_scan))
        // Frame clobbers may require source unit-liveness at insertion
        // points. Unit-free access rows need no additional analysis.
        .and_then(|total| {
            total.checked_add(if access_rows.iter().any(|row| !row.clobbers.is_empty()) {
                function_scan
            } else {
                0
            })
        })
        // The structural-argument check groups the memory accesses once; the
        // establishment pairing above scans them twice more — once for the
        // `AddressLocal` records on the victim's definition, once for the
        // paired `WritePlace` stores. Each binding's walk of its own access
        // group is charged where the groups are known.
        .and_then(|total| total.checked_add(function.memory_accesses.len().checked_mul(3)?))
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    let charged =
        |steps: usize| u64::try_from(steps).map_err(|_| RuntimeSpillError::IdentityOverflow);
    let ceiling = crate::rewrites::required_rewrite_budget(plan, function, budget)
        .ok_or(RuntimeSpillError::IdentityOverflow)?
        .validation_steps();
    if charged(steps)? > ceiling {
        return Err(RuntimeSpillError::WorkBudgetExceeded);
    }
    require_dominated_uses(function, block_index, &use_blocks)?;
    let first_instruction =
        next_instruction_id(function).ok_or(RuntimeSpillError::IdentityOverflow)?;
    let first_register = next_register_id(function).ok_or(RuntimeSpillError::IdentityOverflow)?;
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
                && matches!(
                    access.role,
                    SelectedMemoryAccessRole::ReadPlace
                        | SelectedMemoryAccessRole::ReadIndexedPrimitive { .. }
                )
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
        // The owner index scans the function once more, and each binding
        // walks only the group its own edge and place recorded. Charging every
        // binding against every memory access of the function instead made
        // the bound quadratic where the walks are not.
        steps = structural_uses
            .iter()
            .try_fold(function_scan, |total, pending| {
                total.checked_add(
                    chunk_loads
                        .get(&(pending.edge, pending.place))
                        .map_or(0, Vec::len),
                )
            })
            .and_then(|walks| steps.checked_add(walks))
            .ok_or(RuntimeSpillError::IdentityOverflow)?;
        if charged(steps)? > ceiling {
            return Err(RuntimeSpillError::WorkBudgetExceeded);
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
            for access in accesses.iter().filter(|access| {
                access.byte_offset >= pending.source_byte_offset
                    && access.byte_offset - pending.source_byte_offset < pending.byte_size
                    && pending.source_read_roles.contains(&access.role)
                    && owners
                        .get(&access.instruction)
                        .is_some_and(|&(owner, position)| {
                            owner == pending.block
                                && block.instructions[position]
                                    .operands
                                    .first()
                                    .is_some_and(|operand| operand.virtual_register == register)
                        })
            }) {
                // A multidimensional selector contributes several evidence
                // rows to one physical snapshot load. Reload that load once.
                if pending.loads.contains(&access.instruction) {
                    continue;
                }
                let Some(width) = next_chunk(pending.byte_size, offset) else {
                    return Err(RuntimeSpillError::UnsupportedUse);
                };
                let source_offset = pending
                    .source_byte_offset
                    .checked_add(offset)
                    .ok_or(RuntimeSpillError::UnsupportedUse)?;
                if access.byte_offset != source_offset || access.byte_count != u32::from(width) {
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
                        byte_offset: source_offset,
                    },
                    4 => SelectedInstructionKind::Load32 {
                        byte_offset: source_offset,
                    },
                    2 => SelectedInstructionKind::Load16 {
                        byte_offset: source_offset,
                    },
                    _ => SelectedInstructionKind::Load8 {
                        byte_offset: source_offset,
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
    unit_effects::preserve_live_units(
        function_index,
        function,
        &definitions,
        &use_positions,
        address,
        load,
        store,
    )?;
    // Resolve every emitted access's slot at window granularity: definitions
    // one reload position may observe merge into a single storage window
    // because that load can name only one slot, and each window independently
    // takes the first declared spill slot the last-writer replay proves
    // disjoint from its accesses. Reuse declares nothing, so a shared slot is
    // charged to the frame once; windows no candidate admits share the
    // victim's private slot, appended once. The search scans this function
    // once per declared local slot. Reuse only shrinks the frame, so a search
    // that alone would carry the pass over budget is declined rather than
    // charged: every window then keeps the victim's private slot.
    let reuse_slots = function
        .local_storage_slots
        .len()
        .checked_mul(function_scan)
        .and_then(|slot_scan| steps.checked_add(slot_scan))
        .map(charged)
        .transpose()?
        .is_some_and(|total| total <= ceiling);
    let assignment = slot::assign(
        function,
        register,
        &definitions,
        &use_positions,
        &shared_reload,
        &span_closes,
        reuse_slots,
    );
    Ok(RuntimeSpillDerivation {
        function,
        use_blocks,
        shared_reload,
        dedicated_tie_uses,
        span_closes,
        retained_uses,
        victim,
        lineage,
        address_scalar_type: ScalarType::Integer(unsigned),
        definitions,
        assignment,
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
/// limit rather than an environment fault.
/// The verbatim per-side projection: the admission and replay records carry
/// the derivation's fields one-for-one — the macro keeps the restatement
/// single while each record type stays owned by its side.
macro_rules! runtime_spill_projection {
    ($Type:ident, $derived:ident) => {
        $Type {
            function: $derived.function,
            use_blocks: $derived.use_blocks,
            shared_reload: $derived.shared_reload,
            dedicated_tie_uses: $derived.dedicated_tie_uses,
            span_closes: $derived.span_closes,
            retained_uses: $derived.retained_uses,
            victim: $derived.victim,
            lineage: $derived.lineage,
            address_scalar_type: $derived.address_scalar_type,
            definitions: $derived.definitions,
            assignment: $derived.assignment,
            structural_uses: $derived.structural_uses,
            first_instruction: $derived.first_instruction,
            first_register: $derived.first_register,
            address: $derived.address,
            load: $derived.load,
            store: $derived.store,
            carrier_class: $derived.carrier_class,
            bits_conversion: $derived.bits_conversion,
        }
    };
}
pub(super) use runtime_spill_projection;

pub(super) fn parameter_definitions(
    function: &SelectedFunction,
    destination: usize,
    victim: &VirtualRegister,
    source_value: ValueId,
) -> Result<Vec<StorageDefinition>, RuntimeSpillError> {
    use register_model::RegisterOperandAccess;
    use selected_instructions::{
        SelectedBlockOrigin, SelectedCasePayloadTransport, SelectedSuccessorRole,
        SelectedTerminator, SelectedValueTransport, VirtualRegisterOrigin,
    };
    use slot::physical_definition;
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
                        .virtual_register(argument)
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
                    if case.source.place() != Some(place)
                        || byte_offset != payload.semantic.field_byte_offset
                    {
                        return Err(RuntimeSpillError::UnsupportedValue);
                    }
                    let (owner, load) = physical_definition(function, argument)?;
                    let (byte_count, displacement) =
                        match (payload.semantic.parameter.scalar_type, load.kind) {
                            (
                                ScalarType::Integer(integer),
                                SelectedInstructionKind::Load32 { byte_offset },
                            ) if integer.bits() == 32 => (4, byte_offset),
                            (
                                ScalarType::Integer(integer),
                                SelectedInstructionKind::Load64 { byte_offset },
                            ) if integer.bits() == 64 => (8, byte_offset),
                            _ => return Err(RuntimeSpillError::UnsupportedValue),
                        };
                    if owner != block_index
                        || load.id != instruction
                        || !case_payload_address::case_payload_address_corresponds(
                            function,
                            block_index,
                            load,
                            case.source,
                            successor.psi_edge,
                            byte_offset,
                            byte_count,
                            displacement,
                        )
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
                .virtual_register(argument)
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
            // The transfer consumes this register's current value, not the
            // opcode that first established it. A preceding spill can replace
            // the bridge copy with a reload while preserving exact lineage and
            // the edge binding. Store after that physical definition; requiring
            // CopyI64 here made two individually admitted spills non-composable.
            let (owner, definition) = physical_definition(function, argument)?;
            if owner != block_index
                || definition.id != instruction
                || !definition.operands.iter().any(|operand| {
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

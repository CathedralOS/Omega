//! The forward dead-on-path audit shared by the relocation rewrites that
//! move one member — or one contiguous run of members — across a branch
//! or a join.
//!
//! Moving the members changes which traversals execute them. A sink into
//! one arm removes their execution from every traversal that leaves the
//! branch through a different edge; a hoist into a fork head or a sink
//! into a join adds their execution to traversals that never ran them.
//! Either move is sound only when nothing on the affected paths can
//! observe the definitions the members no longer publish, or newly
//! publish: every register any member writes and every condition-state
//! unit any member defines or clobbers must be dead on those paths —
//! read by no body or terminator position and no edge transport before a
//! write retires the stale or foreign definition. A run's locations are
//! the union of its members': members move as one body, so a location one
//! member publishes and another consumes still leaves the block with the
//! run and must die on the paths that lose or gain the whole body.
//!
//! The audit is the forward mirror of `rewrites/condition_state`'s
//! entry-event walk. Per-block entry sets of still-live member locations
//! propagate forward through each successor edge's register surface — a
//! binding or case-payload argument or a structural transport's argument
//! reading a live location refuses, while a parameter or payload writing
//! one retires it — and scan each reached block's body and terminator
//! positions the same way: a reader of a live location refuses, a writer
//! of one kills it. Entry sets only ever hold member locations and grow by
//! union at joins, so the walk is a monotone fixpoint over a finite subset
//! lattice and terminates.
//!
//! Two positions are special. At the landing position the moved body's
//! occupancy on the walked paths decides what the walk sees
//! ([`Landing`]). Where every traversal reaching the block runs the body
//! there, its position republishes every location its members write and
//! clears the live set. Where its execution is speculative on the walked
//! paths, its reads belong to the added execution and go unaudited, while
//! its writes republish member locations that are foreign to the path:
//! the position re-seeds the live set rather than clearing it, so a
//! reader after it still refuses and only an ordinary writer retires
//! them. The destination instruction at the landing index sits after the
//! body in the transformed stream and is scanned against the published
//! set.
//!
//! In the body's own block the vacated span is absent from the
//! transformed stream — a dead path that loops back through it scans the
//! remaining positions only ([`Vacated`]). Whether the members' missing
//! writes diverge there depends on the move: when every walked path
//! reaching the vacated block crossed the body's new position first, or
//! when every position behind the vacated span that could observe the
//! missing writes sits inside the crossed window the hazard audit owns,
//! the span stays silent. When a walked path can reach the vacated block
//! without the body running — the confluence-hoist family, whose other
//! inflow edges arrive beside the body's new position rather than
//! through it — the vacated span is where the missing writes leave every
//! member location divergent, so each vacated position publishes them
//! into the live set and a later reader still refuses.
//!
//! The members' own reads need no audit on the source side either: the
//! window's position-level hazard proof already refuses every write to a
//! location a member reads between its old and new positions, on every
//! path — so the reaching definition it observes at the landing index is
//! the one it observed at its original position.
use std::collections::{BTreeSet, VecDeque};

use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
    SelectedStructuralTransport, SelectedSuccessor, SelectedValueTransport, VirtualRegisterId,
    terminator_instruction,
};

use crate::rewrites::block_edges::terminator_successors;

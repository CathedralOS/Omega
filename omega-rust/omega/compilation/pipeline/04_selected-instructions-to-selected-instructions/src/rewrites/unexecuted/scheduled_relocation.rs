//! Dominance-based relocation of a member run whose written locations die on
//! every abandoned path, on the selected CFG.
//!
//! This is the second general member-run mechanism in this area, and the two
//! divide by what they prove rather than by CFG shape. `super::relocation`
//! proves that the move changes no traversal, in either direction, for any
//! run, including the in-block case; it refuses a move whose gained or lost
//! traversal set is nonempty. This admission takes the moves it refuses that
//! are still sound: a sink into a block its own block dominates, where some
//! traversal no longer executes the run at all and the `dead_path` audit
//! proves nothing observes the locations the run wrote, and the mirrored
//! hoist. The price is a narrower subject — pure register and condition-state
//! work, cross-block only — so neither mechanism subsumes the other, and
//! merging them means one module with two admissions rather than one
//! admission.
//!
//! The sink into a fork arm is this region's shallow case, not a shape of
//! its own: the fork head dominates an arm no second edge reaches, and the
//! other edge is the skipped path the dead-path audit clears.
//! `member_sinks_into_the_immediate_fork_arm` lands in the arm itself,
//! `member_sinks_beneath_a_fork_arm` lands in a block deeper than any
//! per-shape family could name, and
//! `a_degenerate_single_arm_fork_sinks_with_no_skipped_path` is the boundary
//! where both edges reach the arm and the audit has nothing to clear.
//!
//! This module lived under `local_schedule`, which is the in-block pair
//! interchange, and nothing here ever read that parent. It sits beside
//! `relocation` because that is the operation it performs.
//!
//! Given a contiguous run of members in one block's body and a destination
//! position in a different block, it derives
//! every simple control-flow path between them, and from those paths the
//! crossed positions (the tail or head of the run's own block, each
//! traversed block's whole stream, and the destination block's share on
//! the far side of the landing index), the crossed edges, the skipped
//! edges that begin the traversals a sink no longer executes on, and the
//! traversals that would gain or lose the run — refused outright by the
//! once-per-traversal bound in either direction: sinking requires the
//! member's block to dominate the destination's, hoisting mirrors it —
//! the destination's block must dominate the member's, the member's
//! block must not be re-enterable without crossing the destination
//! again, and every continuation off the destination must reach the
//! member's block before it can exit or cycle. The hazard, dead-path,
//! and settlement audits then apply once to that derived window,
//! independent of which shape realized it.
//!
//! A window any per-shape family locates — the single `Jump` edge, the
//! fork's landing arm, the diamond's join — is a special case of the
//! derived region; windows the families cannot name, such as a run sinking
//! through a chain of sole-successor blocks or through one arm of a fork
//! into a deeper dominated block, or a run hoisting out of a join back
//! through its predecessors, are the shapes this admission exists to
//! cover. Moves where neither block dominates the other — across an
//! inflow or a branch the run would gain or lose on some traversal — and
//! in-block interchanges remain with the existing families and
//! `super::admission`.
//!
//! The member run must be pure register and condition-state work whose
//! written locations die on every path a sink abandons, the window's
//! crossed positions must be schedulable and uncoupled from every member,
//! every crossed edge must be a plain semantic successor the run does not
//! interfere with, no boundary settlement may observe a changed executed
//! prefix, and every block on a crossing path must be a plain `Source`
//! block. Validation consumes the proposed program, requires the run to
//! sit contiguous at the landing index in the destination block, and
//! restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

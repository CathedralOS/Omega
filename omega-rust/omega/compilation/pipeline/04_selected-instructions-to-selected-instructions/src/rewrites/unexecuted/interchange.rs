//! In-block interchange of two runs on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! module proves is the bounded interchange: two disjoint runs of body
//! instructions — each the contiguous span its named first and last members
//! bound — exchange places while every instruction between them keeps its
//! relative order, shifted by the difference of the runs' lengths.
//!
//! Naming one member as both bounds is the run of one, so a member against a
//! run and two members against each other are granularities of this
//! admission rather than families of their own. The memory rule is the
//! commutation audit: every pair of roster rows that newly trades order must
//! commute, shared through `rewrites/commuting_accesses`. A window whose
//! trading pairs carry rows on at most one side satisfies that by having no
//! pair to check, which is the at-most-one-accounted-actor case expressed as
//! the absence of a conflict rather than as a separate rule. Those two facts
//! are why the six families this replaced — two members, a member against a
//! run, two runs, each in a plain and a commuting form — were one operation.
//!
//! Two recorded accesses commute when neither can observe the other's
//! effect on the bytes it reaches — two non-writing rows never conflict,
//! rows reaching provably distinct storage never conflict, and rows on
//! shared storage must be disjoint fixed extents. Because every traded row
//! pair commutes, the relative order of conflicting accesses is unchanged:
//! none of the traded pairs conflict. The roster itself then follows the
//! new execution order — the rows naming the window's instructions are
//! permuted to match, each instruction's own rows keeping their relative
//! order — so the recorded accesses still appear in the order the program
//! performs them. Rows inside one run never trade order, since the run's
//! members keep their relative order as one body.
//!
//! Register and condition-state hazards, barriers, and settlements keep
//! the run interchange's audit unchanged: no register or unit a member
//! writes may be read or written by a crossed position in either
//! direction — a member's crossed positions are the other run's members
//! and the whole interior — calls, hosted effects, and call-roster entries
//! never sit in the window, and a boundary settlement inside the window's
//! span would observe a different executed prefix. Since no location is
//! written by both a member and a crossed position, the last writer before
//! every later observer — the block's terminator and edge transports
//! included — is the same instruction it was before the interchange, and
//! positions before the window are untouched.
//!
//! No instruction, register, constraint key, call, settlement, or edge
//! record changes: each instruction keeps its own id, kind, operands,
//! provenance, and implicit surface while only its position in the block
//! vector moves, and each roster row keeps its own identity while only
//! its position in the access vector follows the move.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block's
//! window and the roster's window rows to equal the independently
//! computed interchange, and restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

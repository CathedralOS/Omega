//! Independent validation of the arm relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! hoist's legality from the source records — the named member's arm
//! block (a plain source block whose every predecessor edge leaves one
//! fork head ending in a two-successor conditional branch), the named
//! destination's landing index in that head's body, the member's
//! hoistable kind, the crossed window's independence (no register or
//! condition-state hazard between the member and any crossed position,
//! no interference with a crossed edge's register transports, no
//! barrier, call, hosted effect, or call-roster entry inside the window,
//! and no boundary settlement whose observed executed prefix changes),
//! and the dead-path audit proving every member-written location dead on
//! every path the new position adds it to — then requires the proposal
//! to place exactly the member's instruction on the derived landing
//! index and restores the source by content: moving the member back must
//! reproduce every crossed instruction, every other block and
//! instruction, register, roster row, call, settlement, and edge. A
//! producer admission error therefore fails validation even when the
//! proposal is exactly what that producer emitted.

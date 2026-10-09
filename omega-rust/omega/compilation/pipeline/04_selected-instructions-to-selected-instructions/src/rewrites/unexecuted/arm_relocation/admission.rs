//! Shared admission for arm relocation: locate the named `member` in one
//! block's body, require that block to be a conditional arm — a plain
//! source block whose every predecessor edge leaves one fork head ending
//! in a two-successor conditional branch — locate the named `destination`
//! in that head's body, and prove the move sound in both directions — the
//! crossed window independent (no register or condition-state hazard
//! between the member and any crossed position, no interference with a
//! crossed edge's register transports, no barrier, call, hosted effect,
//! or call-roster entry inside the window, and no boundary settlement
//! whose observed executed prefix changes) and every member-written
//! location dead on every path the member's new position adds it to.

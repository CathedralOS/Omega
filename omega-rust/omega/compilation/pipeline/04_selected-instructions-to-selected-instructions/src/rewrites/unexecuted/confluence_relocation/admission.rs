//! Shared admission for confluence run relocation: locate the named
//! `first_member` and `last_member` bounding one contiguous run inside a
//! block's body, require that block to end in an unconditional `Jump`
//! whose plain semantic successor is a join at least one other
//! predecessor's edge also reaches — the destination's block — locate
//! the named `destination` there, and prove the move sound in both
//! directions — hand the crossed window to the shared run audit —
//! `crossed_window` derives the positions and edges every acyclic path
//! between the run's block and the join crosses (the lone `Jump` edge
//! under the gates below) and `admit_run_relocation` proves the window
//! independent once (no register or condition-state hazard between any
//! member and any crossed position, no member interference with the
//! crossed edge's register transports, no barrier, call, hosted effect,
//! or call-roster entry inside the window, and no boundary settlement
//! whose observed executed prefix changes) — and every member-written
//! location dead from the landing index forward, where the run's new
//! execution publishes them on arrivals that never ran it.

//! Shared admission for inflow relocation: locate the named `member` in
//! one block's body, locate the named `destination` in a block that ends
//! in the lone unconditional `Jump` reaching the member's block — a plain
//! semantic successor edge — require the member's block to be a join at
//! least one other predecessor's edge also reaches, and prove the move
//! sound in both directions — the crossed window independent, handed to
//! the shared run audit (`crossed_window` derives the positions and edges
//! every acyclic path between the inflow and the join crosses — exactly
//! this `Jump` edge under the gates below — and `admit_run_relocation`
//! proves the window independent once: no register or condition-state
//! hazard between the member and any crossed position, no interference
//! with the crossed edge's register transports, no barrier, call, hosted
//! effect, or call-roster entry inside the window, and no boundary
//! settlement whose observed executed prefix changes) and every
//! member-written location dead on the paths the move removes, where the
//! member's vacated index publishes them on arrivals that no longer run
//! it.

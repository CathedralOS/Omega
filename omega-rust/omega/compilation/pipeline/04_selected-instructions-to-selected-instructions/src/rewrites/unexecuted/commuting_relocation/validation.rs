//! Independent validation of the in-block commuting run relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the named run's span
//! inside one block, the destination's position outside the run, the
//! schedulable bar every window position meets, the register and
//! condition-state hazards between each member and every crossed
//! position, the commutation of every roster row that newly trades
//! order, the rowed-trading-pair accounting that keeps this family
//! disjoint from the plain run relocation, and the absence of a boundary
//! settlement inside the window's span — then requires the proposal to
//! place exactly the run's members on the destination's window edge in
//! their original order with the crossed positions shifted one run-width
//! toward the vacated span and the roster equal to the source's own rows
//! permuted into the new execution order, and restores the source by
//! content. Restoring the window's order and rows must reproduce the
//! complete selected program bit-for-bit. A producer admission error
//! therefore fails validation even when the proposal is exactly what
//! that producer emitted.

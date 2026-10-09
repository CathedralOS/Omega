//! Independent replay of member-run relocation: the validator
//! reconstructs the contiguous run the named members bound inside one
//! block's body, the destination block and landing index the destination
//! instruction names, and the window the move crosses on its own audit —
//! sharing no state with the producer's `admission` record — deriving
//! every acyclic path between the run block and the destination in the
//! direction that has them, re-proving that the move changes no traversal
//! (every predecessor edge into the arrival block lies on a crossed path
//! and every exit of every crossed block stays on one), and applying the shared
//! hazard, transport, memory-roster, and settlement audit once. It then
//! requires the proposed program to place exactly the run's members at
//! the landing index in their original order and restores the complete
//! source by content — every crossed instruction, every other block and
//! instruction, register, roster row, call, settlement, and edge
//! included.

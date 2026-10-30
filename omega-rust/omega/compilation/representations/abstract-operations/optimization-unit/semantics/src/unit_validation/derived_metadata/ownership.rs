//! Ownership-event reconstruction beside the unit's row owner.
//!
//! An ownership event is a pure projection of the operation it belongs to, so
//! the unit's construction, a moved node's replay and this leaf's independent
//! validation rebuild the rows from the one owner rather than from a second
//! 50-line spelling that could drift from it.
pub(crate) use optimization_unit::operation_ownership as expected_ownership;

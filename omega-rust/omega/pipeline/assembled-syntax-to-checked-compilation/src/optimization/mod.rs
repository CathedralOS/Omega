//! Optimizer module role: stage group. Checked optimization hooks and the
//! release rollback request.

pub(crate) mod checked_handoff;
pub(crate) mod checked_trees;
pub mod rollback;

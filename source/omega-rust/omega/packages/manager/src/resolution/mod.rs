//! Turn declared source requests into one validated package closure.
//!
//! [`package`] binds immutable source snapshots to package declarations.
//! [`graph`] follows those declared dependencies and reconciles their complete
//! identity and reachability. Read them in that order.

pub mod graph;
pub mod package;

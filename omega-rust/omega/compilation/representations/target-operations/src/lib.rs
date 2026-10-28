#![forbid(unsafe_code)]

//! Optimizer module role: crate map. target operations representation.
//!
//! Start at [`target_operations`], which defines the program root and maps the
//! subordinate control, value, call, storage, and evidence owners.

pub mod target_operations;
// The crate surface is the module's own roster: every name, module and helper
// the program root re-exports. Spelling it again here made two lists that had
// to be kept in step, and the sibling representation crates (`terminal-psi`,
// `checked-trees`) already read their module surface this way.
pub use target_operations::*;

pub use abstract_operations::ValueBinding;
pub use installation_evidence::InstalledAssumedBindingEvidence;

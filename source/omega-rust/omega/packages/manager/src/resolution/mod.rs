//! Turn one checked manifest into an exact immutable package closure.
//!
//! [`source`] binds declared local, workspace, and Git locations to source
//! custody. [`graph`] follows those bindings, rejects conflicts, and gives the
//! complete closure a canonical review subject.

pub mod graph;
pub mod source;

//! The settled execution of one Product target.
//!
//! Provider settlement chooses the supply of every boundary requirement the
//! accepted Terminal module reaches. `SelectedExecution` is what that leaves
//! for the products of one target: the target and the settled provider plans.
//! Anything else a product needs is derived, where it is used, from those
//! plans and from the accepted Terminal artifact.

#![forbid(unsafe_code)]

mod selected_execution;

pub use selected_execution::SelectedExecution;

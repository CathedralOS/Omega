//! Execution and inspection operations, usable without CLI parsing or process exits.
//!
//! Project Build belongs to `omega_build`; package operations remain owned by
//! `package_manager::operations`. Compiler
//! sequencing belongs to the compiler facade, never these project workflows.

pub mod execution;
pub mod inspection;

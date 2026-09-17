//! Complete application operations, usable without CLI parsing or process exits.
//!
//! Package operations remain owned by `package_manager::operations`. Compiler
//! stages remain ordinary functions below these project-level workflows.

pub mod compilation;
pub mod execution;
pub mod inspection;

mod temporary_directory;

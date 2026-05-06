//! Name and module resolution will live here.
//!
//! This crate should answer questions like "which item does this name refer
//! to?" without performing type checking or graph lowering.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolveStage;

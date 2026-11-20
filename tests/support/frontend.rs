//! Shared fixtures for clients of the source compiler.
use source_admission::LoadedSource;
use source_compiler::test_support::{lexing, parsing};
#[path = "frontend_operations.rs"]
mod operations;
pub use operations::*;

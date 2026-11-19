//! Shared fixtures for clients of the source compiler.
use source_admission::LoadedSource;
use source_compiler::{lexing, parsing};
#[path = "frontend_operations.rs"]
mod operations;
pub use operations::*;

//! What the lexer's test files share: the invariant checks and the text form the fixtures compare.

// Cargo compiles this module once per test file, and each file uses only some of it.
#![allow(dead_code, unused_imports)]

pub mod token_dump_formatter;
pub mod token_invariants;

pub use token_dump_formatter::{dump_error, dump_tokens};
pub use token_invariants::{describe_relex_problem, describe_tiling_problem};

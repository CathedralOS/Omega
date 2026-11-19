//! What the parser's test files share: checks on a tree that need no grammar, and the text form the fixtures compare.

// Each test binary compiles this module separately and uses a different part of it.
#![allow(dead_code, unused_imports)]

pub mod tree_invariants;

pub use spec_corpus::tree_dump_formatter::{dump_parse_error, dump_tree};
pub use tree_invariants::describe_tree_problem;

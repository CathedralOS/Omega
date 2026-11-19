#![forbid(unsafe_code)]

#[cfg(test)]
use source_declarations::source_inputs;
use source_declarations::{declaration_selection, source_symbols};
use typed_trees::source_state;

#[path = "src/lib.rs"]
pub mod preparation;

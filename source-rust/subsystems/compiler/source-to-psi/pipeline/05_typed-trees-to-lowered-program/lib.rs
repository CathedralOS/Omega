#![forbid(unsafe_code)]

use source_declarations::{declaration_selection, source_selections};
#[cfg(test)]
use source_symbol_table::source_inputs;
use source_symbol_table::{source_symbols, source_tables};
use typed_trees::source_state;

#[path = "src/lib.rs"]
pub mod lowering;

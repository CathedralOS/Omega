#![forbid(unsafe_code)]

use source_declarations::{declaration_selection, source_selections, source_tables};
#[cfg(test)]
use source_symbol_table::source_inputs;
use source_symbol_table::source_symbols;
use typed_trees::source_state;

#[path = "src/lib.rs"]
pub mod lowering;

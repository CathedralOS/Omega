#![forbid(unsafe_code)]

use source_declarations::declaration_selection;
#[cfg(test)]
use source_symbol_table::source_inputs;
use source_symbol_table::source_symbols;
use typed_trees::source_state;

#[path = "src/lib.rs"]
pub mod preparation;

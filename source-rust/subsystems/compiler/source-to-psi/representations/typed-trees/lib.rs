#![forbid(unsafe_code)]

use source_declarations::{declaration_selection, source_selections};
use source_symbol_table::{source_inputs, source_symbols, source_tables};

#[path = "src/lib.rs"]
pub mod source_state;

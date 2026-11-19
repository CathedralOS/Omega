#![forbid(unsafe_code)]

use source_declarations::{declaration_selection, source_selections, source_tables};
use source_symbol_table::{source_inputs, source_symbols};

#[path = "src/lib.rs"]
pub mod source_state;

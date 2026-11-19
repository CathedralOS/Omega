#![forbid(unsafe_code)]

use source_declarations::{
    declaration_selection, source_inputs, source_selections, source_symbols, source_tables,
};

#[path = "src/lib.rs"]
pub mod source_resolved;

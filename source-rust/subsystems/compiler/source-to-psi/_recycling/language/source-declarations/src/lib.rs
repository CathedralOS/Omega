//! Source coordinates, symbol tables and declaration selections shared by resolution and typing.
#![forbid(unsafe_code)]

pub mod declaration_selection;
pub use source_symbol_table::source_inputs;
pub mod source_selections;
pub mod source_symbols;
pub mod source_tables;

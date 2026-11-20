#![forbid(unsafe_code)]

#[cfg(test)]
use source_admission::LoadedSource;
use source_declarations::{declaration_selection, source_selections};
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
use source_symbol_table::{source_inputs, source_symbols, source_tables};
use symbol_resolved_trees::source_resolved;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
use typed_trees::source_state;
#[cfg(test)]
#[path = "../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

pub mod data_applications;
pub mod product_declarations;
#[path = "src/lib.rs"]
pub mod typing;

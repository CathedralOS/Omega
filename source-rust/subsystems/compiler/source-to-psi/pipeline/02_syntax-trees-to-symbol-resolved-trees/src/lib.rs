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
#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

#[path = "resolver.rs"]
pub mod resolution;

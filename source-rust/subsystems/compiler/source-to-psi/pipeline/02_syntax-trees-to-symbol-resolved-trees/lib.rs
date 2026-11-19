#![forbid(unsafe_code)]

use source_declarations::{
    declaration_selection, source_inputs, source_selections, source_symbols, source_tables,
};
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
use symbol_resolved_trees::source_resolved;
#[cfg(test)]
use source_admission::LoadedSource;
#[cfg(test)]
#[path = "../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

#[path = "resolver.rs"]
pub mod resolution;

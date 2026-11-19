#![forbid(unsafe_code)]

#[cfg(test)]
use source_declarations::declaration_selection;
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
#[cfg(test)]
use source_symbol_table::{source_inputs, source_symbols};
#[cfg(test)]
use symbol_resolved_trees_to_typed_trees::typing;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
#[cfg(test)]
use typed_trees::source_state;
#[cfg(test)]
use typed_trees_to_lowered_program::lowering;
#[cfg(test)]
#[path = "../prepare_and_check/typed_preparation.rs"]
mod preparation;
#[path = "src/lib.rs"]
pub mod checking;
#[cfg(test)]
use source_admission::LoadedSource;
#[cfg(test)]
#[path = "../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

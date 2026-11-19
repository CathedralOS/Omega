#![forbid(unsafe_code)]

#[path = "src/package_review.rs"]
mod package_review;
#[path = "src/source_publication.rs"]
mod source_publication;
#[path = "src/source_roster.rs"]
pub mod source_roster;
pub use source_publication::SourcePublication;
#[path = "src/provider_choice_inputs.rs"]
pub mod provider_choice_inputs;
#[path = "src/provider_supply/mod.rs"]
pub mod provider_supply;
pub use production::*;
use source_declarations::{declaration_selection, source_inputs, source_symbols};
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
pub use source_roster::SourceCompilationRoster;
#[cfg(test)]
use symbol_resolved_trees_to_typed_trees::typing;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
use terminal_psi_to_checked_terminal_psi::checking;
use typed_trees::source_state;
use typed_trees_to_terminal_psi::lowering;
#[cfg(test)]
use typed_trees_to_typed_trees::preparation;
#[cfg(test)]
#[path = "../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
#[path = "src/lib.rs"]
pub mod production;

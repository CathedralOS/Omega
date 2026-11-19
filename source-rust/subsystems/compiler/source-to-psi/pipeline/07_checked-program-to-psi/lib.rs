#![forbid(unsafe_code)]

#[path = "src/package_review.rs"]
mod package_review;
#[path = "src/source_publication.rs"]
mod source_publication;
pub use source_publication::SourcePublication;
#[path = "src/provider_choice_inputs.rs"]
pub mod provider_choice_inputs;
#[path = "src/provider_supply/mod.rs"]
pub mod provider_supply;
use lowered_program_to_checked_program::checking;
pub use production::*;
#[cfg(test)]
use source_admission::LoadedSource;
use source_declarations::declaration_selection;
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
use typed_trees_to_lowered_program::lowering::source_roster::SourceCompilationRoster;
use source_symbol_table::{source_inputs, source_symbols};
#[cfg(test)]
use symbol_resolved_trees_to_typed_trees::typing;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
use typed_trees::source_state;
use typed_trees_to_lowered_program::lowering;
#[cfg(test)]
#[path = "../prepare_and_check/typed_preparation.rs"]
mod preparation;
#[cfg(test)]
#[path = "../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
#[path = "src/lib.rs"]
pub mod production;

#[cfg(test)]
fn judge_fixture(
    source: std::sync::Arc<source_state::FrozenTypedTrees>,
    compilation: lowering::Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<diagnostics::Diagnostic>> {
    let lowered = lowering::LoweredSourcePublication::prepare(source, compilation, publication)?;
    AcceptedTerminalSource::from_judgment(checking::judge_publication(lowered)?)
}

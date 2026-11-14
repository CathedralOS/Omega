// Products publish the accepted Terminal module once and check each target's
// conditions against that same artifact and the target's settled plans.

mod product_code;
pub(crate) use product_code::ProductCodeSource;
mod provider_execution;
pub(crate) use provider_execution::prepare_provider_execution_inputs;
mod program_entry;
mod terminal_artifact;

pub(crate) use program_entry::prepare_entry_metadata;
pub use program_entry::prepare_program_entry;

pub(crate) use terminal_artifact::{
    ProducedTerminalProduct, TerminalProductRequest, check_terminal_conditions,
    produce_retained_terminal_artifacts,
};

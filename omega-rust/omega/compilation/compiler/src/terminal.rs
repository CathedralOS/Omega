// Products publish the accepted Terminal module once and check each target's
// conditions against that same artifact and the target's settled plans.

mod entry_calling_plans;
mod native_product;
mod product_code;
mod program_entry;
mod terminal_artifact;

pub(crate) use entry_calling_plans::prepare as prepare_entry_calling_plans;
pub(crate) use native_product::prepare as prepare_native_application;
pub(crate) use program_entry::prepare_entry_metadata;
pub use program_entry::prepare_program_entry;

pub(crate) use terminal_artifact::{
    ProducedTerminalProduct, TerminalProductRequest, check_terminal_conditions,
    produce_retained_terminal_artifacts,
};

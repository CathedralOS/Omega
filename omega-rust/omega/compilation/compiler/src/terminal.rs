// Products publish the accepted Terminal module once and check each target's
// conditions against that same artifact and the target's settled plans.

mod product_code;
mod terminal_artifact;

pub(crate) use terminal_artifact::{
    ProducedTerminalProduct, TerminalProductRequest, check_terminal_conditions,
    produce_retained_terminal_artifacts,
};

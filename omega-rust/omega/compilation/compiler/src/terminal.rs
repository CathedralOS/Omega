// Products publish the accepted Terminal module once and attach target entry,
// callback and behavior-exclusion custody to that same artifact. Source-based
// calling-plan capture has been removed with its old checker dependency.

mod behavior_exclusions;
mod product_code;
mod terminal_artifact;

pub(crate) use terminal_artifact::{
    ProducedTerminalProduct, TerminalProductRequest, TerminalProductUse, TerminalProductionView,
    check_terminal_conditions, produce_retained_terminal_artifacts,
};

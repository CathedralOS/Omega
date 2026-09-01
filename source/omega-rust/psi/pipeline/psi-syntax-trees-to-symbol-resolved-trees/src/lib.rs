#![forbid(unsafe_code)]

//! Resolves names in parsed Omega source into stable Psi symbol identities.

mod authored_selections;
mod conformance_blocks;
mod constant;
mod data;
mod domain;
mod domain_establishment;
mod domain_operator_homes;
mod expression;
mod item;
mod lowerer;
mod machine;
mod machine_parameter_requirements;
mod measure;
mod name;
mod operator;
mod proposition;
mod service_reaches;
mod signature_free_requirements;
mod state;
mod statement;
mod symbols;
mod trait_defaults;
mod trait_definition;
mod type_reference;
mod wire;

pub use lowerer::{
    RebasedSeededSymbolResolvedTrees, SeededSymbolResolvedTrees,
    lower_syntax_extension_against_resolved_base,
    lower_syntax_extension_with_authored_selection_frontier, lower_syntax_trees,
    lower_syntax_trees_with_sources, lower_syntax_trees_with_sources_and_top_level_bindings,
};
pub use trait_defaults::synthesize_trait_defaults;

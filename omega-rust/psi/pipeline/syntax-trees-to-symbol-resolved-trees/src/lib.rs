#![forbid(unsafe_code)]

//! Resolves names in parsed Omega source into stable Psi symbol identities.
//!
//! Start at `resolution.rs`: it is the route, one call per phase. Before any
//! symbol exists, `module_normalization` validates the forest, `generic_data`
//! closes eligible generic data applications, and `trait_defaults`
//! synthesizes default trait machines; all three consume syntax and return
//! syntax. The route then walks every root item through `lowering`, which
//! translates each syntax node into the symbol-resolved carrier and leaves
//! pending selections on the `lowerer` state. `symbols` builds the symbol
//! table and assigns identities. `selection` binds what lexical lookup alone
//! cannot settle: operator homes, establishment routes, closed conformance
//! rows, machine-parameter requirements, service reaches, evidence
//! forwardings, and the authored-selection ledger the checked stage consumes.
//! `constant` owns constant declarations, substitution, and initializer
//! custody across every phase. `continuations` holds the carriers a later
//! phase resumes from.

mod constant;
mod continuations;
mod generic_data;
mod lowerer;
mod lowering;
mod module_normalization;
mod resolution;
mod selection;
mod symbols;
mod trait_defaults;

pub use constant::initializer_dependencies::ConstInitializerDependencies;
pub use constant::requires_const_initializer_evaluation;
pub use continuations::{
    ConstInitializerSelection, RebasedSeededSymbolResolvedTrees, SeededSymbolResolvedTrees,
};
pub use generic_data::{
    canonicalize_declared_const_definition, closed_data_const_argument_expressions,
    closed_machine_const_arguments, normalize_generic_data,
    normalize_generic_data_with_retained_base,
    normalize_generic_data_with_sources_and_top_level_bindings,
};
pub use resolution::{
    lower_syntax_extension_against_resolved_base,
    lower_syntax_extension_with_authored_selection_frontier, lower_syntax_trees,
    lower_syntax_trees_for_const_argument_selection,
    lower_syntax_trees_for_const_initializer_selection, lower_syntax_trees_with_sources,
    lower_syntax_trees_with_sources_and_top_level_bindings,
};
pub use trait_defaults::synthesize_trait_defaults;

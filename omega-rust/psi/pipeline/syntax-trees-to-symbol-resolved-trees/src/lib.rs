#![forbid(unsafe_code)]

//! Resolves names in parsed Omega source into stable Psi symbol identities.
//!
//! `lowerer` owns the route. Before any symbol exists, `module_normalization`
//! validates the forest, `generic_data` closes eligible generic data
//! applications, and `trait_defaults` synthesizes default trait machines;
//! all three consume syntax and return syntax. The lowerer then walks every
//! root item through `lowering`, which translates each syntax node into the
//! symbol-resolved carrier and leaves pending selections on the lowerer.
//! `symbols` builds the symbol table and assigns identities. `selection`
//! binds the references lexical lookup alone cannot settle: operator homes,
//! establishment routes, closed conformance rows, machine-parameter
//! requirements, service reaches, and the authored-selection ledger the
//! checked stage consumes. `constant` owns constant declarations,
//! substitution, and initializer custody across every phase.

mod constant;
mod generic_data;
mod lowerer;
mod lowering;
mod module_normalization;
mod selection;
mod symbols;
mod trait_defaults;

pub use constant::initializer_dependencies::ConstInitializerDependencies;
pub use constant::requires_const_initializer_evaluation;
pub use generic_data::{
    canonicalize_declared_const_definition, closed_data_const_argument_expressions,
    closed_machine_const_arguments, normalize_generic_data,
    normalize_generic_data_with_retained_base,
    normalize_generic_data_with_sources_and_top_level_bindings,
};
pub use lowerer::{
    ConstInitializerSelection, RebasedSeededSymbolResolvedTrees, SeededSymbolResolvedTrees,
    lower_syntax_extension_against_resolved_base,
    lower_syntax_extension_with_authored_selection_frontier, lower_syntax_trees,
    lower_syntax_trees_for_const_argument_selection,
    lower_syntax_trees_for_const_initializer_selection, lower_syntax_trees_with_sources,
    lower_syntax_trees_with_sources_and_top_level_bindings,
};
pub use trait_defaults::synthesize_trait_defaults;

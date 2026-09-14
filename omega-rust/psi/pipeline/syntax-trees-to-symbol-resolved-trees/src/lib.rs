#![forbid(unsafe_code)]

//! Resolves names in parsed Omega source into stable Psi symbol identities.
//!
//! Start at `resolution.rs`: it is the route, one call per phase. The folders
//! are its owners. `preparation` rewrites syntax before any symbol exists;
//! `lowering` translates each syntax node into the symbol-resolved carrier;
//! `symbols` builds the symbol table and assigns identities; `selection`
//! binds what lexical lookup alone cannot settle; `constant` owns constant
//! declarations, substitution, and initializer custody across every phase.

mod constant;
mod lowering;
mod preparation;
mod resolution;
mod selection;
mod symbols;

pub use constant::initializer_dependencies::ConstInitializerDependencies;
pub use constant::requires_const_initializer_evaluation;
pub use preparation::generic_data::{
    canonicalize_declared_const_definition, closed_data_const_argument_expressions,
    closed_machine_const_arguments, normalize_generic_data,
    normalize_generic_data_with_retained_base,
    normalize_generic_data_with_sources_and_top_level_bindings,
};
pub use preparation::trait_defaults::synthesize_trait_defaults;
pub use resolution::{
    ConstInitializerSelection, ExtensionRequest, RebasedSeededSymbolResolvedTrees,
    ResolutionRequest, SeededSymbolResolvedTrees, prepare_const_initializer_selection, resolve,
    resolve_const_argument_selection, resolve_extension,
};

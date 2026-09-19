#![forbid(unsafe_code)]

//! Resolves names in parsed Omega source into stable Psi symbol identities.
//!
//! Start at `resolution.rs`: it is the route, one call per phase. The folders
//! are its owners. `preparation` rewrites syntax before any symbol exists;
//! `lowering` translates each syntax node into the symbol-resolved carrier;
//! `symbols` builds the symbol table and assigns identities; `selection`
//! binds what lexical lookup alone cannot settle; `constant` owns constant
//! declarations, substitution, and initializer custody across every phase.
//!
//! The stage has one entry, `resolve`, and one re-entry, `resolve_extension`,
//! for source a build generates after the base was resolved. `pre_resolution`
//! is the seam build-time evaluation drives before the stage can run: it
//! must resolve names to evaluate constants and evaluate constants to close
//! generic data. No later stage may use it.

mod constant;
mod lowering;
mod preparation;
mod resolution;
mod selection;
mod symbols;

pub use resolution::{
    ExtensionRequest, RebasedSeededSymbolResolvedTrees, ResolutionRequest,
    SeededSymbolResolvedTrees, resolve, resolve_extension,
};

/// What build-time evaluation drives before this stage can run.
///
/// Constants must be evaluated to close generic data, and names must be
/// resolved to evaluate constants, so that service runs restricted resolutions
/// and the syntax-to-syntax rewrites here between its evaluations, then hands
/// the forest to [`resolve`]. Nothing downstream of resolution may use this.
pub mod pre_resolution {
    pub use crate::constant::initializer_dependencies::ConstInitializerDependencies;
    pub use crate::constant::requires_const_initializer_evaluation;
    pub use crate::preparation::generic_data::{
        GenericDataRequest, canonicalize_declared_const_definition,
        closed_data_const_argument_expressions, closed_machine_const_arguments,
        normalize_generic_data,
    };
    pub use crate::preparation::trait_defaults::synthesize_trait_defaults;
    pub use crate::resolution::{
        ConstInitializerSelection, prepare_const_initializer_selection,
        resolve_const_argument_selection, resolve_numeric_probe,
    };
}

#![forbid(unsafe_code)]

//! Resolves names in parsed Omega source into stable Psi symbol identities.
//!
//! One entrance: [`resolve`], in `resolution.rs`. Read `drive` there to read
//! this stage — it is the route, one call per phase, and the module order
//! below is the order it calls them in. `resolution::lowerer` owns the
//! `Lowerer` state every phase borrows.
//!
//! `begin` first runs `preparation`, which rewrites syntax to syntax before
//! any symbol exists and validates machine equation declarations. `drive`
//! then runs the phases in the one order their preconditions allow, as its
//! own comment records: `lowering` translates items and then mathematical
//! definitions into the symbol-resolved carrier; `selection` binds operator
//! homes, which need no symbols yet; `symbols` builds the table and assigns
//! identities; `lowering` marks the domain homes of token-bearing machines,
//! which need those just-assigned attached symbols; then `constant` and
//! `selection` close in turn — constants need the table, the authored
//! selection ledger needs substituted constants, operator obligations need
//! the ledger, and every remaining selection needs all of it — before
//! `lowering` rejects duplicate direct token bindings by comparing settled
//! operand identities.
//!
//! `constant` is not one phase of that route. It owns constant declarations,
//! substitution and initializer custody across every phase, and `preparation`
//! and `lowering` reach it directly, not only through the route.
//!
//! There is one re-entry beside the entrance, [`resolve_extension`], for
//! source a build generates after the base was resolved, and one seam,
//! [`pre_resolution`], that build-time evaluation drives before the stage can
//! run: it must resolve names to evaluate constants and evaluate constants to
//! close generic data. No later stage may use it.

// The entrance and the route it drives.
mod resolution;

// The phases, in the order `drive` calls them.
mod lowering;
mod preparation;
mod selection;
mod symbols;

// Not a phase: reached from preparation, lowering and the route alike.
mod constant;

// The entrance, its post-base re-entry, and the requests and carriers both
// name.
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

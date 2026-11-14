/*
    Stage 02 resolves the names in parsed Omega source into stable Psi symbol
    identities.

    We have one entrance, `resolve`, in `resolution`; its `drive` is the
    route, one call per phase. `begin` first runs `preparation`, which rewrites
    syntax to syntax before any symbol exists: target families, type aliases
    and generic data normalize, closed scalar constant initializers fold to
    their values, trait defaults are synthesized, and machine equation
    declarations are validated. `drive` then runs the phases in the
    one order their preconditions allow: `lowering` translates items and
    mathematical definitions into the symbol-resolved carrier; `selection`
    binds operator homes, which need no symbols yet; `symbols` builds the
    table and assigns identities; `lowering` normalizes exact arithmetic
    intrinsics and marks the domain homes of token-bearing machines, which
    need those attached symbols; then `constant` and `selection` close in turn
    (constants need the table, the authored-selection ledger needs substituted
    constants, operator obligations need the ledger, and every remaining
    selection needs all of it) before `lowering` rejects duplicate direct
    token bindings by comparing settled operand identities.
*/

/*
    We keep one route with a fixed phase order because each phase needs facts
    only an earlier one can settle, and an explicit order is easier to audit
    than phases that discover each other. `constant` is deliberately not a
    phase: it owns constant declarations, substitution and initializer custody
    across every phase, and `preparation` and `lowering` reach it directly.
    Const value semantics end here: each selected use receives a fresh copy of
    its initializer, so no later stage grows a const-value concept. An
    initializer that needs no machine (a literal, an alias, or builtin integer
    arithmetic over literals and other such constants) is folded by
    `preparation` itself; one that would have to run a machine is not
    evaluated in this stage.

    Each phase lives in one file named after its module, and the larger
    sub-phases live beside it as `<phase>_<submodule>.rs`, attached with a
    `#[path]` attribute so the module tree, its privacy and every
    `crate::` path stay exactly as the phases name them.
*/

/*
    We consume the `SyntaxTrees` produced by stage 01
    (`tokens-to-syntax-trees`) and publish `SymbolResolvedTrees` together
    with the `SymbolTable` from the `symbols` foundation crate. Stage 03
    (`symbol-resolved-trees-to-typed-trees`) and `psi-compiler` consume that
    result. `resolve_extension` re-enters for source a build generates after
    the base was resolved, and `pre_resolution` is the one seam that
    build-time evaluation drives before this stage can run: it must resolve
    names to evaluate constants and evaluate constants to close generic data.
    No later stage may use it.
*/

#![forbid(unsafe_code)]

/* The entrance and the route it drives. */
mod resolution;

/* The phases, in the order `drive` calls them. */
mod lowering;
mod preparation;
mod selection;
mod symbols;

/* Not a phase: reached from preparation, lowering and the route alike. */
mod constant;

/* Read views onto the carrier's tables, reached from every phase. */
mod tables;

/*
    The entrance, its post-base re-entry, and the requests and carriers both
    name.
*/
pub use resolution::{
    DeferredResolvedExtension, DeferredResolvedFrontier, ExtensionRequest, ResolutionRequest,
    ResolvedBodyCompletion, resolve, resolve_deferred, resolve_deferred_extension,
    resolve_extension,
};

/*
    What build-time evaluation drives before this stage can run.

    Constants must be evaluated to close generic data, and names must be
    resolved to evaluate constants, so that service runs restricted resolutions
    and the syntax-to-syntax rewrites here between its evaluations, then hands
    the forest to [`resolve`]. Nothing downstream of resolution may use this.
*/
pub mod pre_resolution {
    pub use crate::constant::initializer_dependencies::{
        ConstInitializerDependencies, ResolvedMachineBodyDemand,
    };
    pub use crate::constant::initializer_normalization::InitializerCustodyPurpose;
    pub use crate::constant::requires_const_initializer_evaluation;
    pub use crate::preparation::declared_types::normalize_declared_types;
    pub use crate::preparation::generic_data::{
        ClosedConstArguments, ClosedDeclarationConstantUses, GenericDataRequest,
        canonicalize_declared_const_definition, closed_array_extent_expressions,
        closed_const_arguments, closed_data_const_argument_expressions,
        closed_declaration_constant_uses, closed_machine_const_arguments, normalize_generic_data,
        normalize_initializer_probe_data, retain_deferred_array_extent,
    };
    pub use crate::preparation::trait_defaults::synthesize_trait_defaults;
    pub use crate::resolution::{
        ConstArgumentSelection, ConstDestinationObligation, ConstInitializerPayload,
        ConstInitializerSelection, prepare_const_initializer_selection,
        prepare_const_qualification_selection, resolve_call_result_selection,
        resolve_const_argument_selection, resolve_deferred_initializer_probe,
        resolve_deferred_numeric_probe,
    };
}

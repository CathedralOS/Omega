//! Trait declarations and their applied requirement scopes. Inherited edges
//! retain substitutions over existing type nodes: projecting only a parent's
//! immediate arguments loses outer binders inside composite applications.

mod applications;

pub(crate) use applications::RetainedTraitApplicationFrame;
pub use applications::{
    AppliedTraitScope, AppliedTypeReference, GenericBoundRequirement, RetainedTraitScope,
    TraitApplicationArguments, TraitApplicationEnvironment,
};

use crate::name::Identifier;
use crate::signature::StateSignature;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    /// Source visibility retained independently from nominal and callable
    /// identity.
    pub is_public: bool,
    pub name: Identifier,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
    /// The `= Base` head of a transparent refinement. A refinement is a
    /// structural bound over an existing base conformance, never a nominal
    /// conformance target; `None` marks an ordinary trait.
    pub refines: Option<TraitRequirement>,
    /// `machine *` / `machine Base::requirement` narrowing clauses carrying
    /// operational axes only (`reaches`, `suspends`, `blocks`, `terminates`).
    pub refinement_clauses: Vec<TraitRefinementClause>,
}

/// One `machine *` or `machine Base::requirement` narrowing clause of a
/// transparent refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRefinementClause {
    /// The authored base requirement name; `None` for the `machine *`
    /// wildcard covering every base requirement.
    pub requirement: Option<Identifier>,
    /// Operational axes only.
    pub signature: StateSignature,
    /// Authored `reaches` names retained for the bound fit check's
    /// diagnostics; `_` members name the pending independent abstract row.
    pub service_reaches: Vec<Identifier>,
    /// How the clause's `reaches` axis binds, resolved at lowering.
    pub service_reach: TraitRefinementReach,
}

/// How one refinement clause's `reaches` axis binds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TraitRefinementReach {
    /// The clause omits `reaches`; covered requirements inherit the base row.
    #[default]
    Inherited,
    /// Authored `reaches a + b;` (or authored-empty `reaches;`): a concrete
    /// row interned at the clause location.
    Concrete(language_semantics::ServiceReachRowId),
    /// Authored `reaches _;`: one independent abstract row per covered base
    /// requirement, each bounded by that requirement's inherited row and
    /// correlated with no other requirement's row.
    IndependentBounded(Vec<ClauseAbstractReachRow>),
}

/// One covered base requirement's independent abstract reach row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseAbstractReachRow {
    /// The covered base requirement's name.
    pub requirement: Identifier,
    /// The abstract row minted at this clause location, bounded by the
    /// requirement's inherited row.
    pub row: language_semantics::ServiceReachRowId,
}

/// One named implementation: the checked identity and complete map from every
/// inherited requirement to exactly one realization row. It owns no data
/// subject; its trait application's types are ordinary arguments, and any
/// implementation of a runtime-capable trait may be selected dynamically.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conformance {
    pub symbol: SymbolHandle,
    /// Source visibility retained independently from semantic conformance
    /// identity and private realization rows.
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub trait_name: Identifier,
    /// Exact trait declaration selected by `trait_name`.
    pub trait_symbol: SymbolHandle,
    /// Alpha-normalized declaration-order ordinals selecting the conformance
    /// lifetime supplied to each target-trait lifetime parameter.
    pub trait_lifetime_arguments: Vec<u32>,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    pub rows: Vec<ConformanceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceRow {
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_name: Identifier,
    pub requirement: SymbolHandle,
    pub requirement_name: Identifier,
    /// Exact authored or per-conformance default realization. Closed frontend
    /// lowering instantiates trait defaults before this representation; an
    /// invalid survivor is an incomplete internal row and fails closed.
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_name: Identifier,
    pub source: ConformanceRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceRowSource {
    Inline,
    Reference,
    TraitDefault,
    /// A sealed core-trait row the compiler emitted as an ordinary checked
    /// member body (structural `Equatable<T>::equals`). An authored member or
    /// reference row suppresses it, so a block holds at most one per slot.
    Synthesized,
}

impl ConformanceRowSource {
    /// Whether the row's realization machine is a member of its own closed
    /// block rather than an independently declared machine the row references.
    pub fn owns_realization(self) -> bool {
        matches!(self, Self::Inline | Self::TraitDefault | Self::Synthesized)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Erased borrow-region arguments retained independently from runtime type
    /// arguments.
    pub lifetime_arguments: Vec<Identifier>,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Authored relationship location retained for declaration-site semantic
    /// diagnostics after source-backed names are lowered to owned text.
    pub source_span: source::SourceSpan,
}

/// Why a requirement is absent from the signature-derived portion of a local
/// dynamic trait surface. Later contract/lifetime/envelope judgments remain
/// independent and may exclude an otherwise signature-eligible requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
///
/// A selector packages no data, so no parameter position is privileged:
/// zero, one or several data parameters and an owned result are all eligible
/// (wiki/spec/terminal-psi/dynamic_dispatch.md, "Source eligibility").
pub enum DynamicSignatureIneligibility {
    BoundaryRequirement,
    RequirementLocalGenerics,
}

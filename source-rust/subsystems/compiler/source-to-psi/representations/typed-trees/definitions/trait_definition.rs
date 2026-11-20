//! Trait declarations, requirement scopes and selected conformance applications.

mod supply;
pub use supply::Supply;
pub(crate) mod applications;

pub(crate) use applications::RetainedTraitApplicationFrame;
pub use applications::{
    AppliedTraitScope, AppliedTypeReference, GenericBoundRequirement, RetainedTraitScope,
    TraitApplicationArguments, TraitApplicationEnvironment,
};

use crate::source_state::name::Identifier;
use crate::source_state::signature::StateSignature;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    pub is_public: bool,
    pub name: Identifier,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::source_state::data::TypeParameter>,
    pub conformance_bounds: Vec<crate::source_state::machine::GenericConformanceBound>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
    pub refines: Option<TraitRequirement>,
    pub refinement_clauses: Vec<TraitRefinementClause>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct TraitRefinementClause {
    pub requirement: Option<Identifier>,
    pub signature: StateSignature,
    pub service_reaches: Vec<Identifier>,
    pub service_reach: TraitRefinementReach,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub enum TraitRefinementReach {
    #[default]
    Inherited,
    Concrete(semantic_vocabulary::ServiceReachRowId),
    IndependentBounded(Vec<ClauseAbstractReachRow>),
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ClauseAbstractReachRow {
    pub requirement: Identifier,
    pub row: semantic_vocabulary::ServiceReachRowId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct Conformance {
    pub symbol: SymbolHandle,
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::source_state::data::TypeParameter>,
    pub trait_name: Identifier,
    pub trait_symbol: SymbolHandle,
    pub trait_lifetime_arguments: Vec<u32>,
    pub arguments: HandleSpan<crate::source_state::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    pub rows: Vec<ConformanceRow>,
    pub value_identity: ImplementationValueIdentity,
}

/*
    What distinguishes the value a named implementation constructs
    (conformances.md, "Resulting-value identity"): the schema application
    and the selected operations, never the constructor's name or a binder
    the schema application does not use. The default keeps the declaration
    as its own representative with every binder relevant, which is the
    answer for an implementation that owns a member: an inline or default
    body is a new machine over the whole telescope.
*/
#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct ImplementationValueIdentity {
    /* The first declaration constructing this same value; invalid means this one. */
    pub representative: SymbolHandle,
    /* One flag per binder: set when the schema application does not mention it. */
    pub unused_lifetimes: Vec<bool>,
    pub unused_parameters: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ConformanceRow {
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_name: Identifier,
    pub requirement: SymbolHandle,
    pub requirement_name: Identifier,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_name: Identifier,
    pub source: ConformanceRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum ConformanceRowSource {
    Inline,
    Reference,
    TraitDefault,
    Synthesized,
}

impl ConformanceRowSource {
    pub fn owns_realization(self) -> bool {
        matches!(self, Self::Inline | Self::TraitDefault | Self::Synthesized)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub lifetime_arguments: Vec<Identifier>,
    pub arguments: HandleSpan<crate::source_state::types::TypeReferenceHandle>,
    pub source_span: source_text::SourceSpan,
}

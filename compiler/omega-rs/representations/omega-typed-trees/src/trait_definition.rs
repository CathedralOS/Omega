use crate::name::Identifier;
use crate::signature::StateSignature;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    pub semantic_role: omega_core::semantics::TraitSemanticRole,
    pub name: Identifier,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub invariants: HandleSpan<crate::domain::ProofFact>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
}

impl Default for TraitDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            is_boundary: false,
            semantic_role: omega_core::semantics::TraitSemanticRole::Ordinary,
            name: Identifier::default(),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            invariants: HandleSpan::empty(),
            requires: HandleSpan::empty(),
            machines: HandleSpan::empty(),
        }
    }
}

/// A standalone conformance item (`Point satisfies Equatable;`, frozen
/// decision 8): a data type claims a whole, optionally generic trait;
/// validation checks its written/default attached machines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataConformance {
    pub type_name: Identifier,
    pub trait_name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Authored relationship location retained for declaration-site semantic
    /// diagnostics after source-backed names are lowered to owned text.
    pub source_span: omega_core::source::SourceSpan,
}

/// The semantic role of a trait-composition edge. It is derived from the
/// referenced trait, never authored: boundary parents extend service reach;
/// ordinary parents contribute policy/requirement identity only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraitCompositionKind {
    Policy,
    ServiceReach,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            arguments: HandleSpan::empty(),
            source_span: omega_core::source::SourceSpan::default(),
        }
    }
}

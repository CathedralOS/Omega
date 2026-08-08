use crate::name::Identifier;
use crate::signature::StateSignature;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    pub name: Identifier,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
    pub invariants: HandleSpan<crate::domain::ProofFact>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
}

impl Default for TraitDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            is_boundary: false,
            name: Identifier::default(),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            invariants: HandleSpan::empty(),
            requires: HandleSpan::empty(),
            machines: HandleSpan::empty(),
        }
    }
}

/// One whole nominal conformance. A closed implementation retains its exact
/// inherited requirement rows; the legacy bodyless form temporarily retains
/// attached-machine lookup until corpus migration removes that path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataConformance {
    pub symbol: SymbolHandle,
    pub type_name: Identifier,
    pub trait_name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    pub implementation: ConformanceImplementation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConformanceImplementation {
    #[default]
    LegacyAttachedMachines,
    Closed {
        rows: Vec<ConformanceRow>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceRow {
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_name: Identifier,
    pub requirement: SymbolHandle,
    pub requirement_name: Identifier,
    /// Exact authored realization. A selected trait-default template keeps
    /// these invalid until per-conformance default instantiation creates its
    /// checked machine; checked dynamic lowering rejects it meanwhile.
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Authored relationship location retained for declaration-site semantic
    /// diagnostics after source-backed names are lowered to owned text.
    pub source_span: psi_source::SourceSpan,
}

/// The semantic role of a trait-composition edge. It is derived from the
/// referenced trait, never authored: boundary parents extend service reach;
/// ordinary parents contribute policy/requirement identity only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraitCompositionKind {
    Policy,
    ServiceReach,
}

/// Why a requirement is absent from the signature-derived portion of a local
/// dynamic trait surface. Later contract/lifetime/envelope judgments remain
/// independent and may exclude an otherwise signature-eligible requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicSignatureIneligibility {
    BoundaryRequirement,
    RequirementLocalGenerics,
    MissingBorrowedReceiver,
    ByValueReceiver,
    MultipleReceivers,
    SelfOutsideReceiver,
    SelfResult,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            arguments: HandleSpan::empty(),
            source_span: psi_source::SourceSpan::default(),
        }
    }
}

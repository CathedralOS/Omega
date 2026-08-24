use crate::name::DiagnosticName;
use crate::signature::StateSignature;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    /// Source visibility retained independently from symbol and requirement
    /// identity.
    pub is_public: bool,
    pub name: DiagnosticName,
    pub storage: TraitStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitStorage {
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
    pub invariants: HandleSpan<crate::domain::ProofFact>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConformanceSubject {
    #[default]
    Subjectless,
    Carrier(DiagnosticName),
}

/// One whole conformance. Every closed implementation retains its exact
/// inherited requirement rows. Carrier-owned closed forms alone are eligible
/// for local dynamic dispatch; subjectless forms remain proof evidence. The
/// bodyless form remains a carrier-owned static declaration whose satisfiers
/// are validated separately.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conformance {
    pub symbol: SymbolHandle,
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub subject: ConformanceSubject,
    /// Exact carrier declaration for a carrier-owned conformance. Subjectless
    /// proof evidence retains the invalid symbol.
    pub carrier_symbol: SymbolHandle,
    pub trait_name: DiagnosticName,
    /// Exact trait declaration selected by `trait_name`.
    pub trait_symbol: SymbolHandle,
    pub arguments: HandleSpan<crate::types::TypeReference>,
    pub alias: Option<DiagnosticName>,
    pub implementation: ConformanceImplementation,
}

impl Conformance {
    pub fn carrier_name(&self) -> Option<&DiagnosticName> {
        match &self.subject {
            ConformanceSubject::Carrier(name) => Some(name),
            ConformanceSubject::Subjectless => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConformanceImplementation {
    #[default]
    AttachedRequirementMachines,
    Closed {
        rows: Vec<ConformanceRow>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceRow {
    /// Exact declaring trait after normalization. Inline short names begin
    /// with an invalid symbol and empty diagnostic name, then resolve against
    /// the inherited requirement closure before typed lowering.
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_name: DiagnosticName,
    pub requirement: SymbolHandle,
    pub requirement_name: DiagnosticName,
    /// Pre-normalization declaration ordinal used only by synthesized trait
    /// defaults. Exact requirement symbols replace it before typed lowering.
    pub provisional_requirement_ordinal: Option<usize>,
    /// Exact authored realization. A selected trait-default template keeps
    /// these invalid until per-conformance default instantiation creates its
    /// checked machine; checked dynamic lowering rejects it meanwhile.
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_name: DiagnosticName,
    /// Pre-normalization root-machine ordinal for inline/default members.
    /// This prevents same-named overloads from being re-selected by text.
    pub provisional_realization_ordinal: Option<usize>,
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
    pub name: DiagnosticName,
    /// Erased borrow-region arguments authored on a header parent. These are
    /// semantic relationship data even though they do not affect runtime
    /// generic identity.
    pub lifetime_arguments: Vec<DiagnosticName>,
    /// Generic arguments authored on a header parent (`Policy<C>`). Empty for
    /// the body-level `requires Policy;` form.
    pub arguments: HandleSpan<crate::types::TypeReference>,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            lifetime_arguments: Vec::new(),
            arguments: HandleSpan::empty(),
        }
    }
}

impl Deref for TraitDefinition {
    type Target = TraitStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for TraitDefinition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

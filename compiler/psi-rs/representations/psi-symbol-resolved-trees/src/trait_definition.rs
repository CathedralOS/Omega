use crate::name::DiagnosticName;
use crate::signature::StateSignature;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
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

/// One whole nominal conformance. A closed implementation retains its exact
/// inherited requirement rows; the legacy bodyless form temporarily retains
/// attached-machine lookup until corpus migration removes that path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataConformance {
    pub symbol: SymbolHandle,
    pub type_name: DiagnosticName,
    pub trait_name: DiagnosticName,
    pub arguments: HandleSpan<crate::types::TypeReference>,
    pub alias: Option<DiagnosticName>,
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
    /// Exact declaring trait after normalization. Inline short names begin
    /// with an invalid symbol and empty diagnostic name, then resolve against
    /// the inherited requirement closure before typed lowering.
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_name: DiagnosticName,
    pub requirement: SymbolHandle,
    pub requirement_name: DiagnosticName,
    /// Exact authored realization. A selected trait-default template keeps
    /// these invalid until per-conformance default instantiation creates its
    /// checked machine; checked dynamic lowering rejects it meanwhile.
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_name: DiagnosticName,
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
    /// Generic arguments authored on a header parent (`Policy<C>`). Empty for
    /// the body-level `requires Policy;` form.
    pub arguments: HandleSpan<crate::types::TypeReference>,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
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

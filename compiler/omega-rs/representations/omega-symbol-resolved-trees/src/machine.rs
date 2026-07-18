use crate::expression::ExpressionHandle;
use crate::name::DiagnosticName;
use crate::signature::SignatureContract;
use crate::state::State;
use crate::types::TypeReference;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub attached_data: Option<DiagnosticName>,
    pub boundary: bool,
    pub storage: MachineStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineStorage {
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub contains: HandleSpan<ContainedObject>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub termination_guarantee: omega_core::termination::TerminationGuarantee,
    pub ranking_witness: RankingWitness,
    pub effects: HandleSpan<DiagnosticName>,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<Handle<State>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RankingWitness {
    pub subjects: HandleSpan<ExpressionHandle>,
    pub view: HandleSpan<DiagnosticName>,
    pub view_arguments: HandleSpan<ExpressionHandle>,
    pub range: RankingRange,
}

impl RankingWitness {
    pub fn is_present(self) -> bool {
        !self.subjects.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RankingRange {
    pub start: ExpressionHandle,
    pub end: ExpressionHandle,
    pub end_inclusive: bool,
}

impl RankingRange {
    pub fn is_present(self) -> bool {
        self.start.is_valid() && self.end.is_valid()
    }
}

impl Deref for Machine {
    type Target = MachineStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for Machine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContainedObject {
    pub symbol: SymbolHandle,
    pub type_symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_name: DiagnosticName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: TypeReference,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    /// The single-requirement binding (`satisfies Trait::requirement`,
    /// rearrange settle 2026-07-18): `Some` conforms the machine to that one
    /// requirement instead of the whole trait; `alias` names the satisfier
    /// (`as Name`) for plural algebras / signature collisions.
    pub requirement: Option<DiagnosticName>,
    pub alias: Option<DiagnosticName>,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            requirement: None,
            alias: None,
        }
    }
}

impl Default for OwnedData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            type_reference: TypeReference::Unit,
            initial_value: ExpressionHandle::invalid(),
        }
    }
}

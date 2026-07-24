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
    /// STR3: the first-class supply mode (`boundary: bool` is the
    /// compatibility flag until STR7 retires it). Populated at the
    /// syntax->resolved lowering, copied -- never re-derived -- downstream.
    /// Requirement/Accepted gain their own sources when their spellings
    /// reach this record.
    pub supply_mode: omega_core::semantics::MachineSupplyMode,
    /// TPR2 (decision 23): the normalized termination plan -- the authored
    /// PUBLIC guarantee and the PRIVATE ranking witness as separate fields.
    /// Populated ONCE at the syntax->resolved lowering (bare `terminates;`
    /// -> published guarantee; `terminates by ...` -> witness subjects +
    /// explicit view, canonical defaults elaborated where the root-state
    /// parameter type determines them), copied -- never re-derived --
    /// downstream. `checked_summary` stays `NoGuarantee` until TPR3's
    /// migrated cycle checker establishes it. `terminates`/`decreases`/
    /// `decrease_order` in the storage below remain the compatibility
    /// shape the current checker consumes until TPR3/TPR6 retire them.
    pub termination_plan: omega_core::semantics::MachineTerminationPlan,
    /// EFX: normalized boundary-service row, populated after symbol
    /// assignment. Every member is a resolved boundary trait identity.
    pub service_reach_row: omega_core::semantics::ServiceReachRowId,
    pub storage: MachineStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineStorage {
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub terminates: bool,
    pub decreases: HandleSpan<ExpressionHandle>,
    pub decrease_order: HandleSpan<DiagnosticName>,
    /// TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    pub decrease_view_arguments: HandleSpan<ExpressionHandle>,
    /// TPR3: the optional `in <range>` rank constraint (a Range expression;
    /// invalid = absent). The checker verifies it structurally.
    pub decrease_range: ExpressionHandle,
    pub effects: HandleSpan<DiagnosticName>,
    /// Authored operational ceilings, copied independently from the service
    /// row compatibility span.
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<Handle<State>>,
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
    /// PRV4: the external leaf's NORMALIZED binding rendering (`via`).
    pub via: Option<String>,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            requirement: None,
            alias: None,
            via: None,
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

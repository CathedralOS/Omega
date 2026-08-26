use crate::expression::ExpressionHandle;
use crate::name::DiagnosticName;
use crate::signature::SignatureContract;
use crate::state::State;
use crate::types::TypeReference;
use psi_arena::{Handle, HandleSpan};
use psi_symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub attached_data: Option<DiagnosticName>,
    /// Exact nominal declaration named by `attached_data`. Spelling remains
    /// diagnostic only; semantic consumers must use this symbol.
    pub attached_data_symbol: SymbolHandle,
    /// Retained source-level package visibility. Public checked bodies publish
    /// strict effect and operational ceilings without changing supply mode.
    pub is_public: bool,
    /// Populated once at syntax-to-resolved lowering and copied downstream;
    /// semantic consumers must not reconstruct supply from source spelling or
    /// body presence.
    pub supply_mode: psi_language_semantics::MachineSupplyMode,
    /// Exact source-body presence retained independently from supply mode.
    /// Boundary supply admits both bodyless host declarations and checked
    /// adapters, so downstream review must not reconstruct this distinction
    /// from synthesized state rows.
    pub body_is_present: bool,
    /// TPR2 (decision 23): the normalized termination plan -- the authored
    /// PUBLIC guarantee and the PRIVATE ranking witness as separate fields.
    /// Populated ONCE at the syntax->resolved lowering (bare `terminates;`
    /// -> published guarantee; `terminates by ...` -> witness subjects +
    /// explicit view, canonical defaults elaborated where the root-state
    /// parameter type determines them), copied -- never re-derived --
    /// downstream. `checked_summary` stays `NoGuarantee` until the cycle
    /// checker establishes it. Ranking subjects remain private witness
    /// material in the storage below.
    pub termination_plan: psi_language_semantics::MachineTerminationPlan,
    /// EFX: normalized boundary-service row, populated after symbol
    /// assignment. Every member is a resolved boundary trait identity.
    pub service_reach_row: psi_language_semantics::ServiceReachRowId,
    /// The published row is an installation-selected upper bound rather than
    /// a fixed callable ceiling.
    pub service_reach_is_installation_bound: bool,
    pub storage: MachineStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineStorage {
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub ranking_subjects: HandleSpan<ExpressionHandle>,
    pub ranking_view: HandleSpan<DiagnosticName>,
    /// TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    pub ranking_view_arguments: HandleSpan<ExpressionHandle>,
    /// TPR3: the optional `in <range>` rank constraint (a Range expression;
    /// invalid = absent). The checker verifies it structurally.
    pub ranking_range: ExpressionHandle,
    pub invokes: HandleSpan<DiagnosticName>,
    /// Authored operational ceilings, independent from the service row.
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<Handle<State>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericConformanceBound {
    pub binder: Option<SymbolHandle>,
    pub binder_name: Option<DiagnosticName>,
    pub subject: SymbolHandle,
    pub subject_name: DiagnosticName,
    /// The ordinary trait symbol, or the data carrier symbol for a named
    /// conformance path.
    pub carrier: SymbolHandle,
    pub carrier_name: DiagnosticName,
    pub arguments: HandleSpan<TypeReference>,
    /// Exact selected declaration and its complete declaration-owned
    /// application telescope.
    pub selected_conformance: Option<crate::expression::StaticMachineArgument>,
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
    pub arguments: HandleSpan<crate::types::TypeReference>,
    /// The exact requirement binding (`satisfies Trait::requirement`). Source
    /// lowering always supplies `Some`; `alias` names the satisfier
    /// (`as Name`) for plural algebras / signature collisions.
    pub requirement: Option<DiagnosticName>,
    pub alias: Option<DiagnosticName>,
    /// Exact interned identity of the external leaf's structured `via`
    /// binding. Absence means this is an ordinary checked satisfier.
    pub external_binding: Option<psi_language_semantics::ExternalBindingId>,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            arguments: HandleSpan::empty(),
            requirement: None,
            alias: None,
            external_binding: None,
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

use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::signature::SignatureContract;
use crate::state::State;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    /// Copied from symbol-resolved trees; semantic consumers must not
    /// reconstruct supply from source spelling or body presence.
    pub supply_mode: psi_language_semantics::MachineSupplyMode,
    /// TPR2 (decision 23): the normalized termination plan (published
    /// guarantee vs private ranking witness), populated ONCE at the
    /// syntax->resolved lowering and COPIED here -- never re-derived. Ranking
    /// subjects below are implementation witnesses, not compatibility
    /// contract flags.
    pub termination_plan: psi_language_semantics::MachineTerminationPlan,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: psi_language_semantics::ServiceReachRowId,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub decreases: HandleSpan<ExpressionHandle>,
    pub decrease_order: HandleSpan<Identifier>,
    /// TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    pub decrease_view_arguments: HandleSpan<ExpressionHandle>,
    /// TPR3: the optional `in <range>` rank constraint (a Range expression;
    /// invalid = absent). The checker verifies it structurally.
    pub decrease_range: ExpressionHandle,
    pub invokes: HandleSpan<Identifier>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<State>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            supply_mode: psi_language_semantics::MachineSupplyMode::CheckedBody,
            termination_plan: psi_language_semantics::MachineTerminationPlan::default(),
            service_reach_row: psi_language_semantics::ServiceReachRowId::NULL,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            decrease_view_arguments: HandleSpan::empty(),
            decrease_range: ExpressionHandle::invalid(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericConformanceBound {
    pub subject: SymbolHandle,
    pub subject_name: Identifier,
    pub carrier: SymbolHandle,
    pub carrier_name: Identifier,
    pub arguments: Vec<TypeReferenceHandle>,
    pub conformance: Option<SymbolHandle>,
    pub conformance_name: Option<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// The single-requirement binding (`satisfies Trait::requirement`,
    /// rearrange settle 2026-07-18): `Some` conforms the machine to that one
    /// requirement instead of the whole trait; `alias` names the satisfier
    /// (`as Name`) for plural algebras / signature collisions.
    pub requirement: Option<Identifier>,
    pub alias: Option<Identifier>,
    /// PRV4: the external leaf's NORMALIZED binding rendering (`via`).
    pub via: Option<String>,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            arguments: HandleSpan::empty(),
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
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
            initial_value: ExpressionHandle::invalid(),
        }
    }
}

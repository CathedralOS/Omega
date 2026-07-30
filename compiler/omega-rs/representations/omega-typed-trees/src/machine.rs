use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::signature::SignatureContract;
use crate::state::State;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub boundary: bool,
    /// STR3: the first-class supply mode (`boundary: bool` is the
    /// compatibility flag until STR7 retires it). Populated at the
    /// syntax->resolved lowering, copied -- never re-derived -- downstream.
    /// Requirement/Accepted gain their own sources when their spellings
    /// reach this record.
    pub supply_mode: omega_core::semantics::MachineSupplyMode,
    /// TPR2 (decision 23): the normalized termination plan (published
    /// guarantee vs private ranking witness), populated ONCE at the
    /// syntax->resolved lowering and COPIED here -- never re-derived. Ranking
    /// subjects below are implementation witnesses, not compatibility
    /// contract flags.
    pub termination_plan: omega_core::semantics::MachineTerminationPlan,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: omega_core::semantics::ServiceReachRowId,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub decreases: HandleSpan<ExpressionHandle>,
    pub decrease_order: HandleSpan<Identifier>,
    /// TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    pub decrease_view_arguments: HandleSpan<ExpressionHandle>,
    /// TPR3: the optional `in <range>` rank constraint (a Range expression;
    /// invalid = absent). The checker verifies it structurally.
    pub decrease_range: ExpressionHandle,
    pub service_reaches: HandleSpan<Identifier>,
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
            boundary: false,
            supply_mode: omega_core::semantics::MachineSupplyMode::CheckedBody,
            termination_plan: omega_core::semantics::MachineTerminationPlan::default(),
            service_reach_row: omega_core::semantics::ServiceReachRowId::NULL,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            decrease_view_arguments: HandleSpan::empty(),
            decrease_range: ExpressionHandle::invalid(),
            service_reaches: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
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
    pub semantic_role: omega_core::semantics::TraitConformanceSemanticRole,
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
            semantic_role: omega_core::semantics::TraitConformanceSemanticRole::Ordinary,
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

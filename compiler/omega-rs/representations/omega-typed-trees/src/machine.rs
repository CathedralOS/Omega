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
    /// syntax->resolved lowering and COPIED here -- never re-derived. The
    /// `terminates`/`decreases`/`decrease_order` fields below remain the
    /// compatibility shape the current cycle checker consumes until TPR3
    /// migrates it onto this plan.
    pub termination_plan: omega_core::semantics::MachineTerminationPlan,
    /// STR4 (decision 22): the normalized effect-row identity (copied,
    /// never re-derived; indexes the tree's `effect_rows` table).
    pub effect_row: omega_core::semantics::EffectRowId,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: omega_core::semantics::ServiceReachRowId,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub contains: HandleSpan<ContainedObject>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub terminates: bool,
    pub decreases: HandleSpan<ExpressionHandle>,
    pub decrease_order: HandleSpan<Identifier>,
    /// TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    pub decrease_view_arguments: HandleSpan<ExpressionHandle>,
    /// TPR3: the optional `in <range>` rank constraint (a Range expression;
    /// invalid = absent). The checker verifies it structurally.
    pub decrease_range: ExpressionHandle,
    pub effects: HandleSpan<Identifier>,
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
            effect_row: omega_core::semantics::EffectRowId::NULL,
            service_reach_row: omega_core::semantics::ServiceReachRowId::NULL,
            type_parameters: HandleSpan::empty(),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            terminates: false,
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            decrease_view_arguments: HandleSpan::empty(),
            decrease_range: ExpressionHandle::invalid(),
            effects: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub symbol: SymbolHandle,
    pub type_symbol: SymbolHandle,
    pub name: Identifier,
    pub type_name: Identifier,
}

impl Default for ContainedObject {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            type_symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_name: Identifier::default(),
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

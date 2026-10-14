//! Exact replay of derived and selected provider provenance.
//!
//! This file owns the provenance carriers and the derivation entry points.
//! `plan_derivation.rs` derives satisfies plans with provenance,
//! `requirement_identities.rs` names satisfied requirements and their
//! bindings, `adapter_conformance.rs` resolves checked adapters and their
//! invocations, and `selection_replay.rs` replays selected provenance for
//! package review. Derivation is the one producer of a plan and its
//! provenance: the compile does not validate the candidates it just derived
//! (pipeline.md, "Verification placement"), and adapter refinement of a
//! requirement's service and invocation ceilings is Psi conformance checking.

mod adapter_conformance;
mod plan_derivation;
mod requirement_identities;
mod specialization_rows;

pub(crate) use adapter_conformance::{
    exact_authored_invocations, exact_checked_adapter_invocations, exact_row_for_schema_method,
};
pub use adapter_conformance::{exact_checked_adapter, selected_requirement_checked_adapter};
pub(crate) use requirement_identities::exact_satisfied_requirement_identity;
pub use requirement_identities::{same_semantic_name, satisfies_plan_name};
pub(crate) use specialization_rows::derive_selected_requirement_specializations;

use super::ProviderPlan;
use crate::ProviderPlanningContext;
use crate::provider_planning::provenance_replay::plan_derivation::derive_provider_plans;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaDeclaration {
    BoundaryTrait(symbols::SymbolHandle),
    BoundaryRequirement(symbols::SymbolHandle),
    BoundaryOperator(symbols::SymbolHandle),
    SealedPlacedAccessor(symbols::SymbolHandle),
}

impl ProviderSchemaDeclaration {
    pub const fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::BoundaryTrait(symbol)
            | Self::BoundaryRequirement(symbol)
            | Self::BoundaryOperator(symbol)
            | Self::SealedPlacedAccessor(symbol) => symbol,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanProvenance {
    pub schema: ProviderSchemaDeclaration,
    pub provider_type: Option<symbols::SymbolHandle>,
    pub row_requirements: Vec<symbols::SymbolHandle>,
    pub row_realizations: Vec<symbols::SymbolHandle>,
    /// Exact pre-resolution target-scoped declaration custody for catalog-
    /// inferred rows. `None` is mandatory for ordinary checked, legacy
    /// external, and evaluated-payload rows.
    pub row_target_machine_origins: Vec<Option<SelectedTargetMachineOrigin>>,
    /// Compiler-catalog custody is distinct from authored target-machine origins.
    pub row_placed_accessors: Vec<Option<crate::SealedPlacedAccessor>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTargetMachineOrigin {
    pub machine: symbols::SymbolHandle,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedProviderPlan {
    pub plan: ProviderPlan,
    pub provenance: ProviderPlanProvenance,
}

/// One provider-plan derivation request: the selected target, and when the
/// program's `via` bindings were evaluated, that table with the target-machine
/// origins it selected. Build the evaluated form through
/// [`ProviderPlanDerivation::evaluated`] with the table this compile just
/// evaluated for the same typed program and target.
#[derive(Clone, Copy)]
pub struct ProviderPlanDerivation<'a> {
    context: ProviderPlanningContext<'a>,
    calling_plans: &'a [crate::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_target: Option<&'a str>,
    evaluated_bindings: Option<&'a crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    target_machine_origins: &'a [SelectedTargetMachineOrigin],
}

impl<'a> ProviderPlanDerivation<'a> {
    /// Derive from retained conformance alone, before `via` bindings are
    /// evaluated.
    pub const fn unevaluated(
        context: ProviderPlanningContext<'a>,
        calling_plans: &'a [crate::calling_policy_plans::BoundaryCallingPlanRealization],
        selected_target: Option<&'a str>,
    ) -> Self {
        Self {
            context,
            calling_plans,
            selected_target,
            evaluated_bindings: None,
            target_machine_origins: &[],
        }
    }

    /// Derive with the evaluated `via` binding table this compile produced for
    /// the same typed program and target. The table is not replayed against
    /// the program it was just evaluated from.
    pub fn evaluated(
        context: ProviderPlanningContext<'a>,
        calling_plans: &'a [crate::calling_policy_plans::BoundaryCallingPlanRealization],
        selected_target: Option<&'a str>,
        evaluated_bindings: &'a crate::evaluated_via_bindings::EvaluatedViaBindingTable,
        target_machine_origins: &'a [SelectedTargetMachineOrigin],
    ) -> Self {
        Self {
            context,
            calling_plans,
            selected_target,
            evaluated_bindings: Some(evaluated_bindings),
            target_machine_origins,
        }
    }
}

/// Derive every provider plan the typed program's retained conformance
/// satisfies, each with its provenance. This is the one derivation entry.
pub fn derive_satisfies_plans(
    derivation: ProviderPlanDerivation<'_>,
) -> Result<Vec<DerivedProviderPlan>, Vec<diagnostics::Diagnostic>> {
    derive_provider_plans(
        derivation.context,
        derivation.calling_plans,
        derivation.selected_target,
        derivation.evaluated_bindings,
        derivation.target_machine_origins,
    )
}

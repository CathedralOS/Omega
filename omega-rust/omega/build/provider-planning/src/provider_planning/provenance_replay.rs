//! Exact replay of derived and selected provider provenance.
//!
//! This file owns the provenance carriers and the derivation entry points.
//! `plan_derivation.rs` derives satisfies plans with provenance,
//! `requirement_identities.rs` names satisfied requirements and their
//! bindings, `adapter_conformance.rs` proves checked adapters conform to
//! their schemas and `candidate_validation.rs` validates provider-plan
//! candidates against replayed provenance.

mod adapter_conformance;
mod candidate_validation;
mod plan_derivation;
mod requirement_identities;

#[cfg(test)]
pub(crate) use adapter_conformance::exact_canonical_provider_schema;
pub use adapter_conformance::exact_checked_adapter;
pub(crate) use adapter_conformance::{
    exact_authored_invocations, exact_checked_adapter_invocations, exact_row_for_schema_method,
};
pub(crate) use candidate_validation::validate_derived_provider_plan_provenance;
pub use candidate_validation::{
    validate_derived_provider_plan_candidates, validate_provider_plan_candidates,
};
#[cfg(feature = "installed-writer")]
pub(crate) use requirement_identities::same_semantic_name;
pub use requirement_identities::{satisfied_requirement_identity, satisfies_plan_name};

use super::{ProviderPlan, TypedTrees};
use crate::provider_planning::provenance_replay::plan_derivation::derive_provider_plans;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaDeclaration {
    BoundaryTrait(symbols::SymbolHandle),
    BoundaryRequirement(symbols::SymbolHandle),
    BoundaryOperator(symbols::SymbolHandle),
}

impl ProviderSchemaDeclaration {
    pub const fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::BoundaryTrait(symbol)
            | Self::BoundaryRequirement(symbol)
            | Self::BoundaryOperator(symbol) => symbol,
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
/// [`ProviderPlanDerivation::evaluated`], which validates the table against
/// the typed program and the target before any plan is derived.
#[derive(Clone, Copy)]
pub struct ProviderPlanDerivation<'a> {
    pub selected_target: Option<&'a str>,
    pub evaluated_bindings: Option<&'a crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    pub target_machine_origins: &'a [SelectedTargetMachineOrigin],
}

impl<'a> ProviderPlanDerivation<'a> {
    /// Derive from retained conformance alone, before `via` bindings are
    /// evaluated.
    pub const fn unevaluated(selected_target: Option<&'a str>) -> Self {
        Self {
            selected_target,
            evaluated_bindings: None,
            target_machine_origins: &[],
        }
    }

    /// Derive with an evaluated `via` binding table, which must belong to
    /// this typed program and name the same target.
    pub fn evaluated(
        typed: &TypedTrees,
        selected_target: Option<&'a str>,
        evaluated_bindings: &'a crate::evaluated_via_bindings::EvaluatedViaBindingTable,
        target_machine_origins: &'a [SelectedTargetMachineOrigin],
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        evaluated_bindings.validate_against_typed(typed)?;
        let retained_target = evaluated_bindings
            .target()
            .map(target::TargetProfile::target_name);
        if retained_target != selected_target {
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "evaluated `via` binding table target `{}` does not match provider derivation target `{}`",
                retained_target.unwrap_or("<none>"),
                selected_target.unwrap_or("<none>"),
            ))]);
        }
        Ok(Self {
            selected_target,
            evaluated_bindings: Some(evaluated_bindings),
            target_machine_origins,
        })
    }
}

/// Derive every provider plan the typed program's retained conformance
/// satisfies, each with its provenance. This is the one derivation entry.
pub fn derive_satisfies_plans(
    typed: &TypedTrees,
    derivation: ProviderPlanDerivation<'_>,
) -> Vec<DerivedProviderPlan> {
    derive_provider_plans(
        typed,
        derivation.selected_target,
        derivation.evaluated_bindings,
        derivation.target_machine_origins,
    )
}

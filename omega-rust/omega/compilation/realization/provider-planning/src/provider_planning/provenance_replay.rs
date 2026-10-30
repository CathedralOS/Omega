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
mod named_provider_plans;
mod plan_derivation;
mod requirement_identities;
mod specialization_rows;

pub(crate) use adapter_conformance::exact_checked_adapter_invocations;
pub use adapter_conformance::{exact_checked_adapter, selected_requirement_checked_adapter};
pub use requirement_identities::{same_semantic_name, satisfies_plan_name};
pub use specialization_rows::project_checked_provider_instances;

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

/// A row's supplying owner. Native Build rows own an evaluated intent, never
/// an invented bodyless machine; toolchain settlement likewise has no source body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRowRealization {
    Authored(symbols::SymbolHandle),
    BuildNative(std::sync::Arc<crate::native_bindings::NativeBuildRowOrigin>),
    Toolchain,
}
impl ProviderRowRealization {
    pub const fn authored(&self) -> Option<symbols::SymbolHandle> {
        match self {
            Self::Authored(symbol) => Some(*symbol),
            _ => None,
        }
    }
    pub fn native(&self) -> Option<&crate::native_bindings::NativeBuildRowOrigin> {
        match self {
            Self::BuildNative(origin) => Some(origin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanProvenance {
    pub schema: ProviderSchemaDeclaration,
    pub provider_type: Option<symbols::SymbolHandle>,
    /// Original named application, before selected rows lose their source scope.
    pub named_application: Option<checked_trees::provider_declarations::NamedProviderApplication>,
    pub row_requirements: Vec<symbols::SymbolHandle>,
    pub row_realizations: Vec<ProviderRowRealization>,
    /// Exact pre-resolution target-scoped declaration custody for catalog-
    /// inferred rows. `None` is mandatory for checked and Build-native rows.
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

/// One derivation of source-owned candidate intent. The caller lends exact
/// target declaration origins and admitted Build-native rows; this operation
/// neither evaluates source nor reconstructs a forwarding machine.
#[derive(Clone, Copy)]
pub struct ProviderPlanDerivation<'a> {
    context: ProviderPlanningContext<'a>,
    calling_plans: &'a [crate::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_target: Option<&'a str>,
    target_machine_origins: &'a [SelectedTargetMachineOrigin],
}
impl<'a> ProviderPlanDerivation<'a> {
    pub const fn new(
        context: ProviderPlanningContext<'a>,
        calling_plans: &'a [crate::calling_policy_plans::BoundaryCallingPlanRealization],
        selected_target: Option<&'a str>,
        target_machine_origins: &'a [SelectedTargetMachineOrigin],
    ) -> Self {
        Self {
            context,
            calling_plans,
            selected_target,
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
        derivation.target_machine_origins,
    )
}

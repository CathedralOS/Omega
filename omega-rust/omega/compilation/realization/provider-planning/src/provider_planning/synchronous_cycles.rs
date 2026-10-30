//! Source-checking projection into the shared whole-provider invocation graph.

use crate::provider_planning::provenance_replay::exact_checked_adapter_invocations;
pub(crate) use backend_plan::synchronous_invocations::SynchronousBoundaryTarget;
use checked_trees::typed_trees::TypedTrees;

pub(crate) fn validate_selected_synchronous_invocation_cycles(
    typed: &TypedTrees,
    invocations: &checked_trees::SynchronousInvocationFacts,
    selected_plans: &[effects::provider_plan::ProviderPlan],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    backend_plan::synchronous_invocations::validate_selected_synchronous_invocation_cycles(
        selected_plans,
        |plan, row| exact_checked_adapter_invocations(context, invocations, plan, row),
    )
}

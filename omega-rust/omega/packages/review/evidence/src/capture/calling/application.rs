//! Rejoin checked source custody before discarding realization-only receipts.

mod native;
mod signature;

use super::{callbacks, opaque};
use crate::capture::representation::physical_contract::{
    project_boundary_shape_graph, project_representation_target,
};
use crate::record::{PackagePolicyCallingPlan, PackagePolicyPhysicalCallingContract};
use omega_compiler::CheckedCompilation;
use omega_provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use psi_diagnostics::Diagnostic;

/// Capture the complete published calling application. This is inert policy,
/// not a validator certificate and not a replacement for fresh compilation.
pub fn project_checked_calling_policy(
    compilation: &CheckedCompilation,
    realization: &BoundaryCallingPlanRealization,
) -> Result<PackagePolicyCallingPlan, Vec<Diagnostic>> {
    let candidates = compilation
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|candidate| {
            candidate.boundary_trait == realization.boundary_trait
                && candidate.boundary_arguments == realization.boundary_arguments
                && candidate.requirement_machine == realization.requirement_machine
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() || candidates.iter().any(|candidate| *candidate != realization) {
        return Err(rejected(
            "missing, ambiguous, or changed checked realization",
        ));
    }
    let materialized = realization.materialized_signature();
    if realization.native_parameters != materialized.native_parameters()
        || realization.callback_binders != materialized.callback_binders()
        || realization.callback_demands != materialized.callback_demands()
        || (!realization.callback_context_closed
            && (!materialized.callback_binders().is_empty()
                || !materialized.callback_demands().is_empty()
                || !materialized.callback_layout_catalog().is_empty()
                || !materialized.direct_callback_parameters().is_empty()))
        || &realization.boundary_entry_plan != realization.exact_boundary_entry_plan()
        || compilation.selected_native_target() != Some(materialized.native_target())
    {
        return Err(rejected(
            "detached signature, callback context, plan, or target",
        ));
    }
    let (validated, report, commitment) = realization
        .replayed_validated_application()
        .map_err(|reason| rejected(&format!("invalid calling application: {reason}")))?;
    if report == 0
        || report != realization.report_fingerprint
        || commitment != realization.commitment
    {
        return Err(rejected("stale calling application custody"));
    }
    let semantic = signature::project(compilation, realization)?;
    let callbacks = callbacks::project_callback_policy(
        compilation,
        materialized,
        &validated,
        &semantic.lifetime_binders,
    )?;
    let native_parameters =
        native::project(materialized, &semantic.semantic_parameters, &callbacks)?;
    let opaque_uses = opaque::project(compilation, realization, &validated)?;
    let policy = PackagePolicyCallingPlan {
        boundary_trait: semantic.boundary_trait,
        boundary_arguments: semantic.boundary_arguments,
        boundary_lifetime_parameter_count: semantic.boundary_lifetime_parameter_count,
        requirement: semantic.requirement,
        requirement_trait: semantic.requirement_trait,
        requirement_arguments: semantic.requirement_arguments,
        requirement_lifetime_arguments: semantic.requirement_lifetime_arguments,
        requirement_lifetime_parameter_count: semantic.requirement_lifetime_parameter_count,
        static_parameters: semantic.static_parameters,
        target: project_representation_target(compilation)?,
        shape_graph: project_boundary_shape_graph(materialized),
        semantic_parameters: semantic.semantic_parameters,
        semantic_result: semantic.semantic_result,
        native_parameters,
        callbacks,
        opaque_uses,
        physical: PackagePolicyPhysicalCallingContract::from_validated_plan(&validated),
    };
    policy.validate_canonical_structure().map_err(rejected)?;
    Ok(policy)
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "calling policy rejects {reason}"
    ))]
}

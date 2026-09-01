#![forbid(unsafe_code)]

//! Checked-Psi dispatch settlement for exact build-selected realizations.
//!
//! The compiler coordinates these rewrites after checking. This crate owns
//! their semantics and atomic plan/apply behavior.

mod adapter;
mod compiler_intrinsic;
mod float_intrinsic;
mod intrinsic_review;
mod operator_adapter;

pub use adapter::settle_selected_boundary_adapter_dispatch;
pub use compiler_intrinsic::{
    derive_selected_compiler_intrinsic_execution_identity_for_row,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding,
};
pub use float_intrinsic::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
    settle_selected_float_intrinsic_dispatch,
};
pub use intrinsic_review::{
    ResolvedAcceptedSemanticBinding, resolve_accepted_service_binding,
    retain_selected_compiler_intrinsic_review_identities,
};
pub use operator_adapter::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    CheckedSpecializedOperatorApplicationRealization,
    derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
    settle_selected_operator_adapter_dispatch, validate_selected_operator_terminal_custody,
};

/// Settle checked-body adapters and compiler-intrinsic float execution in one
/// atomic Unit-plan rebuild. Separate rebuilds would make the later family
/// erase applications retained by the earlier one.
pub fn settle_selected_execution_dispatch(
    checked: &mut std::sync::Arc<psi_checked_trees::CheckedTrees>,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let operator_rewrites = operator_adapter::plan_selected_operator_adapter_rewrites(
        checked,
        selected_provider_plans,
    )?;
    let float_rewrites =
        float_intrinsic::plan_selected_float_intrinsic_rewrites(checked, selected_provider_plans)?;
    if operator_rewrites.is_empty() && float_rewrites.is_empty() {
        return Ok(());
    }

    let operator_applications =
        operator_adapter::selected_unit_applications(checked, &operator_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
    let fma_applications =
        float_intrinsic::selected_ieee_float_fma_unit_applications(checked, &float_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
    let mut staged = checked.as_ref().clone();
    if !operator_applications.is_empty() || !fma_applications.is_empty() {
        psi_typed_trees_to_checked_trees::rebuild_checked_unit_effect_plans_with_selected_execution(
            &mut staged,
            &operator_applications,
            &fma_applications,
        );
        operator_adapter::validate_selected_unit_applications(&staged, &operator_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
        float_intrinsic::validate_selected_ieee_float_fma_unit_applications(
            &staged,
            &fma_applications,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    }
    operator_adapter::apply_selected_operator_adapter_rewrites(&mut staged, &operator_rewrites);
    float_intrinsic::apply_selected_float_intrinsic_rewrites(&mut staged, float_rewrites);
    *std::sync::Arc::make_mut(checked) = staged;
    Ok(())
}

#![forbid(unsafe_code)]

//! Checked-Psi dispatch settlement for exact build-selected realizations.
//!
//! The compiler coordinates these rewrites after checking. This crate owns
//! their semantics and atomic plan/apply behavior.

mod adapter;
mod compiler_intrinsic;
mod float_comparisons;
mod float_intrinsic;
mod intrinsic_review;
mod operator_adapter;
mod service_custody;
mod source_edits;

pub use adapter::settle_selected_boundary_adapter_dispatch;
pub use adapter::settle_selected_boundary_adapter_dispatch_with_source_edits;
pub use compiler_intrinsic::{
    derive_selected_compiler_intrinsic_execution_identity_for_row,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding,
};
pub use float_intrinsic::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution, settle_selected_float_intrinsic_dispatch,
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
pub use service_custody::{
    derive_fused_program_entry_establishments, validate_fused_service_terminal_custody,
};
pub use source_edits::SelectedDispatchSourceEdits;

/// Settle checked-body adapters and compiler-intrinsic float execution in one
/// atomic Unit-plan rebuild. Separate rebuilds would make the later family
/// erase applications retained by the earlier one.
/// This transformation-only entrance does not retain source-query custody.
/// Compiler publication uses the corresponding `with_source_edits` entrance.
pub fn settle_selected_execution_dispatch(
    checked: &mut std::sync::Arc<checked_trees::CheckedTrees>,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    settle_execution(
        checked,
        selected_provider_plans,
        source_edits::SourceEditBuilder::ignored(),
    )
    .map(|_| ())
}

/// Atomically settle execution and seal the replaced source graph for later
/// source-semantic queries. Failure publishes neither rewrites nor a journal.
pub fn settle_selected_execution_dispatch_with_source_edits(
    checked: &mut std::sync::Arc<checked_trees::CheckedTrees>,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<SelectedDispatchSourceEdits, Vec<diagnostics::Diagnostic>> {
    settle_execution(
        checked,
        selected_provider_plans,
        source_edits::SourceEditBuilder::default(),
    )
}

fn settle_execution(
    checked: &mut std::sync::Arc<checked_trees::CheckedTrees>,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    mut source_edits: source_edits::SourceEditBuilder,
) -> Result<SelectedDispatchSourceEdits, Vec<diagnostics::Diagnostic>> {
    let operator_rewrites = operator_adapter::plan_selected_operator_adapter_rewrites(
        checked,
        selected_provider_plans,
    )?;
    let float_rewrites =
        float_intrinsic::plan_selected_float_intrinsic_rewrites(checked, selected_provider_plans)?;
    if operator_rewrites.is_empty() && float_rewrites.is_empty() {
        let executions =
            float_comparisons::selected_executions(checked, selected_provider_plans.plans())?;
        if !checked
            .facts
            .operators
            .selected_float_comparisons
            .iter()
            .map(|(_, execution)| execution)
            .eq(executions.iter())
        {
            // All fallible derivation finished above; publishing these records
            // needs no second whole-program scratch copy.
            float_comparisons::replace_executions(std::sync::Arc::make_mut(checked), executions);
        }
        return Ok(SelectedDispatchSourceEdits::default());
    }

    let operator_applications =
        operator_adapter::selected_operator_applications(checked, &operator_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
    let fma_applications =
        float_intrinsic::selected_ieee_float_fma_unit_applications(checked, &float_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
    let mut staged = checked.as_ref().clone();
    if !operator_applications.is_empty() || !fma_applications.is_empty() {
        typed_trees_to_checked_trees::rebuild_checked_terminal_plans_with_selected_execution(
            &mut staged,
            &operator_applications,
            &fma_applications,
        )?;
        operator_adapter::validate_selected_unit_applications(&staged, &operator_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
        float_intrinsic::validate_selected_ieee_float_fma_unit_applications(
            &staged,
            &fma_applications,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    }
    operator_adapter::apply_selected_operator_adapter_rewrites(
        &mut staged,
        &operator_rewrites,
        &mut source_edits,
    );
    float_intrinsic::apply_selected_float_intrinsic_rewrites(
        &mut staged,
        float_rewrites,
        &mut source_edits,
    );
    // Rebuilds may refresh checked occurrence handles. Derive execution
    // custody from the final staged program, without replacing Match syntax.
    let comparisons =
        float_comparisons::selected_executions(&staged, selected_provider_plans.plans())?;
    float_comparisons::replace_executions(&mut staged, comparisons);
    let source_edits = source_edits.finish(&staged.typed)?;
    *std::sync::Arc::make_mut(checked) = staged;
    Ok(source_edits)
}

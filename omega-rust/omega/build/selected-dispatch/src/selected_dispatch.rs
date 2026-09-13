//! Plan selected operator and float execution, validate one staged rebuild,
//! then publish the checked program and its optional source-query journal.
//!
//! This is the atomic execution-settlement owner. Boundary call associations,
//! intrinsic review, and later Terminal custody checks have separate owners.

use std::sync::Arc;

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;

use crate::source_edits::{self, SelectedDispatchSourceEdits};

mod float_comparisons;
mod float_intrinsic;
mod operator_adapter;
#[cfg(test)]
mod tests;

pub use float_intrinsic::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution, settle_selected_float_intrinsic_dispatch,
};
pub use operator_adapter::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    CheckedSpecializedOperatorApplicationRealization,
    derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
    validate_selected_operator_terminal_custody,
};

/// Settle checked-body adapters and compiler-intrinsic float execution in one
/// atomic Unit-plan rebuild. Separate rebuilds would make the later family
/// erase applications retained by the earlier one.
/// This transformation-only entrance does not retain source-query custody.
/// Compiler publication uses the corresponding `with_source_edits` entrance.
pub fn settle_selected_execution_dispatch(
    checked: &mut Arc<CheckedTrees>,
    selected_provider_plans: &SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
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
    checked: &mut Arc<CheckedTrees>,
    selected_provider_plans: &SelectedProviderPlanFacts,
) -> Result<SelectedDispatchSourceEdits, Vec<Diagnostic>> {
    settle_execution(
        checked,
        selected_provider_plans,
        source_edits::SourceEditBuilder::default(),
    )
}

fn settle_execution(
    checked: &mut Arc<CheckedTrees>,
    selected_provider_plans: &SelectedProviderPlanFacts,
    mut source_edits: source_edits::SourceEditBuilder,
) -> Result<SelectedDispatchSourceEdits, Vec<Diagnostic>> {
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
            float_comparisons::replace_executions(Arc::make_mut(checked), executions);
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
    *checked = Arc::new(staged);
    Ok(source_edits)
}

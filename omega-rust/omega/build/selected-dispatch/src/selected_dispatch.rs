//! Plan selected operator, requirement and float execution from checked
//! facts, hand the settlement to Psi's checked->checked settlement transform,
//! then validate the settled program and publish it with its optional
//! source-query journal.
//!
//! This is the atomic execution-settlement owner. Boundary call associations,
//! intrinsic review, and later Terminal custody checks have separate owners.
//! Settlement takes sole custody of the checked program: the planned rewrites
//! are applied by `typed_trees_to_checked_trees::settle_checked_execution` to
//! the program itself, never to a staged copy.

use std::sync::Arc;

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use typed_trees_to_checked_trees::{ExecutionSettlement, SettledCallSite};

use crate::source_edits::{self, SelectedDispatchSourceEdits};

mod float_comparisons;
mod float_intrinsic;
mod operator_adapter;
mod requirement_adapter;
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

/// Settle checked-body adapters, direct requirement calls and
/// compiler-intrinsic float execution in one atomic Terminal plan rebuild.
/// Separate rebuilds would make the later family erase applications retained
/// by the earlier one. This transformation-only entrance does not retain
/// source-query custody; compiler publication uses the corresponding
/// `with_source_edits` entrance. The caller hands over its only reference
/// to the checked program and receives the settled program back.
pub fn settle_selected_execution_dispatch(
    checked: Arc<CheckedTrees>,
    selected_provider_plans: &SelectedProviderPlanFacts,
) -> Result<Arc<CheckedTrees>, Vec<Diagnostic>> {
    settle_execution(
        checked,
        selected_provider_plans,
        source_edits::SourceEditBuilder::ignored(),
    )
    .map(|(checked, _)| checked)
}

/// Atomically settle execution and seal the replaced source graph for later
/// source-semantic queries. Failure publishes neither rewrites nor a journal.
pub fn settle_selected_execution_dispatch_with_source_edits(
    checked: Arc<CheckedTrees>,
    selected_provider_plans: &SelectedProviderPlanFacts,
) -> Result<(Arc<CheckedTrees>, SelectedDispatchSourceEdits), Vec<Diagnostic>> {
    settle_execution(
        checked,
        selected_provider_plans,
        source_edits::SourceEditBuilder::default(),
    )
}

fn settle_execution(
    mut checked: Arc<CheckedTrees>,
    selected_provider_plans: &SelectedProviderPlanFacts,
    mut source_edits: source_edits::SourceEditBuilder,
) -> Result<(Arc<CheckedTrees>, SelectedDispatchSourceEdits), Vec<Diagnostic>> {
    let operator_rewrites = operator_adapter::plan_selected_operator_adapter_rewrites(
        &checked,
        selected_provider_plans,
    )?;
    let float_rewrites =
        float_intrinsic::plan_selected_float_intrinsic_rewrites(&checked, selected_provider_plans)?;
    // A settled direct-call row for a top-level boundary requirement changes
    // no source and no operator fact, but the Unit plans built before
    // settlement still target the bodyless requirement; the rows are settled
    // first (the later association pass recomputes the same set) and the
    // plans are rebuilt so the plan builder consumes them.
    crate::boundary_dispatch::settle_selected_boundary_adapter_dispatch(
        &mut checked,
        selected_provider_plans,
    )?;
    let requirement_rewrites = requirement_adapter::plan_selected_requirement_rewrites(&checked)?;
    let requirement_dispatch =
        crate::boundary_dispatch::has_top_level_requirement_dispatch(&checked);
    if operator_rewrites.is_empty() && float_rewrites.is_empty() && !requirement_dispatch {
        let executions =
            float_comparisons::selected_executions(&checked, selected_provider_plans.plans())?;
        if !checked
            .facts
            .operators
            .selected_float_comparisons
            .iter()
            .map(|(_, execution)| execution)
            .eq(executions.iter())
        {
            // All fallible derivation finished above; publishing these records
            // needs no whole-program scratch copy.
            float_comparisons::replace_executions(Arc::make_mut(&mut checked), executions);
        }
        return Ok((checked, SelectedDispatchSourceEdits::default()));
    }

    let operator_applications =
        operator_adapter::selected_operator_applications(&checked, &operator_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
    let fma_applications =
        float_intrinsic::selected_ieee_float_fma_unit_applications(&checked, &float_rewrites)
            .map_err(|diagnostic| vec![diagnostic])?;
    // Journal every site before the settlement replaces it, in application
    // order: requirement calls, then operator adapters, then float intrinsics.
    for rewrite in &requirement_rewrites {
        match rewrite.site {
            SettledCallSite::Expression(expression) => {
                source_edits.expression(&checked.typed, expression);
            }
            SettledCallSite::Statement(statement) => {
                source_edits.statement_call(&checked.typed, statement);
            }
        }
    }
    let operator_calls = operator_rewrites
        .iter()
        .map(|rewrite| {
            source_edits.expression(&checked.typed, rewrite.expression);
            rewrite.settled_call()
        })
        .collect::<Vec<_>>();
    let float_intrinsics = float_rewrites
        .iter()
        .map(|rewrite| {
            source_edits.expression(&checked.typed, rewrite.expression);
            rewrite.settled_intrinsic()
        })
        .collect::<Vec<_>>();
    let program = Arc::try_unwrap(checked).map_err(|_| {
        vec![Diagnostic::error(
            "selected execution settlement needs sole custody of the checked program",
        )]
    })?;
    let settled = typed_trees_to_checked_trees::settle_checked_execution(
        program,
        &ExecutionSettlement {
            requirement_calls: &requirement_rewrites,
            operator_adapter_calls: &operator_calls,
            float_intrinsics: &float_intrinsics,
            operator_applications: &operator_applications,
            ieee_float_fma_unit_applications: &fma_applications,
        },
    )?;
    operator_adapter::validate_selected_unit_applications(&settled, &operator_rewrites)
        .map_err(|diagnostic| vec![diagnostic])?;
    float_intrinsic::validate_selected_ieee_float_fma_unit_applications(
        &settled,
        &fma_applications,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    // Derive execution custody from the settled program, without replacing
    // Match syntax.
    let mut settled = settled;
    let comparisons =
        float_comparisons::selected_executions(&settled, selected_provider_plans.plans())?;
    float_comparisons::replace_executions(&mut settled, comparisons);
    let source_edits = source_edits.finish(&settled.typed)?;
    Ok((Arc::new(settled), source_edits))
}

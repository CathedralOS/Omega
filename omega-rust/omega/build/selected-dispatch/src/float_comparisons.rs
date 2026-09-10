//! Selected intrinsic execution custody for implicit arm comparisons.

use checked_trees::{
    CheckedOperatorOccurrence, CheckedSelectedFloatComparisonExecution, CheckedTrees,
};
use diagnostics::Diagnostic;
use numerics::literals::FloatFormat;
use provider_planning::plans::{
    CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation,
};
use semantic_vocabulary::IeeeFloatComparisonOperation;
use typed_trees::types::PrimitiveType;

#[cfg(test)]
mod tests;

pub(super) fn selected_executions(
    checked: &CheckedTrees,
    plans: &[effects::provider_plan::ProviderPlan],
) -> Result<Vec<CheckedSelectedFloatComparisonExecution>, Vec<Diagnostic>> {
    let mut executions = Vec::new();
    for (handle, operator_use) in checked.facts.operators.uses.iter() {
        if !matches!(
            operator_use.occurrence,
            CheckedOperatorOccurrence::MatchEquality { .. }
        ) {
            continue;
        }
        let Some((comparison, primitive)) = checked
            .facts
            .operators
            .selected_float_comparison(&checked.typed, handle)
        else {
            continue;
        };
        if operator_use.provider_plan_report_fingerprint == 0
            && operator_use.provider_plan_commitment.is_empty()
        {
            continue;
        }
        let fail = |reason: &str| {
            vec![Diagnostic::error(format!(
                "selected floating Match equality at {:?}: {reason}",
                operator_use.application_site(),
            ))]
        };
        if operator_use.provider_plan_commitment.is_empty() {
            return Err(fail("requires an exact ProviderPlan commitment"));
        }
        let mut matching_plans = plans.iter().filter(|plan| {
            plan.report_fingerprint() == operator_use.provider_plan_report_fingerprint
                && plan.identity_digest().as_bytes()
                    == operator_use.provider_plan_commitment.as_bytes()
        });
        let Some(plan) = matching_plans.next() else {
            return Err(fail("requires exactly one exact selected ProviderPlan"));
        };
        if matching_plans.next().is_some() {
            return Err(fail("requires exactly one exact selected ProviderPlan"));
        }
        let execution = super::derive_selected_primitive_float_binary_execution(
            &checked.typed,
            plan,
            operator_use.selected_operator_symbol,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let Some(execution) = execution else {
            // A checked adapter is not an intrinsic execution child. Its
            // separate call custody cannot be inferred from source equality.
            continue;
        };
        let format = match primitive {
            PrimitiveType::F32 => FloatFormat::F32,
            PrimitiveType::F64 => FloatFormat::F64,
            _ => return Err(fail("selected comparison has no IEEE format")),
        };
        if comparison != IeeeFloatComparisonOperation::Equal
            || execution
                != (CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                    operation: CompilerPrimitiveFloatBinaryOperation::Equal,
                    format,
                })
        {
            return Err(fail(
                "selected execution is not the exact IEEE equality and format",
            ));
        }
        let Some(execution) = CheckedSelectedFloatComparisonExecution::from_selected_provider(
            &checked.typed,
            &checked.facts.operators,
            handle,
            comparison,
            primitive,
        ) else {
            return Err(fail(
                "selected source occurrence or closed application custody drifted",
            ));
        };
        executions.push(execution);
    }
    Ok(executions)
}

pub(super) fn replace_executions(
    checked: &mut CheckedTrees,
    executions: Vec<CheckedSelectedFloatComparisonExecution>,
) {
    let retained = &mut checked.facts.operators.selected_float_comparisons;
    retained.clear();
    for execution in executions {
        retained.append(execution);
    }
}

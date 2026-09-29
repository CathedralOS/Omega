//! Selected intrinsic execution custody for implicit arm comparisons.

use checked_trees::{
    CheckedOperatorOccurrence, CheckedSelectedFloatComparisonExecution, CheckedTrees,
};
use diagnostics::Diagnostic;
use numerics::literals::FloatFormat;
use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation,
    ProviderPlanningContext, SelectedProviderReviewProvenance,
};
use semantic_vocabulary::IeeeFloatComparisonOperation;
use typed_trees::types::PrimitiveType;

#[cfg(test)]
mod tests;

pub(super) fn selected_executions(
    checked: &CheckedTrees,
    scope: source::DependencyScope,
    providers: &[SelectedProviderReviewProvenance],
) -> Result<Vec<CheckedSelectedFloatComparisonExecution>, Vec<Diagnostic>> {
    let context = ProviderPlanningContext::new(&checked.typed, scope);
    let mut executions = Vec::new();
    for (handle, operator_use) in checked.facts.operators.uses.iter() {
        if operator_use.origin.machine_symbol().is_some_and(|symbol| {
            !context.contains(symbol)
                || !checked
                    .typed
                    .machines()
                    .iter()
                    .any(|machine| machine.symbol == symbol)
        }) {
            continue;
        }
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
        let fail = |reason: &str| {
            vec![Diagnostic::error(format!(
                "selected floating Match equality at {:?}: {reason}",
                operator_use.application_site(),
            ))]
        };
        let requirement = operator_use.selected_operator_symbol;
        if !context.contains(requirement) {
            return Err(fail(
                "selected requirement belongs to a different dependency purpose",
            ));
        }
        // Keep the exact declaration association through execution settlement.
        // Build and Product copies can have identical portable schema names,
        // packages and signatures; those alone cannot select this occurrence.
        let mut matches = providers
            .iter()
            .filter(|provider| provider.provider.schema.symbol() == requirement);
        let Some(provider) = matches.next() else {
            if scope == source::DependencyScope::Build {
                return Err(fail("no provider was selected in the Build context"));
            }
            continue;
        };
        if matches.next().is_some()
            || provider.provider.row_requirements.as_slice() != [requirement]
            || provider.provider.row_realizations.len() != 1
            || !context.contains(provider.provider.row_realizations[0])
        {
            return Err(fail(
                "selected provider lost its exact requirement or realization custody",
            ));
        }
        let plan = &provider.plan;
        let execution = super::derive_selected_primitive_float_binary_execution(
            &checked.typed,
            plan,
            operator_use.selected_operator_symbol,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let Some(execution) = execution else {
            // A checked adapter is not an intrinsic execution child. Its
            // separate call custody cannot be inferred from source equality.
            if scope == source::DependencyScope::Build {
                return Err(fail(
                    "selected provider has no admitted Build comparison execution",
                ));
            }
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

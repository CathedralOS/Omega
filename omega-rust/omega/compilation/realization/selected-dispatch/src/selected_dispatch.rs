/*
Exact operator realizations and Build comparison execution settlement.

Product calls retain their public requirements through Terminal Psi, and
Product installation does not build a comparison execution table.

We settle comparison execution without importing runtime adapter dispatch or
service grants. Build admission uses this same comparison producer before
effects; unlike the product's provisional route, every Build demand must have
an executable selected child. We keep the exact declaration association through
execution settlement because Build and Product copies can have identical
portable schema names, packages, and signatures, and a checked adapter is not an
intrinsic execution child.

derive_selected_primitive_float_binary_execution binds a validated numeric
intrinsic classification from provider planning to the exact selected plan,
checking the selection and realization joins without recognizing names or
repeating signature classification.
*/

use checked_trees::{
    CheckedOperatorOccurrence, CheckedSelectedFloatComparisonExecution, CheckedTrees,
};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use numerics::literals::FloatFormat;
use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation,
    IntrinsicRequirement, ProviderPlanningContext, SelectedProviderReviewProvenance,
};
use semantic_vocabulary::IeeeFloatComparisonOperation;
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

mod operator_adapter;

pub use operator_adapter::{
    CheckedOperatorAuthoredUseKind, derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
};

pub fn settle_selected_comparison_execution(
    checked: &mut CheckedTrees,
    scope: source::DependencyScope,
    selected_provider_provenance: &[SelectedProviderReviewProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let executions =
        selected_float_comparison_executions(checked, scope, selected_provider_provenance)?;
    replace_executions(checked, executions);
    Ok(())
}

pub fn selected_float_comparison_executions(
    checked: &CheckedTrees,
    scope: source::DependencyScope,
    providers: &[SelectedProviderReviewProvenance],
) -> Result<Vec<CheckedSelectedFloatComparisonExecution>, Vec<Diagnostic>> {
    let context = ProviderPlanningContext::new(checked, scope);
    let mut executions = Vec::new();
    for (handle, operator_use) in checked.facts.operators.uses.iter() {
        if operator_use.origin.machine_symbol().is_some_and(|symbol| {
            !context.contains(symbol) || !checked.typed.machine(symbol).is_some()
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
            .selected_float_comparison(checked, handle)
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
            || provider.provider.row_realizations[0]
                .authored()
                .is_none_or(|symbol| !context.contains(symbol))
        {
            return Err(fail(
                "selected provider lost its exact requirement or realization custody",
            ));
        }
        let plan = &provider.plan;
        let execution = derive_selected_primitive_float_binary_execution(
            checked,
            plan,
            operator_use.selected_operator_symbol,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let Some(execution) = execution else {
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
            checked,
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

fn replace_executions(
    checked: &mut CheckedTrees,
    executions: Vec<CheckedSelectedFloatComparisonExecution>,
) {
    let retained = &mut checked.facts.operators.selected_float_comparisons;
    retained.clear();
    for execution in executions {
        retained.append(execution);
    }
}

pub fn derive_selected_primitive_float_binary_execution(
    typed: &typed_trees::TypedTrees,
    plan: &ProviderPlan,
    requirement: SymbolHandle,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic> {
    Ok(
        match selected_numeric_execution(typed, plan, requirement)? {
            Some(execution @ CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { .. }) => {
                Some(execution)
            }
            _ => None,
        },
    )
}

fn selected_numeric_execution(
    typed: &typed_trees::TypedTrees,
    plan: &ProviderPlan,
    requirement_symbol: SymbolHandle,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic> {
    let Some(requirement) = IntrinsicRequirement::by_symbol(typed, requirement_symbol) else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` resolves requirement symbol {:?} to no boundary operator or public receiver-free boundary requirement",
            plan.name, requirement_symbol,
        )));
    };
    let slot = &requirement.requirement_identity;
    let [method] = plan.schema.declaration.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` must retain exactly one schema method for a compiler intrinsic",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` must retain exactly one realization row for a compiler intrinsic",
            plan.name,
        )));
    };
    if !requirement.plan_row_binds(plan, method, row) {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` does not bind exact overload `{slot}`",
            plan.name,
        )));
    }
    let ProviderBinding::CompilerIntrinsic { machine, .. } = &row.binding else {
        return Ok(None);
    };
    if !requirement.intrinsic_realization_matches(typed, machine) {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact overload `{slot}` as an external leaf",
            plan.name,
        )));
    }
    requirement
        .execution_identity(typed)
        .map(Some)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "selected overload `{slot}` has no compiler-known intrinsic realization",
            ))
        })
}

//! Resolving selected float intrinsic calls.

use crate::selected_dispatch::float_intrinsic::execution_identities::{
    named_float_realization_arity, selected_compiler_intrinsic_realization,
};
use crate::selected_dispatch::float_intrinsic::named_float_realizations::preflight_named_float_execution;
use crate::selected_dispatch::float_intrinsic::{
    NamedFloatRealization, SelectedCompilerIntrinsicRealization, StagedNamedFloatRewrite,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use provider_planning::{IntrinsicRequirement, IntrinsicRequirementKind};
use typed_trees::expression::ExpressionNode;

/// One selected named use of an intrinsic-realizable requirement: a named
/// boundary-operator use or a direct top-level requirement call, each
/// stamped by provider planning with its exact selected plan.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedIntrinsicUse {
    pub(crate) expression: typed_trees::expression::ExpressionHandle,
    pub(crate) origin: checked_trees::CheckedValueOrigin,
    /// The operator symbol or the requirement machine symbol.
    pub(crate) requirement_symbol: symbols::SymbolHandle,
    pub(crate) provider_plan_report_fingerprint: u64,
    pub(crate) provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
}

impl From<&checked_trees::CheckedNamedOperatorUseFact> for SelectedIntrinsicUse {
    fn from(operator_use: &checked_trees::CheckedNamedOperatorUseFact) -> Self {
        Self {
            expression: operator_use.expression,
            origin: operator_use.origin,
            requirement_symbol: operator_use.selected_operator_symbol,
            provider_plan_report_fingerprint: operator_use.provider_plan_report_fingerprint,
            provider_plan_commitment: operator_use.provider_plan_commitment,
        }
    }
}

impl From<&checked_trees::CheckedNamedRequirementUseFact> for SelectedIntrinsicUse {
    fn from(requirement_use: &checked_trees::CheckedNamedRequirementUseFact) -> Self {
        Self {
            expression: requirement_use.expression,
            origin: requirement_use.origin,
            requirement_symbol: requirement_use.requirement_symbol,
            provider_plan_report_fingerprint: requirement_use.provider_plan_report_fingerprint,
            provider_plan_commitment: requirement_use.provider_plan_commitment,
        }
    }
}

pub(crate) fn resolve_selected_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    selected_use: &SelectedIntrinsicUse,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    if selected_use.provider_plan_commitment.is_empty() {
        return Err(Diagnostic::error(format!(
            "named float use carries ProviderPlan report fingerprint {:#018x} without an exact commitment",
            selected_use.provider_plan_report_fingerprint,
        )));
    }
    let report_matches = selected_provider_plans
        .iter()
        .filter(|plan| plan.report_fingerprint() == selected_use.provider_plan_report_fingerprint)
        .collect::<Vec<_>>();
    let plans = report_matches
        .iter()
        .copied()
        .filter(|plan| {
            plan.identity_digest().as_bytes() == selected_use.provider_plan_commitment.as_bytes()
        })
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        return Err(Diagnostic::error(
            match (report_matches.len(), plans.len()) {
                (1, 0) => format!(
                    "named float use ProviderPlan report fingerprint {:#018x} has an exact commitment that does not match the selected plan",
                    selected_use.provider_plan_report_fingerprint,
                ),
                (0, _) => format!(
                    "named float use carries unknown ProviderPlan report fingerprint {:#018x}",
                    selected_use.provider_plan_report_fingerprint,
                ),
                (_, count) => format!(
                    "named float use ProviderPlan report fingerprint {:#018x} and exact commitment match {count} selected plans",
                    selected_use.provider_plan_report_fingerprint,
                ),
            },
        ));
    };

    resolve_float_intrinsic_call(checked, selected_use, plan)
}

fn resolve_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_use: &SelectedIntrinsicUse,
    plan: &effects::provider_plan::ProviderPlan,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    let Some(requirement) =
        IntrinsicRequirement::by_symbol(&checked.typed, selected_use.requirement_symbol)
    else {
        return Err(Diagnostic::error(format!(
            "selected named float at expression {:?} resolves symbol {:?} to no boundary operator or public receiver-free boundary requirement",
            selected_use.expression, selected_use.requirement_symbol,
        )));
    };
    let overload_identity = requirement.requirement_identity.clone();
    let Some(selected_realization) =
        selected_compiler_intrinsic_realization(&checked.typed, plan, requirement.symbol)?
    else {
        return Ok(None);
    };
    let SelectedCompilerIntrinsicRealization::NamedFloat(realization) = selected_realization else {
        return Err(Diagnostic::error(format!(
            "selected named-float requirement `{overload_identity}` has no named-float execution realization",
        )));
    };
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(selected_use.expression)
    else {
        return Err(Diagnostic::error(format!(
            "selected named float intrinsic at expression {:?} is not a call",
            selected_use.expression,
        )));
    };
    let expected_arity = named_float_realization_arity(realization);
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments);
    if arguments.len() != expected_arity {
        return Err(Diagnostic::error(format!(
            "selected named float requirement `{overload_identity}` requires {expected_arity} runtime argument(s), but its checked call retains {}",
            arguments.len(),
        )));
    }
    let source_names_selected = match requirement.kind {
        IntrinsicRequirementKind::Operator => {
            typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
                .map(|resolved| resolved.symbol)
                == Some(requirement.symbol)
        }
        IntrinsicRequirementKind::TopLevelRequirement => {
            call.target_symbol == requirement.call_target
        }
    };
    let source_is_matching_builtin = matches!(realization, NamedFloatRealization::Builtin { .. })
        && typed_trees_to_checked_trees::resolve_checked_builtin_float_operator_requirement(
            &checked.typed,
            selected_use.expression,
            selected_use.origin,
        ) == Some(requirement.symbol);
    if !source_names_selected && !source_is_matching_builtin {
        return Err(Diagnostic::error(format!(
            "selected named float intrinsic at expression {:?} no longer names its checked operator symbol, requirement entry or normalized builtin",
            selected_use.expression,
        )));
    }

    let execution = preflight_named_float_execution(checked, &requirement, realization)?;
    Ok(Some(StagedNamedFloatRewrite {
        expression: selected_use.expression,
        realization,
        execution,
    }))
}

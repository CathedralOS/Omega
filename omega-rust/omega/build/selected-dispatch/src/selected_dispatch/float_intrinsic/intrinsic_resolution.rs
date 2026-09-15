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
use typed_trees::expression::ExpressionNode;

pub(crate) fn resolve_selected_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    operator_use: &checked_trees::CheckedNamedOperatorUseFact,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    if operator_use.provider_plan_commitment.is_empty() {
        return Err(Diagnostic::error(format!(
            "named float operator use carries ProviderPlan report fingerprint {:#018x} without an exact commitment",
            operator_use.provider_plan_report_fingerprint,
        )));
    }
    let report_matches = selected_provider_plans
        .iter()
        .filter(|plan| plan.report_fingerprint() == operator_use.provider_plan_report_fingerprint)
        .collect::<Vec<_>>();
    let plans = report_matches
        .iter()
        .copied()
        .filter(|plan| {
            plan.identity_digest().as_bytes() == operator_use.provider_plan_commitment.as_bytes()
        })
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        return Err(Diagnostic::error(
            match (report_matches.len(), plans.len()) {
                (1, 0) => format!(
                    "named float operator use ProviderPlan report fingerprint {:#018x} has an exact commitment that does not match the selected plan",
                    operator_use.provider_plan_report_fingerprint,
                ),
                (0, _) => format!(
                    "named float operator use carries unknown ProviderPlan report fingerprint {:#018x}",
                    operator_use.provider_plan_report_fingerprint,
                ),
                (_, count) => format!(
                    "named float operator use ProviderPlan report fingerprint {:#018x} and exact commitment match {count} selected plans",
                    operator_use.provider_plan_report_fingerprint,
                ),
            },
        ));
    };

    resolve_float_intrinsic_call(checked, operator_use, plan)
}

fn resolve_float_intrinsic_call(
    checked: &CheckedTrees,
    operator_use: &checked_trees::CheckedNamedOperatorUseFact,
    plan: &effects::provider_plan::ProviderPlan,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    let operators = checked
        .typed
        .operators()
        .iter()
        .filter(|operator| operator.symbol == operator_use.selected_operator_symbol)
        .collect::<Vec<_>>();
    let [operator] = operators.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected named float at expression {:?} resolves symbol {:?} to {} operator definitions",
            operator_use.expression,
            operator_use.selected_operator_symbol,
            operators.len(),
        )));
    };
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected named float at expression {:?} does not name a boundary operator",
            operator_use.expression,
        )));
    }
    let overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected named float at expression {:?} has an empty canonical overload identity",
            operator_use.expression,
        )));
    }
    let [method] = plan.schema.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected named-float ProviderPlan `{}` must retain exactly one schema method",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected named-float ProviderPlan `{}` must retain exactly one realization row",
            plan.name,
        )));
    };
    let operator_package = checked
        .typed
        .symbols
        .symbol_package_identity(operator.symbol);
    if plan.schema.trait_name != overload_identity
        || plan.schema.trait_package_identity != operator_package
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_owner_package_identity != operator_package
        || method.requirement_identity != overload_identity
        || !plan.schema.row_binds_method(row, method)
    {
        return Err(Diagnostic::error(format!(
            "selected named-float ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }

    let Some(selected_realization) = selected_compiler_intrinsic_realization(
        &checked.typed,
        plan,
        operator_use.selected_operator_symbol,
    )?
    else {
        return Ok(None);
    };
    let SelectedCompilerIntrinsicRealization::NamedFloat(realization) = selected_realization else {
        return Err(Diagnostic::error(format!(
            "selected named-float overload `{overload_identity}` has no named-float execution realization",
        )));
    };
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(operator_use.expression)
    else {
        return Err(Diagnostic::error(format!(
            "selected named float intrinsic at expression {:?} is not a call",
            operator_use.expression,
        )));
    };
    let expected_arity = named_float_realization_arity(realization);
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments);
    if arguments.len() != expected_arity {
        return Err(Diagnostic::error(format!(
            "selected named float overload `{overload_identity}` requires {expected_arity} runtime argument(s), but its checked call retains {}",
            arguments.len(),
        )));
    }
    let source_names_selected_operator =
        typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
            .map(|resolved| resolved.symbol)
            == Some(operator.symbol);
    let source_is_matching_builtin = matches!(realization, NamedFloatRealization::Builtin { .. })
        && typed_trees_to_checked_trees::resolve_checked_builtin_float_operator_requirement(
            &checked.typed,
            operator_use.expression,
            operator_use.origin,
        ) == Some(operator.symbol);
    if !source_names_selected_operator && !source_is_matching_builtin {
        return Err(Diagnostic::error(format!(
            "selected named float intrinsic at expression {:?} no longer names its checked operator symbol or normalized builtin",
            operator_use.expression,
        )));
    }

    let execution = preflight_named_float_execution(checked, operator, realization)?;
    Ok(Some(StagedNamedFloatRewrite {
        expression: operator_use.expression,
        realization,
        execution,
    }))
}

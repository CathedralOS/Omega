//! Resolving selected float intrinsic calls.

use crate::provider_planning::{IntrinsicRequirement, IntrinsicRequirementKind};
use crate::selected_dispatch::selected_dispatch::float_intrinsic::execution_identities::{
    named_float_realization_arity, selected_compiler_intrinsic_realization,
};
use crate::selected_dispatch::selected_dispatch::float_intrinsic::{
    NamedFloatRealization, SelectedCompilerIntrinsicRealization, StagedNamedFloatRewrite,
};
use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode;
use typed_trees_to_checked_trees::checked_trees::CheckedTrees;

/// One named use of an intrinsic-realizable requirement: a named
/// boundary-operator use or a direct top-level requirement call. Its selected
/// plan is joined by requirement identity.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedIntrinsicUse {
    pub(crate) expression:
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    pub(crate) origin: typed_trees_to_checked_trees::checked_trees::CheckedValueOrigin,
    /// The operator symbol or the requirement machine symbol.
    pub(crate) requirement_symbol: symbols::SymbolHandle,
}

impl From<&typed_trees_to_checked_trees::checked_trees::CheckedNamedOperatorUseFact>
    for SelectedIntrinsicUse
{
    fn from(
        operator_use: &typed_trees_to_checked_trees::checked_trees::CheckedNamedOperatorUseFact,
    ) -> Self {
        Self {
            expression: operator_use.expression,
            origin: operator_use.origin,
            requirement_symbol: operator_use.selected_operator_symbol,
        }
    }
}

impl From<&typed_trees_to_checked_trees::checked_trees::CheckedNamedRequirementUseFact>
    for SelectedIntrinsicUse
{
    fn from(
        requirement_use: &typed_trees_to_checked_trees::checked_trees::CheckedNamedRequirementUseFact,
    ) -> Self {
        Self {
            expression: requirement_use.expression,
            origin: requirement_use.origin,
            requirement_symbol: requirement_use.requirement_symbol,
        }
    }
}

pub(crate) fn resolve_selected_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan],
    selected_use: &SelectedIntrinsicUse,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    let Some((_, plan)) = crate::provider_planning::selected_use_plan(
        checked,
        selected_provider_plans,
        selected_use.requirement_symbol,
        selected_use.origin,
    ) else {
        return Ok(None);
    };

    resolve_float_intrinsic_call(checked, selected_use, plan)
}

fn resolve_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_use: &SelectedIntrinsicUse,
    plan: &abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan,
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
            symbol_resolved_trees_to_typed_trees::typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
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

    Ok(Some(StagedNamedFloatRewrite {
        expression: selected_use.expression,
        origin: selected_use.origin,
        requirement: requirement.symbol,
    }))
}

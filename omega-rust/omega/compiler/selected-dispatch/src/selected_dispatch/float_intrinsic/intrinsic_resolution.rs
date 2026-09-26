//! Resolving selected float intrinsic calls.

use crate::selected_dispatch::float_intrinsic::execution_identities::{
    named_float_realization_arity, selected_compiler_intrinsic_realization,
};
use crate::selected_dispatch::float_intrinsic::{
    NamedFloatRealization, SelectedCompilerIntrinsicRealization, StagedNamedFloatRewrite,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use provider_planning::{IntrinsicRequirement, IntrinsicRequirementKind};
use typed_trees::expression::ExpressionNode;

/// One named use of an intrinsic-realizable requirement: a named
/// boundary-operator use or a direct top-level requirement call. Its selected
/// plan is joined by requirement identity.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedIntrinsicUse {
    pub(crate) expression: typed_trees::expression::ExpressionHandle,
    pub(crate) origin: checked_trees::CheckedValueOrigin,
    /// The operator symbol or the requirement machine symbol.
    pub(crate) requirement_symbol: symbols::SymbolHandle,
}

impl From<&checked_trees::CheckedNamedOperatorUseFact> for SelectedIntrinsicUse {
    fn from(operator_use: &checked_trees::CheckedNamedOperatorUseFact) -> Self {
        Self {
            expression: operator_use.expression,
            origin: operator_use.origin,
            requirement_symbol: operator_use.selected_operator_symbol,
        }
    }
}

impl From<&checked_trees::CheckedNamedRequirementUseFact> for SelectedIntrinsicUse {
    fn from(requirement_use: &checked_trees::CheckedNamedRequirementUseFact) -> Self {
        Self {
            expression: requirement_use.expression,
            origin: requirement_use.origin,
            requirement_symbol: requirement_use.requirement_symbol,
        }
    }
}

pub(crate) fn resolve_selected_float_intrinsic_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    selected_use: &SelectedIntrinsicUse,
) -> Result<Option<StagedNamedFloatRewrite>, Diagnostic> {
    let Some((_, plan)) = provider_planning::selected_use_plan(
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
    // This walk includes every named requirement call. Primitive binary and
    // integer-comparison realizations have their own execution identities;
    // they are not missing named-float realizations. The exact provider join
    // above still validates their selected row before this classification.
    if matches!(
        selected_realization,
        SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(_)
            | SelectedCompilerIntrinsicRealization::PrimitiveIntegerComparison(_)
    ) {
        return Ok(None);
    }
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

    Ok(Some(StagedNamedFloatRewrite {
        expression: selected_use.expression,
        origin: selected_use.origin,
        requirement: requirement.symbol,
    }))
}

#[cfg(test)]
mod tests {
    use super::{SelectedIntrinsicUse, resolve_float_intrinsic_call};

    #[test]
    fn primitive_requirement_calls_do_not_enter_named_float_settlement() {
        for (primitive, namespace) in [("i32", "Comparison"), ("f32", "Float")] {
            let source = format!(
                "data {namespace} {{}}
                 boundary machine == {namespace}::equal(left: {primitive}, right: {primitive}) -> bool;
                 data Provider {{}}
                 machine Provider::equal(left: {primitive}, right: {primitive}) -> bool
                     satisfies {namespace}::equal via ForeignBinding::CompilerIntrinsic;
                 pub machine compare(left: {primitive}, right: {primitive}) -> bool {{ left == right }}"
            );
            let typed = crate::core_service_fixture::typed_with_core_service(
                "primitive_requirement.omg",
                &source,
            );
            let plans = provider_planning::derive_satisfies_plans(
                &typed,
                provider_planning::ProviderPlanDerivation::unevaluated(None),
            );
            let [derived] = plans.as_slice() else {
                panic!("one exact comparison provider is required")
            };
            let checked = typed_trees_to_checked_trees::lower_typed_trees(
                typed,
                &typed_trees_to_checked_trees::CheckingRequest::settled(),
            )
            .expect("check primitive comparison call");
            let uses = checked
                .facts
                .operators
                .named_requirement_uses
                .iter()
                .map(|(_, requirement_use)| SelectedIntrinsicUse::from(requirement_use))
                .collect::<Vec<_>>();
            let [selected_use] = uses.as_slice() else {
                panic!("token syntax must retain one ordinary requirement call")
            };
            assert!(
                resolve_float_intrinsic_call(&checked, selected_use, &derived.plan)
                    .expect("primitive comparisons belong to their own execution lane")
                    .is_none()
            );
            let mut mismatched = derived.plan.clone();
            mismatched.rows[0].binding =
                effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                    machine: "Provider::wrong".into(),
                };
            assert!(resolve_float_intrinsic_call(&checked, selected_use, &mismatched).is_err());
        }
    }
}

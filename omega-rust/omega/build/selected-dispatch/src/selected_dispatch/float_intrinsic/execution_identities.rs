//! Selected compiler intrinsic execution identities and realizations.

use crate::selected_dispatch::float_intrinsic::named_float_realizations::{
    named_float_realization_builtin, named_float_realization_from_operator,
};
use crate::selected_dispatch::float_intrinsic::{
    NamedFloatRealization, SelectedCompilerIntrinsicExecutionIdentity,
    SelectedCompilerIntrinsicRealization,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use numerics::arithmetic::ArithmeticDomain;
use provider_planning::{CompilerIntrinsicExecutionIdentity, CompilerNumericType};

/// Rederive the closed execution child for one exact selected provider row.
///
/// This consumes the selected plan and the checked requirement symbol. The
/// realization-machine string is validated only as an authored-plan join; it
/// is never parsed into compiler identity. A package-authored lookalike name
/// cannot enter the returned closed lane.
pub fn derive_selected_compiler_intrinsic_execution_identity(
    checked: &CheckedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    let Some(selected_realization) =
        selected_compiler_intrinsic_realization(&checked.typed, plan, requirement_symbol)?
    else {
        return Ok(None);
    };
    if let SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(identity) =
        selected_realization
    {
        return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
            identity,
        )));
    }
    let SelectedCompilerIntrinsicRealization::NamedFloat(realization) = selected_realization else {
        return Ok(Some(
            SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
        ));
    };
    if let NamedFloatRealization::Negate(format) = realization {
        return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format),
        )));
    }
    if let NamedFloatRealization::Convert(domain) = realization {
        let operators = checked
            .typed
            .operators()
            .iter()
            .filter(|operator| operator.symbol == requirement_symbol)
            .collect::<Vec<_>>();
        let [operator] = operators.as_slice() else {
            return Err(Diagnostic::error(format!(
                "selected ProviderPlan `{}` resolves conversion requirement symbol {:?} to {} operator declarations",
                plan.name,
                requirement_symbol,
                operators.len(),
            )));
        };
        return named_float_conversion_execution_identity(checked, operator, domain)
            .map(SelectedCompilerIntrinsicExecutionIdentity::Closed)
            .map(Some);
    }
    let function = named_float_realization_builtin(realization)?;
    let symbol = checked
        .typed
        .symbols
        .builtin_function_symbol(function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler builtin `{}` is absent while deriving selected provider review identity",
                function.name(),
            ))
        })?;
    if checked.typed.symbols.builtin_function_for_symbol(symbol) != Some(function) {
        return Err(Diagnostic::error(format!(
            "compiler builtin `{}` does not round-trip through its exact root slot",
            function.name(),
        )));
    }
    Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function),
    )))
}

fn named_float_conversion_execution_identity(
    checked: &CheckedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
    domain: ArithmeticDomain,
) -> Result<CompilerIntrinsicExecutionIdentity, Diagnostic> {
    let [value] = checked.typed.operator_parameters(operator) else {
        return Err(Diagnostic::error(
            "selected named-float conversion must retain exactly one parameter",
        ));
    };
    let source = checked
        .typed
        .primitive_type_reference(value.type_reference)
        .and_then(CompilerNumericType::from_primitive)
        .ok_or_else(|| {
            Diagnostic::error("selected named-float conversion has no closed numeric source type")
        })?;
    let target = checked
        .typed
        .primitive_type_reference(operator.return_type)
        .and_then(CompilerNumericType::from_primitive)
        .ok_or_else(|| {
            Diagnostic::error("selected named-float conversion has no closed numeric target type")
        })?;
    if !source.is_float() && !target.is_float() {
        return Err(Diagnostic::error(
            "selected named-float conversion does not cross a floating-point type",
        ));
    }
    Ok(CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
        source,
        target,
        domain,
    })
}

/// Derive the sealed binary semantic child only after the ordinary exact
/// selected-provider/overload/realization join. No default provider is chosen.
pub fn derive_selected_primitive_float_binary_execution(
    typed: &typed_trees::TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: symbols::SymbolHandle,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic> {
    Ok(
        match selected_compiler_intrinsic_realization(typed, plan, requirement)? {
            Some(SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(identity)) => {
                Some(identity)
            }
            _ => None,
        },
    )
}

pub(crate) fn selected_compiler_intrinsic_realization(
    typed: &typed_trees::TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<Option<SelectedCompilerIntrinsicRealization>, Diagnostic> {
    let operators = typed
        .operators()
        .iter()
        .filter(|operator| operator.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [operator] = operators.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` resolves requirement symbol {:?} to {} operator declarations",
            plan.name,
            requirement_symbol,
            operators.len(),
        )));
    };
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` compiler intrinsic does not realize a boundary operator",
            plan.name,
        )));
    }
    let overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
    if overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` compiler intrinsic has an empty canonical overload identity",
            plan.name,
        )));
    }
    let [method] = plan.schema.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` must retain exactly one schema method",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` must retain exactly one realization row",
            plan.name,
        )));
    };
    let operator_package = typed.symbols.symbol_package_identity(operator.symbol);
    if plan.schema.trait_name != overload_identity
        || plan.schema.trait_package_identity != operator_package
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_owner_package_identity != operator_package
        || method.requirement_identity != overload_identity
        || row.requirement_identity != overload_identity
        || !plan.schema.row_binds_method(row, method)
    {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }
    let ProviderBinding::CompilerIntrinsic { machine } = &row.binding else {
        return Ok(None);
    };
    if !provider_planning::intrinsic_realization_matches_operator(typed, machine, operator) {
        return Err(Diagnostic::error(format!(
            "selected compiler-intrinsic ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact overload `{overload_identity}` as an external leaf",
            plan.name,
        )));
    }
    provider_planning::compiler_intrinsic_diagnostic_label(typed, operator).ok_or_else(|| {
        Diagnostic::error(format!(
            "selected overload `{overload_identity}` has no compiler-known intrinsic realization",
        ))
    })?;
    if let Some(identity) =
        provider_planning::primitive_float_binary_intrinsic_execution_identity(typed, operator)
    {
        return Ok(Some(
            SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(identity),
        ));
    }
    Ok(Some(
        named_float_realization_from_operator(typed, operator)
            .map(SelectedCompilerIntrinsicRealization::NamedFloat)
            .unwrap_or(SelectedCompilerIntrinsicRealization::OtherCompilerPath),
    ))
}

pub(crate) const fn named_float_realization_arity(realization: NamedFloatRealization) -> usize {
    match realization {
        NamedFloatRealization::Builtin { arity, .. } => arity,
        NamedFloatRealization::Negate(_) | NamedFloatRealization::Convert(_) => 1,
        NamedFloatRealization::MultiplyThenAdd(_)
        | NamedFloatRealization::FusedMultiplyAdd(_)
        | NamedFloatRealization::DirectedFusedMultiplyAdd(_, _) => 3,
        NamedFloatRealization::DirectedSquareRoot(_, _) => 1,
        NamedFloatRealization::DirectedBinary(_, _, _) => 2,
    }
}

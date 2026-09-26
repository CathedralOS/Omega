//! Selected compiler intrinsic execution identities and realizations.

use crate::selected_dispatch::float_intrinsic::named_float_realizations::{
    named_float_realization_builtin, named_float_realization_for,
};
use crate::selected_dispatch::float_intrinsic::{
    NamedFloatRealization, SelectedCompilerIntrinsicExecutionIdentity,
    SelectedCompilerIntrinsicRealization,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use numerics::arithmetic::ArithmeticDomain;
use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, IntrinsicRequirement,
};

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
    if let SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(identity)
    | SelectedCompilerIntrinsicRealization::PrimitiveIntegerComparison(identity) =
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
        let Some(requirement) = IntrinsicRequirement::by_symbol(&checked.typed, requirement_symbol)
        else {
            return Err(Diagnostic::error(format!(
                "selected ProviderPlan `{}` resolves conversion requirement symbol {:?} to no intrinsic requirement declaration",
                plan.name, requirement_symbol,
            )));
        };
        return named_float_conversion_execution_identity(checked, &requirement, domain)
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
    requirement: &IntrinsicRequirement<'_>,
    domain: ArithmeticDomain,
) -> Result<CompilerIntrinsicExecutionIdentity, Diagnostic> {
    let [value] = requirement.parameters else {
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
        .primitive_type_reference(requirement.return_type)
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
    let Some(requirement) = IntrinsicRequirement::by_symbol(typed, requirement_symbol) else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` resolves requirement symbol {:?} to no boundary operator or public receiver-free boundary requirement",
            plan.name, requirement_symbol,
        )));
    };
    let slot = requirement.requirement_identity.clone();
    let [method] = plan.schema.methods.as_slice() else {
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
    provider_planning::compiler_intrinsic_diagnostic_label_for(typed, &requirement).ok_or_else(
        || {
            Diagnostic::error(format!(
                "selected overload `{slot}` has no compiler-known intrinsic realization",
            ))
        },
    )?;
    if let Some(identity) =
        provider_planning::primitive_float_binary_intrinsic_execution_identity_for(
            typed,
            &requirement,
        )
    {
        return Ok(Some(
            SelectedCompilerIntrinsicRealization::PrimitiveFloatBinary(identity),
        ));
    }
    if let Some(identity) =
        provider_planning::primitive_integer_comparison_intrinsic_execution_identity_for(
            typed,
            &requirement,
        )
    {
        return Ok(Some(
            SelectedCompilerIntrinsicRealization::PrimitiveIntegerComparison(identity),
        ));
    }
    Ok(Some(
        match named_float_realization_for(typed, &requirement) {
            Some(realization) => SelectedCompilerIntrinsicRealization::NamedFloat(realization),
            None => SelectedCompilerIntrinsicRealization::OtherCompilerPath,
        },
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

//! Selected compiler intrinsic execution identities and realizations.

use crate::compiler_catalog::named_float_realizations::{
    named_float_realization_builtin, named_float_realization_for,
};
use crate::compiler_catalog::{
    NamedFloatRealization, SelectedCompilerIntrinsicExecutionIdentity,
    SelectedCompilerIntrinsicRealization,
};
use crate::{CompilerIntrinsicExecutionIdentity, IntrinsicRequirement};
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use numerics::arithmetic::ArithmeticDomain;

/// Rederive the closed execution child for one exact selected provider row.
///
/// This consumes the selected plan and the checked requirement symbol. The
/// realization-machine string is validated only as an authored-plan join; it
/// is never parsed into compiler identity. This is execution classification for an already selected row, not authority
/// to select a package-authored or compiler-catalog candidate.
pub fn derive_selected_compiler_intrinsic_execution_identity(
    checked: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    let Some(selected_realization) =
        selected_compiler_intrinsic_realization(checked, plan, requirement_symbol)?
    else {
        return Ok(None);
    };
    let requirement = IntrinsicRequirement::by_symbol(checked, requirement_symbol)
        .ok_or_else(|| Diagnostic::error("selected intrinsic lost its original requirement"))?;
    execution_identity(checked, &requirement, selected_realization)
}

pub(super) fn execution_identity(
    checked: &TypedTrees,
    requirement: &IntrinsicRequirement<'_>,
    selected_realization: SelectedCompilerIntrinsicRealization,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    if let SelectedCompilerIntrinsicRealization::ClosedPrimitive(identity) = selected_realization {
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
        return named_float_conversion_execution_identity(checked, requirement, domain)
            .map(SelectedCompilerIntrinsicExecutionIdentity::Closed)
            .map(Some);
    }
    let function = named_float_realization_builtin(realization)?;
    let symbol = checked
        .symbols
        .builtin_function_symbol(function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler builtin `{}` is absent while deriving selected provider review identity",
                function.name(),
            ))
        })?;
    if checked.symbols.builtin_function_for_symbol(symbol) != Some(function) {
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
    checked: &TypedTrees,
    requirement: &IntrinsicRequirement<'_>,
    domain: ArithmeticDomain,
) -> Result<CompilerIntrinsicExecutionIdentity, Diagnostic> {
    let [value] = requirement.parameters else {
        return Err(Diagnostic::error(
            "selected named-float conversion must retain exactly one parameter",
        ));
    };
    let source = checked
        .primitive_type_reference(value.type_reference)
        .and_then(crate::primitive_numeric_type)
        .ok_or_else(|| {
            Diagnostic::error("selected named-float conversion has no closed numeric source type")
        })?;
    let target = checked
        .primitive_type_reference(requirement.return_type)
        .and_then(crate::primitive_numeric_type)
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
    typed: &checked_trees::typed_trees::TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: symbols::SymbolHandle,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic> {
    Ok(
        match selected_compiler_intrinsic_realization(typed, plan, requirement)? {
            Some(SelectedCompilerIntrinsicRealization::ClosedPrimitive(
                identity @ CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { .. },
            )) => Some(identity),
            _ => None,
        },
    )
}

fn selected_compiler_intrinsic_realization(
    typed: &checked_trees::typed_trees::TypedTrees,
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
    requirement_execution(typed, &requirement)
}

pub(super) fn requirement_execution(
    typed: &TypedTrees,
    requirement: &IntrinsicRequirement<'_>,
) -> Result<Option<SelectedCompilerIntrinsicRealization>, Diagnostic> {
    requirement.diagnostic_label(typed).ok_or_else(|| {
        Diagnostic::error(format!(
            "selected overload `{}` has no compiler-known intrinsic realization",
            requirement.requirement_identity,
        ))
    })?;
    if let Some(identity) = requirement.closed_execution(typed) {
        return Ok(Some(SelectedCompilerIntrinsicRealization::ClosedPrimitive(
            identity,
        )));
    }
    Ok(Some(
        match named_float_realization_for(typed, requirement) {
            Some(realization) => SelectedCompilerIntrinsicRealization::NamedFloat(realization),
            None => SelectedCompilerIntrinsicRealization::OtherCompilerPath,
        },
    ))
}

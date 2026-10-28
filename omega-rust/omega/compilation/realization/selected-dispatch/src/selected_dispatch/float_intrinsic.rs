//! Bind a validated numeric intrinsic classification to the exact selected plan.
//!
//! Requirement classification belongs to provider planning. This owner checks
//! the selection and realization joins, then consumes the resulting execution
//! identity without recognizing names or repeating signature classification.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use provider_planning::{CompilerIntrinsicExecutionIdentity, IntrinsicRequirement};
use symbols::SymbolHandle;

/// Every compiler path without a closed catalog child stays unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCompilerIntrinsicExecutionIdentity {
    Closed(CompilerIntrinsicExecutionIdentity),
    Unsupported,
}

/// Derive a selected child without treating realization spelling as identity.
/// Package review uses this same exact join against its supplied candidate.
pub(crate) fn derive_selected_compiler_intrinsic_execution_identity(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    requirement_symbol: SymbolHandle,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    let Some(execution) = selected_numeric_execution(checked, plan, requirement_symbol)? else {
        return Ok(None);
    };
    if let CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) = execution {
        let symbol = checked.symbols.builtin_function_symbol(function).ok_or_else(|| {
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
    }
    Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
        execution,
    )))
}

/// Build-time comparison evaluation asks only for the primitive binary lane;
/// named operations do not become primitive expressions.
pub(crate) fn derive_selected_primitive_float_binary_execution(
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
    requirement
        .execution_identity(typed)
        .map(Some)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "selected overload `{slot}` has no compiler-known intrinsic realization",
            ))
        })
}

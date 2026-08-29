use crate::model::PackageReviewCompilerIntrinsicExecution;
use omega_compiler::CheckedCompilation;
use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use omega_provider_planning::plans::CompilerIntrinsicExecutionIdentity;
use omega_selected_dispatch::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
};
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_compiler_intrinsic_execution(
    compilation: &CheckedCompilation,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    boundary_operator: bool,
    requirement_symbol: SymbolHandle,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<PackageReviewCompilerIntrinsicExecution>, Vec<Diagnostic>> {
    if !boundary_operator || !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
        return reconcile_compiler_intrinsic_execution(&plan.name, false, None, retained)
            .map(|identity| identity.map(project_execution_identity))
            .map_err(|message| vec![Diagnostic::error(message)]);
    }
    let derived = derive_selected_compiler_intrinsic_execution_identity(
        compilation,
        plan,
        requirement_symbol,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    reconcile_compiler_intrinsic_execution(&plan.name, true, derived, retained)
        .map(|identity| identity.map(project_execution_identity))
        .map_err(|message| vec![Diagnostic::error(message)])
}

const fn project_execution_identity(
    identity: CompilerIntrinsicExecutionIdentity,
) -> PackageReviewCompilerIntrinsicExecution {
    match identity {
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            PackageReviewCompilerIntrinsicExecution::BuiltinFunction(function)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(format)
        }
    }
}

fn execution_identity_label(identity: CompilerIntrinsicExecutionIdentity) -> String {
    match identity {
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            format!("builtin function `{}`", function.name())
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            format!("named-float negation `{}`", format.name())
        }
    }
}

fn reconcile_compiler_intrinsic_execution(
    plan_name: &str,
    is_compiler_intrinsic: bool,
    derived: Option<SelectedCompilerIntrinsicExecutionIdentity>,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, String> {
    if !is_compiler_intrinsic {
        return match retained {
            None => Ok(None),
            Some(identity) => Err(format!(
                "selected provider plan `{plan_name}` non-intrinsic row carries spoofed compiler execution identity {}",
                execution_identity_label(identity),
            )),
        };
    }
    match derived {
        Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(expected)) => match retained {
            Some(actual) if actual == expected => Ok(Some(expected)),
            None => Err(format!(
                "selected provider plan `{plan_name}` is missing compiler execution identity {}",
                execution_identity_label(expected),
            )),
            Some(actual) => Err(format!(
                "selected provider plan `{plan_name}` retains compiler execution identity {}, but exact selected execution rederives {}",
                execution_identity_label(actual),
                execution_identity_label(expected),
            )),
        },
        Some(SelectedCompilerIntrinsicExecutionIdentity::Unsupported) => Err(format!(
            "selected provider plan `{plan_name}` uses a compiler-intrinsic execution child without a closed package-review identity",
        )),
        None => Err(format!(
            "selected provider plan `{plan_name}` declares a compiler intrinsic without exact compiler-owned execution identity",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_reconciliation_rejects_missing_mismatched_and_spoofed_state() {
        use omega_provider_planning::plans::CompilerIntrinsicExecutionIdentity::{
            BuiltinFunction, NamedFloatNegation,
        };
        use psi_numerics::literals::FloatFormat;
        use psi_symbols::BuiltinFunction::{Max, Min};

        assert_eq!(
            reconcile_compiler_intrinsic_execution(
                "minimum",
                true,
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    BuiltinFunction(Min),
                )),
                Some(BuiltinFunction(Min)),
            ),
            Ok(Some(BuiltinFunction(Min))),
        );

        for (derived, retained, expected) in [
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    BuiltinFunction(Min),
                )),
                None,
                "missing compiler execution identity builtin function `min`",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    BuiltinFunction(Min),
                )),
                Some(BuiltinFunction(Max)),
                "retains compiler execution identity builtin function `max`, but exact selected execution rederives builtin function `min`",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    NamedFloatNegation(FloatFormat::F32),
                )),
                Some(NamedFloatNegation(FloatFormat::F64)),
                "retains compiler execution identity named-float negation `f64`, but exact selected execution rederives named-float negation `f32`",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Unsupported),
                None,
                "without a closed package-review identity",
            ),
        ] {
            let error = reconcile_compiler_intrinsic_execution("minimum", true, derived, retained)
                .expect_err("invalid intrinsic custody must reject");
            assert!(error.contains(expected), "unexpected diagnostic: {error}");
        }

        let spoofed = reconcile_compiler_intrinsic_execution(
            "ordinary",
            false,
            None,
            Some(NamedFloatNegation(FloatFormat::F32)),
        )
        .expect_err("a non-intrinsic row cannot claim compiler identity");
        assert!(spoofed.contains("spoofed compiler execution identity named-float negation `f32`"));
    }
}

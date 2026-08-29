use omega_compiler::CheckedCompilation;
use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use omega_selected_dispatch::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
};
use psi_diagnostics::Diagnostic;
use psi_symbols::{BuiltinFunction, SymbolHandle};

pub(crate) fn project_compiler_intrinsic_builtin(
    compilation: &CheckedCompilation,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    boundary_operator: bool,
    requirement_symbol: SymbolHandle,
    retained: Option<BuiltinFunction>,
) -> Result<Option<BuiltinFunction>, Vec<Diagnostic>> {
    if !boundary_operator || !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
        return reconcile_compiler_intrinsic_builtin(&plan.name, false, None, retained)
            .map_err(|message| vec![Diagnostic::error(message)]);
    }
    let derived = derive_selected_compiler_intrinsic_execution_identity(
        compilation,
        plan,
        requirement_symbol,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    reconcile_compiler_intrinsic_builtin(&plan.name, true, derived, retained)
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn reconcile_compiler_intrinsic_builtin(
    plan_name: &str,
    is_compiler_intrinsic: bool,
    derived: Option<SelectedCompilerIntrinsicExecutionIdentity>,
    retained: Option<BuiltinFunction>,
) -> Result<Option<BuiltinFunction>, String> {
    if !is_compiler_intrinsic {
        return match retained {
            None => Ok(None),
            Some(function) => Err(format!(
                "selected provider plan `{plan_name}` non-intrinsic row carries spoofed compiler builtin identity `{}`",
                function.name(),
            )),
        };
    }
    match derived {
        Some(SelectedCompilerIntrinsicExecutionIdentity::BuiltinFunction(expected)) => {
            match retained {
                Some(actual) if actual == expected => Ok(Some(expected)),
                None => Err(format!(
                    "selected provider plan `{plan_name}` is missing compiler builtin identity `{}`",
                    expected.name(),
                )),
                Some(actual) => Err(format!(
                    "selected provider plan `{plan_name}` retains compiler builtin identity `{}`, but exact selected execution rederives `{}`",
                    actual.name(),
                    expected.name(),
                )),
            }
        }
        Some(SelectedCompilerIntrinsicExecutionIdentity::NonBuiltin) => Err(format!(
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
    fn builtin_reconciliation_rejects_missing_mismatched_and_spoofed_state() {
        assert_eq!(
            reconcile_compiler_intrinsic_builtin(
                "minimum",
                true,
                Some(SelectedCompilerIntrinsicExecutionIdentity::BuiltinFunction(
                    BuiltinFunction::Min,
                )),
                Some(BuiltinFunction::Min),
            ),
            Ok(Some(BuiltinFunction::Min)),
        );

        for (derived, retained, expected) in [
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::BuiltinFunction(
                    BuiltinFunction::Min,
                )),
                None,
                "missing compiler builtin identity `min`",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::BuiltinFunction(
                    BuiltinFunction::Min,
                )),
                Some(BuiltinFunction::Max),
                "retains compiler builtin identity `max`, but exact selected execution rederives `min`",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::NonBuiltin),
                None,
                "without a closed package-review identity",
            ),
        ] {
            let error = reconcile_compiler_intrinsic_builtin("minimum", true, derived, retained)
                .expect_err("invalid intrinsic custody must reject");
            assert!(error.contains(expected), "unexpected diagnostic: {error}");
        }

        let spoofed = reconcile_compiler_intrinsic_builtin(
            "ordinary",
            false,
            None,
            Some(BuiltinFunction::Min),
        )
        .expect_err("a non-intrinsic row cannot claim compiler identity");
        assert!(spoofed.contains("spoofed compiler builtin identity `min`"));
    }
}

use crate::record::PackageReviewCompilerIntrinsicExecution;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use provider_planning::plans::{CompilerIntrinsicExecutionIdentity, ProviderSchemaDeclaration};
use selected_dispatch::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding,
};
use symbols::SymbolHandle;

pub(crate) fn project_compiler_intrinsic_execution(
    compilation: &CheckedCompilation,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    schema: ProviderSchemaDeclaration,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<PackageReviewCompilerIntrinsicExecution>, Vec<Diagnostic>> {
    if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
        return reconcile_compiler_intrinsic_execution(&plan.name, false, None, retained)
            .map(|identity| identity.map(project_execution_identity))
            .map_err(|message| vec![Diagnostic::error(message)]);
    }
    let derived =
        derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding(
            compilation,
            plan,
            schema,
            row,
            requirement_symbol,
            realization_symbol,
            selected_target,
            compilation.resolved_semantic_binding(
                package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            ),
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
        CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => {
            PackageReviewCompilerIntrinsicExecution::LinuxExitGroupI32
        }
        CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32 => {
            PackageReviewCompilerIntrinsicExecution::LinuxWriteByteI32
        }
        CompilerIntrinsicExecutionIdentity::LinuxReadByte => {
            PackageReviewCompilerIntrinsicExecution::LinuxReadByte
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            PackageReviewCompilerIntrinsicExecution::BuiltinFunction(function)
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary { operation, format }
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(format)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
            source,
            target,
            domain,
        },
    }
}

fn execution_identity_label(identity: CompilerIntrinsicExecutionIdentity) -> String {
    match identity {
        CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => {
            "Linux exit-group with one `i32` argument".to_owned()
        }
        CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32 => {
            "Linux write-byte with one `i32` argument".to_owned()
        }
        CompilerIntrinsicExecutionIdentity::LinuxReadByte => {
            "Linux read-byte returning `ByteRead`".to_owned()
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            format!("builtin function `{}`", function.name())
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            format!(
                "primitive float binary `{}.{}`",
                operation.name(),
                format.name(),
            )
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            format!("named-float negation `{}`", format.name())
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => format!(
            "named-float conversion `{} -> {}` in `{}` arithmetic",
            source.name(),
            target.name(),
            domain.name(),
        ),
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
        Some(SelectedCompilerIntrinsicExecutionIdentity::Unsupported) => match retained {
            None => Ok(None),
            Some(identity) => Err(format!(
                "selected provider plan `{plan_name}` unsupported compiler-intrinsic row carries spoofed compiler execution identity {}",
                execution_identity_label(identity),
            )),
        },
        None => Err(format!(
            "selected provider plan `{plan_name}` declares a compiler intrinsic without exact compiler-owned execution identity",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{project_execution_identity, reconcile_compiler_intrinsic_execution};
    use crate::record::PackageReviewCompilerIntrinsicExecution;
    use selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity;

    #[test]
    fn execution_reconciliation_rejects_missing_mismatched_and_spoofed_state() {
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::FloatFormat;
        use provider_planning::plans::CompilerIntrinsicExecutionIdentity::{
            BuiltinFunction, LinuxExitGroupI32, NamedFloatConversion, NamedFloatNegation,
            PrimitiveFloatBinary,
        };
        use provider_planning::plans::CompilerNumericType;
        use symbols::BuiltinFunction::{Max, Min};

        assert_eq!(
            reconcile_compiler_intrinsic_execution(
                "linux-exit",
                true,
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    LinuxExitGroupI32,
                )),
                Some(LinuxExitGroupI32),
            ),
            Ok(Some(LinuxExitGroupI32)),
        );
        assert_eq!(
            project_execution_identity(LinuxExitGroupI32),
            PackageReviewCompilerIntrinsicExecution::LinuxExitGroupI32,
        );

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

        let conversion = NamedFloatConversion {
            source: CompilerNumericType::F64,
            target: CompilerNumericType::F32,
            domain: ArithmeticDomain::Exact,
        };
        let primitive_add_f32 = PrimitiveFloatBinary {
            operation: provider_planning::plans::CompilerPrimitiveFloatBinaryOperation::Add,
            format: FloatFormat::F32,
        };
        assert_eq!(
            reconcile_compiler_intrinsic_execution(
                "convert",
                true,
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    conversion
                )),
                Some(conversion),
            ),
            Ok(Some(conversion)),
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
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    primitive_add_f32,
                )),
                Some(PrimitiveFloatBinary {
                    operation:
                        provider_planning::plans::CompilerPrimitiveFloatBinaryOperation::Subtract,
                    format: FloatFormat::F32,
                }),
                "retains compiler execution identity primitive float binary `subtract.f32`, but exact selected execution rederives primitive float binary `add.f32`",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    primitive_add_f32,
                )),
                Some(PrimitiveFloatBinary {
                    operation: provider_planning::plans::CompilerPrimitiveFloatBinaryOperation::Add,
                    format: FloatFormat::F64,
                }),
                "retains compiler execution identity primitive float binary `add.f64`, but exact selected execution rederives primitive float binary `add.f32`",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    conversion,
                )),
                Some(NamedFloatConversion {
                    source: CompilerNumericType::F32,
                    target: CompilerNumericType::F32,
                    domain: ArithmeticDomain::Exact,
                }),
                "retains compiler execution identity named-float conversion `f32 -> f32` in `Exact` arithmetic, but exact selected execution rederives named-float conversion `f64 -> f32` in `Exact` arithmetic",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    conversion,
                )),
                Some(NamedFloatConversion {
                    source: CompilerNumericType::F64,
                    target: CompilerNumericType::F64,
                    domain: ArithmeticDomain::Exact,
                }),
                "retains compiler execution identity named-float conversion `f64 -> f64` in `Exact` arithmetic, but exact selected execution rederives named-float conversion `f64 -> f32` in `Exact` arithmetic",
            ),
            (
                Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    conversion,
                )),
                Some(NamedFloatConversion {
                    source: CompilerNumericType::F64,
                    target: CompilerNumericType::F32,
                    domain: ArithmeticDomain::Wrapping,
                }),
                "retains compiler execution identity named-float conversion `f64 -> f32` in `Wrapping` arithmetic, but exact selected execution rederives named-float conversion `f64 -> f32` in `Exact` arithmetic",
            ),
        ] {
            let error = reconcile_compiler_intrinsic_execution("minimum", true, derived, retained)
                .expect_err("invalid intrinsic custody must reject");
            assert!(error.contains(expected), "unexpected diagnostic: {error}");
        }

        assert_eq!(
            reconcile_compiler_intrinsic_execution(
                "unresolved",
                true,
                Some(SelectedCompilerIntrinsicExecutionIdentity::Unsupported),
                None,
            ),
            Ok(None),
            "review evidence discloses an unsupported selected intrinsic without authorizing it",
        );
        let spoofed_unresolved = reconcile_compiler_intrinsic_execution(
            "unresolved",
            true,
            Some(SelectedCompilerIntrinsicExecutionIdentity::Unsupported),
            Some(LinuxExitGroupI32),
        )
        .expect_err("an unsupported intrinsic cannot retain a closed execution identity");
        assert!(spoofed_unresolved.contains(
            "unsupported compiler-intrinsic row carries spoofed compiler execution identity Linux exit-group with one `i32` argument"
        ));

        let spoofed = reconcile_compiler_intrinsic_execution(
            "ordinary",
            false,
            None,
            Some(NamedFloatNegation(FloatFormat::F32)),
        )
        .expect_err("a non-intrinsic row cannot claim compiler identity");
        assert!(spoofed.contains("spoofed compiler execution identity named-float negation `f32`"));

        let spoofed_conversion =
            reconcile_compiler_intrinsic_execution("ordinary", false, None, Some(conversion))
                .expect_err("a non-intrinsic row cannot claim compiler conversion identity");
        assert!(spoofed_conversion.contains(
            "spoofed compiler execution identity named-float conversion `f64 -> f32` in `Exact` arithmetic"
        ));

        let spoofed_primitive = reconcile_compiler_intrinsic_execution(
            "ordinary",
            false,
            None,
            Some(primitive_add_f32),
        )
        .expect_err("a non-intrinsic row cannot claim primitive float execution identity");
        assert!(
            spoofed_primitive
                .contains("spoofed compiler execution identity primitive float binary `add.f32`")
        );
    }
}

use crate::capture::PackageReviewInput;
use crate::record::PackageReviewCompilerIntrinsicExecution;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use provider_planning::{CompilerIntrinsicExecutionIdentity, ProviderSchemaDeclaration};
use selected_dispatch::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity_for_row,
};
use symbols::SymbolHandle;

pub(crate) fn project_compiler_intrinsic_execution(
    compilation: &PackageReviewInput<'_>,
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
    let derived = derive_selected_compiler_intrinsic_execution_identity_for_row(
        compilation,
        plan,
        schema,
        row,
        requirement_symbol,
        realization_symbol,
        selected_target,
        compilation.custody.execution().resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        ),
        compilation.custody.execution().resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
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
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => {
            PackageReviewCompilerIntrinsicExecution::HostedExitProcessI32
        }
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => {
            PackageReviewCompilerIntrinsicExecution::HostedWriteByteI32
        }
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            PackageReviewCompilerIntrinsicExecution::HostedReadByte
        }
        CompilerIntrinsicExecutionIdentity::PlacedScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => PackageReviewCompilerIntrinsicExecution::PlacedScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        },
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            PackageReviewCompilerIntrinsicExecution::BuiltinFunction(function)
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary { operation, format }
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
            operation,
            integer_type,
        } => PackageReviewCompilerIntrinsicExecution::PrimitiveIntegerComparison {
            operation,
            integer_type,
        },
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
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => {
            "Hosted process exit with one `i32` argument".to_owned()
        }
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => {
            "Hosted write-byte with one `i32` argument".to_owned()
        }
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            "Hosted read-byte returning `ByteRead`".to_owned()
        }
        CompilerIntrinsicExecutionIdentity::PlacedScalarAccess {
            access_slot,
            operation,
            ..
        } => format!("sealed placed scalar {operation:?} at field slot {access_slot}"),
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
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
            operation,
            integer_type,
        } => {
            format!(
                "primitive integer comparison `{}.{}`",
                operation.name(),
                integer_type.name(),
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

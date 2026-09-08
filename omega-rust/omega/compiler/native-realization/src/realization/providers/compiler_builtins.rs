use crate::realization::model::{NativeRealizationCoreRequest, NativeRealizationInput};
use crate::realization::providers::AdmittedTerminalMechanism;
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use diagnostics::Diagnostic;
use effects::{CompilerIntrinsicExecutionIdentity, provider_plan::ProviderBinding};
use target_operations::{
    BoundarySettlementRealization, CompilerBuiltinExecution, HostedExitProcessI32Realization,
    HostedWriteByteI32Realization, LinuxReadByteRealization,
};

pub(super) fn settle_compiler_builtins<'request>(
    input: &NativeRealizationInput,
    request: &NativeRealizationCoreRequest<'request>,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let mut admitted = Vec::with_capacity(request.compiler_builtins.len());
    let mut mechanisms = Vec::with_capacity(request.compiler_builtins.len());
    let mut seen_requirements = std::collections::BTreeSet::new();
    for proposal in request.compiler_builtins {
        let requirement = proposal.requirement_identity;
        if !seen_requirements.insert(requirement) {
            return Err(vec![Diagnostic::error(format!(
                "native realization received duplicate compiler-builtin proposal for `{requirement}`"
            ))]);
        }
        let selected_matches = request
            .selected_provider_plans
            .plans()
            .iter()
            .filter(|selected| *selected == proposal.provider_plan)
            .collect::<Vec<_>>();
        let [selected_plan] = selected_matches.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` does not rejoin one exact selected provider plan"
            ))]);
        };
        let selected_rows = selected_plan
            .rows
            .iter()
            .filter(|row| {
                row.requirement_identity == requirement
                    && matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. })
            })
            .collect::<Vec<_>>();
        if selected_rows.len() != 1 {
            return Err(vec![Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` does not rejoin one selected intrinsic row"
            ))]);
        }
        let boundaries = input
            .plan()
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == requirement)
            .collect::<Vec<_>>();
        let [boundary] = boundaries.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` does not rejoin one Terminal boundary"
            ))]);
        };
        let mechanism = compiler_intrinsic_execution_identity(proposal.execution);
        request
            .terminal_authority_policy
            .classify(mechanism)
            .map_err(|unclassified| {
                vec![Diagnostic::error(format!(
                    "receiving terminal-authority policy version {} does not classify compiler intrinsic {:?} required by `{requirement}`",
                    request.terminal_authority_policy.identity().version(),
                    unclassified.mechanism(),
                ))]
            })?;
        let realization = match proposal.execution {
            CompilerBuiltinExecution::HostedExitProcessI32
                if HostedExitProcessI32Realization::supports_target(request.target) =>
            {
                HostedExitProcessI32Realization.into()
            }
            CompilerBuiltinExecution::HostedExitProcessI32 => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize hosted process exit for `{requirement}` on {:?}",
                    request.target
                ))]);
            }
            CompilerBuiltinExecution::HostedWriteByteI32
                if HostedWriteByteI32Realization::supports_target(request.target) =>
            {
                HostedWriteByteI32Realization.into()
            }
            CompilerBuiltinExecution::HostedWriteByteI32 => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize hosted write-byte for `{requirement}` on {:?}",
                    request.target
                ))]);
            }
            CompilerBuiltinExecution::LinuxReadByte
                if request.target.object_format == target::ObjectFormat::Elf =>
            {
                LinuxReadByteRealization.into()
            }
            CompilerBuiltinExecution::LinuxReadByte => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize Linux read-byte for `{requirement}` on {:?}",
                    request.target
                ))]);
            }
        };
        admitted.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(proposal.execution),
            realization: BoundarySettlementRealization::Builtin(realization),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary: boundary.id,
            mechanism: mechanism.into(),
        });
    }
    Ok((admitted, mechanisms))
}

const fn compiler_intrinsic_execution_identity(
    execution: CompilerBuiltinExecution,
) -> CompilerIntrinsicExecutionIdentity {
    match execution {
        CompilerBuiltinExecution::HostedExitProcessI32 => {
            CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
        }
        CompilerBuiltinExecution::HostedWriteByteI32 => {
            CompilerIntrinsicExecutionIdentity::HostedWriteByteI32
        }
        CompilerBuiltinExecution::LinuxReadByte => {
            CompilerIntrinsicExecutionIdentity::LinuxReadByte
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HostedExitProcessI32Realization;
    use target::{Architecture, NativeTarget};

    #[test]
    fn hosted_exit_catalog_requires_a_complete_canonical_target() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            assert!(HostedExitProcessI32Realization::supports_target(target));
        }
        for target in [
            NativeTarget::windows_x64(),
            NativeTarget {
                architecture: Architecture::X86_64,
                ..NativeTarget::macos_arm64()
            },
            NativeTarget {
                pointer_size: 4,
                ..NativeTarget::macos_arm64()
            },
            NativeTarget {
                pointer_alignment: 4,
                ..NativeTarget::linux_arm64()
            },
        ] {
            assert!(
                !HostedExitProcessI32Realization::supports_target(target),
                "{target:?}"
            );
        }
    }
}

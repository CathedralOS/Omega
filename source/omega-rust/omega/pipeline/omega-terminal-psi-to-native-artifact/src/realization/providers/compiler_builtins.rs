use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use crate::realization::providers::AdmittedTerminalMechanism;
use omega_abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use omega_effects::{CompilerIntrinsicExecutionIdentity, provider_plan::ProviderBinding};
use omega_target_operations::{
    BoundarySettlementRealization, CompilerBuiltinExecution, LinuxExitGroupI32Realization,
};
use psi_diagnostics::Diagnostic;

pub(super) fn settle_compiler_builtins<'request>(
    input: &NativeRealizationInput,
    request: &NativeRealizationRequest<'request>,
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
            CompilerBuiltinExecution::LinuxExitGroupI32
                if request.target.object_format == omega_target::ObjectFormat::Elf =>
            {
                LinuxExitGroupI32Realization.into()
            }
            CompilerBuiltinExecution::LinuxExitGroupI32 => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize Linux exit-group for `{requirement}` on {:?}",
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
        CompilerBuiltinExecution::LinuxExitGroupI32 => {
            CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32
        }
    }
}

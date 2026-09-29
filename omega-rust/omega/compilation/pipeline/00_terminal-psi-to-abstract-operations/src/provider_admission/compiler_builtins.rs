//! Native builtin admission: independently rejoin selected provider rows,
//! Terminal demand and the target catalog before producing settlements.
//! Candidate projection lives in `proposals`; retained request construction
//! and plan-index custody live in `retained`. Neither replaces admission.
//!
//! Hosted builtins settle as call-shaped rows. Selected primitive float
//! arithmetic settles as `NearestIeeeFloatArithmetic`, which stage 02
//! realizes inline as one IEEE operation: its admission additionally
//! requires the boundary's exact `(format, format) -> format` signature, so
//! a row selected for another format or shape cannot stand in.

mod proposals;
mod retained;

pub use proposals::{
    CompilerIntrinsicPlanProvenance, CompilerIntrinsicSettlementProposal,
    demanded_boundary_identities, derive_selected_intrinsic_settlement_proposals,
};
pub use retained::{
    TerminalCompilerBuiltinProposal, prepare_retained_compiler_builtins,
    validate_retained_compiler_builtins,
};

use crate::provider_admission::AdmittedTerminalMechanism;
use crate::provider_admission::{NativeRealizationInput, ProviderAdmissionRequest};
use crate::{AdmittedBoundaryExecution, AdmittedBoundarySettlement};
use diagnostics::Diagnostic;
use effects::{CompilerIntrinsicExecutionIdentity, provider_plan::ProviderBinding};
use target_operations::{
    BoundarySettlementRealization, CompilerBuiltinExecution, HostedExitProcessI32Realization,
    HostedReadByteRealization, HostedWriteByteI32Realization,
    NearestIeeeFloatArithmeticRealization,
};

pub(super) fn settle_compiler_builtins<'request>(
    input: &NativeRealizationInput,
    request: &ProviderAdmissionRequest<'request>,
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
            .filter(|boundary| {
                boundary.identity == requirement
                    || request
                        .selected_provider_plans
                        .specialized_requirement_row(&boundary.identity)
                        .is_some_and(|specialization| {
                            specialization.selected_requirement_identity == requirement
                                && specialization.provider_plan_report_identity
                                    == selected_plan.report_fingerprint()
                        })
            })
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
            CompilerBuiltinExecution::HostedReadByte
                if HostedReadByteRealization::supports_target(request.target) =>
            {
                HostedReadByteRealization.into()
            }
            CompilerBuiltinExecution::HostedReadByte => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize hosted read-byte for `{requirement}` on {:?}",
                    request.target
                ))]);
            }
            CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } => {
                float_arithmetic_realization(
                    NearestIeeeFloatArithmeticRealization { operation, format },
                    request.target,
                    boundary,
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "compiler float arithmetic for `{requirement}` {reason}"
                    ))]
                })?
                .into()
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

/// Admit one selected float-arithmetic execution through the receiving
/// target catalog. The realization must denote exactly the Terminal
/// boundary's signature: two scalar operands and one scalar result of the
/// execution's own format, with no structural lane. A row selected for
/// another format or operand shape cannot stand in for this boundary.
fn float_arithmetic_realization(
    realization: NearestIeeeFloatArithmeticRealization,
    target: target::NativeTarget,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<NearestIeeeFloatArithmeticRealization, String> {
    if !NearestIeeeFloatArithmeticRealization::supports_target(target) {
        return Err(format!(
            "has no local target catalog realization on {target:?}"
        ));
    }
    let scalar = semantic_vocabulary::ScalarType::IeeeFloat(realization.format);
    if boundary.scalar_parameters != [scalar, scalar]
        || !boundary.structural_parameters.is_empty()
        || boundary.result != terminal_psi::BoundaryMachineResult::Scalar(scalar)
    {
        return Err("does not match its Terminal boundary signature".to_owned());
    }
    Ok(realization)
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
        CompilerBuiltinExecution::HostedReadByte => {
            CompilerIntrinsicExecutionIdentity::HostedReadByte
        }
        CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } => {
            use effects::CompilerPrimitiveFloatBinaryOperation as Primitive;
            use target_operations::IeeeFloatArithmeticOperation as Arithmetic;
            CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                operation: match operation {
                    Arithmetic::Add => Primitive::Add,
                    Arithmetic::Subtract => Primitive::Subtract,
                    Arithmetic::Multiply => Primitive::Multiply,
                    Arithmetic::Divide => Primitive::Divide,
                },
                format: match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                        numerics::literals::FloatFormat::F32
                    }
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                        numerics::literals::FloatFormat::F64
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests;

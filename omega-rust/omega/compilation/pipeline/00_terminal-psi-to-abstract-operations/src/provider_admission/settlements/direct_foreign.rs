//! Exact concrete native call settlement; no abstract provider election.
//! Evaluated Import/Syscall and compiler catalog supply retain distinct origins.
use crate::provider_admission::{
    AdmittedTerminalMechanism, NativeRealizationInput, ProviderAdmissionRequest,
};
use crate::{AdmittedBoundaryExecution, AdmittedBoundarySettlement};
use diagnostics::Diagnostic;
use effects::DirectForeignBinding;

pub(super) fn settle<'execution>(
    input: &NativeRealizationInput,
    request: &ProviderAdmissionRequest<'_, 'execution>,
    callback_boundaries: &[(
        semantic_vocabulary::BoundaryMachineId,
        &crate::AdmittedNativeCallbackArgument,
    )],
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'execution>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let mut settlements = Vec::new();
    let mut mechanisms = Vec::new();
    let mut settled = std::collections::BTreeSet::new();
    for prepared in request.direct_foreign_settlements {
        let direct = prepared.realization;
        let callable = direct.callable_identity();
        if !settled.insert(callable) {
            return Err(vec![Diagnostic::error(format!(
                "direct foreign callable `{callable}` has ambiguous original binding custody"
            ))]);
        }
        let error = |reason: &str| {
            vec![Diagnostic::error(format!(
                "direct foreign callable `{callable}` {reason}"
            ))]
        };
        if direct.target() != request.profile {
            return Err(error("does not carry the admitted target profile"));
        }
        let mut boundaries =
            input.plan().boundary_machines.iter().filter(|boundary| {
                input.plan().identities.text(boundary.identity) == Some(callable)
            });
        let boundary = boundaries
            .next()
            .ok_or_else(|| error("has no retained concrete boundary"))?;
        if boundaries.next().is_some() {
            return Err(error("has ambiguous concrete boundary custody"));
        }
        if !super::super::compiler_builtins::boundary_contract_matches_schema(
            boundary,
            direct.schema(),
            direct.requirement_identity(),
        ) {
            return Err(error(
                "does not match its original public completion contract",
            ));
        }
        let entry = direct
            .boundary_entry_plan()
            .ok_or_else(|| error("has no original calling application"))?;
        if let DirectForeignBinding::CompilerIntrinsic { execution, .. } = direct.binding() {
            let execution =
                super::super::compiler_builtins::compiler_builtin_execution(*execution, None)
                    .ok_or_else(|| error("has no closed native catalog execution"))?;
            let realization = super::super::compiler_builtins::admit_compiler_builtin_realization(
                input,
                request,
                boundary,
                execution,
                callable,
                Some(entry),
            )?;
            settlements.push(AdmittedBoundarySettlement {
                boundary: boundary.id,
                execution: AdmittedBoundaryExecution::DirectCompilerIntrinsic(direct, execution),
                realization: target_operations::BoundarySettlementRealization::Builtin(realization),
            });
            mechanisms.push(AdmittedTerminalMechanism {
                boundary: boundary.id,
                mechanism: super::super::compiler_builtins::compiler_intrinsic_execution_identity(
                    execution,
                )
                .into(),
            });
            continue;
        }
        let (binding, mechanism) = match direct.binding() {
            DirectForeignBinding::Import(import) => {
                let stack = prepared
                    .same_stack
                    .as_ref()
                    .ok_or_else(|| error("has no exact same-stack contribution"))?;
                if stack.provider_plan_commitment()
                    != task_plans::SameStackExecutionCommitment::DirectForeign(direct.identity())
                    || input.plan().identities.text(stack.requirement_identity()) != Some(callable)
                {
                    return Err(error("does not retain its own direct execution commitment"));
                }
                let callbacks = callback_boundaries
                    .iter()
                    .filter(|(owner, _)| *owner == boundary.id)
                    .map(|(_, callback)| *callback)
                    .collect::<Vec<_>>();
                let mechanism = super::source_imports::import_terminal_mechanism(
                    import.locator(),
                    entry,
                    &callbacks,
                )
                .map_err(|reason| error(&reason))?;
                (
                    target_operations::NormalizedForeignCallBinding {
                        boundary_entry_plan: entry.clone(),
                        mechanism: target_operations::NormalizedForeignCallMechanism::Import {
                            locator: import.locator().clone(),
                            same_stack_contribution: stack.clone(),
                        },
                    },
                    mechanism,
                )
            }
            DirectForeignBinding::Syscall(syscall) => {
                let mechanism = super::super::terminal_authority_policy::conservative_syscall_terminal_mechanism(
                    direct.target(), syscall.number(), input.plan(), boundary.id,
                ).map_err(|reason| error(&reason))?;
                (
                    target_operations::NormalizedForeignCallBinding {
                        boundary_entry_plan: entry.clone(),
                        mechanism: target_operations::NormalizedForeignCallMechanism::SystemCall(
                            target_operations::NormalizedSystemCallBinding {
                                target: direct.target(),
                                number: syscall.number(),
                                requirement_identity: callable.to_owned(),
                                provider_plan_report_identity: u64::from_le_bytes(
                                    direct.identity()[..8].try_into().expect("digest prefix"),
                                )
                                .max(1),
                                provider_plan_commitment: direct.identity(),
                                direct_foreign_realization: Some(direct.identity()),
                            },
                        ),
                    },
                    mechanism,
                )
            }
            DirectForeignBinding::CompilerIntrinsic { .. } => {
                return Err(error("lost its catalog settlement"));
            }
        };
        settlements.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: AdmittedBoundaryExecution::DirectForeign(direct),
            realization: target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                binding,
            ),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary: boundary.id,
            mechanism,
        });
    }
    Ok((settlements, mechanisms))
}

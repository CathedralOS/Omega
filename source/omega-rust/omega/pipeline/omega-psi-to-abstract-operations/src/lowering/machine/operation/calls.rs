use omega_abstract_operations::{
    AbstractOperation, AbstractReboundDynamicScalarDispatch, AbstractResult, CompletionClaimSource,
};
use psi_terminal::{
    ClosedConformanceApplication, Operation, OperationKind, TerminalDynamicDispatchCatalog,
    TerminalMachine,
};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => AbstractOperation::CallUnit {
            psi_operation: operation.id,
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        },
        OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let result = operation.result.expect_scalar();
            AbstractOperation::CallStructuralScalar {
                psi_operation: operation.id,
                result: AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                },
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            }
        }
        OperationKind::CallDynamicScalar {
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        } => {
            let descriptors = dynamic_dispatch
                .rebound_descriptors
                .iter()
                .filter(|descriptor| {
                    descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
                })
                .collect::<Vec<_>>();
            let [descriptor] = descriptors.as_slice() else {
                return Err(LoweringError::InvalidDynamicScalarCall(operation.id));
            };
            let selections = |ordinal| {
                dynamic_dispatch
                    .selections
                    .iter()
                    .filter(|selection| {
                        selection.owner == machine.id && selection.ordinal == ordinal
                    })
                    .collect::<Vec<_>>()
            };
            let initial = selections(descriptor.initial_selection_ordinal);
            let rebound = selections(descriptor.rebound_selection_ordinal);
            let dispatches = dynamic_dispatch
                .indirect_dispatches
                .iter()
                .filter(|dispatch| {
                    dispatch.owner == machine.id
                        && dispatch.operation == operation.id
                        && dispatch.descriptor_ordinal == descriptor_ordinal
                })
                .collect::<Vec<_>>();
            let ([initial], [rebound], [dispatch]) = (
                initial.as_slice(),
                rebound.as_slice(),
                dispatches.as_slice(),
            ) else {
                return Err(LoweringError::InvalidDynamicScalarCall(operation.id));
            };
            let applications = closed_conformance_applications
                .iter()
                .filter(|application| {
                    application.owner == machine.id
                        && application.report_fingerprint
                            == initial.conformance_application_report_fingerprint
                        && application.commitment == initial.conformance_application_commitment
                        && application.report_fingerprint
                            == rebound.conformance_application_report_fingerprint
                        && application.commitment == rebound.conformance_application_commitment
                })
                .collect::<Vec<_>>();
            let [application] = applications.as_slice() else {
                return Err(LoweringError::InvalidDynamicScalarCall(operation.id));
            };
            let rows = application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait_identity == dispatch.declaring_trait_identity
                        && row.public_requirement_identity == dispatch.public_requirement_identity
                        && row.requirement_identity == dispatch.requirement_identity
                        && row.realization_identity == dispatch.realization_identity
                        && row.realization_callable_identity.as_deref()
                            == Some(dispatch.realization_callable_identity.as_str())
                })
                .collect::<Vec<_>>();
            let callables = application
                .realization_callables
                .iter()
                .filter(|callable| {
                    callable.source_callable_identity == dispatch.realization_callable_identity
                        && callable.machine == dispatch.realization
                })
                .collect::<Vec<_>>();
            if !matches!(rows.as_slice(), [_]) || !matches!(callables.as_slice(), [_]) {
                return Err(LoweringError::InvalidDynamicScalarCall(operation.id));
            }
            let result = operation.result.expect_scalar();
            AbstractOperation::CallDynamicScalar {
                psi_operation: operation.id,
                result: AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                },
                dynamic_dispatch: AbstractReboundDynamicScalarDispatch {
                    initial: (*initial).clone(),
                    rebound: (*rebound).clone(),
                    descriptor: (*descriptor).clone(),
                    application: (*application).clone(),
                    dispatch: (*dispatch).clone(),
                },
                requirement_obligations,
                crash_continuations,
            }
        }
        OperationKind::CallDynamicParameterScalar { .. } => {
            return Err(LoweringError::InvalidDynamicScalarCall(operation.id));
        }
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedStructuralResult(machine.id));
            };
            AbstractOperation::CallStructural {
                psi_operation: operation.id,
                result,
                callee,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
            }
        }
        OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let mut completion_claim_sources = machine
                .entry_claims
                .iter()
                .cloned()
                .map(|entry| CompletionClaimSource {
                    claim: entry.claim,
                    entry: Some(entry),
                    content: None,
                })
                .collect::<Vec<_>>();
            for content in &machine.content_entry_claims {
                if let Some(source) = completion_claim_sources
                    .iter_mut()
                    .find(|source| source.claim == content.claim)
                {
                    source.content = Some(content.clone());
                } else {
                    completion_claim_sources.push(CompletionClaimSource {
                        claim: content.claim,
                        entry: None,
                        content: Some(content.clone()),
                    });
                }
            }
            completion_claim_sources.sort();
            AbstractOperation::BoundaryCall {
                psi_operation: operation.id,
                result: operation.result.scalar().map(|result| AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                }),
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            }
        }
        OperationKind::Call {
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => AbstractOperation::Call {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            scalar_type: operation.result.expect_scalar().scalar_type,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        },
        _ => unreachable!("call router is exhaustive"),
    })
}

#[cfg(test)]
mod tests {
    use omega_abstract_operations::AbstractOperation;
    use psi_core::{
        BlockId, ContractId, IntegerSign, IntegerType, MachineId, ObligationId, OperationId,
        ScalarType, ValueId,
    };
    use psi_terminal::{
        CrashCause, CrashRouteBucket, CrashRouteGuard, MachineContract, Operation, OperationKind,
        OperationResult, TerminalMachine, TerminalMachineResult, ValueDeclaration,
    };

    use super::lower;

    fn machine() -> TerminalMachine {
        TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: Vec::new(),
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn call_rows() -> (Vec<ObligationId>, Vec<CrashRouteBucket>) {
        (
            vec![ObligationId::new(1).unwrap()],
            vec![CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        )
    }

    fn scalar_result() -> OperationResult {
        OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        })
    }

    #[test]
    fn ordinary_unit_and_structural_scalar_calls_retain_nonempty_semantic_rows() {
        let machine = machine();
        let callee = MachineId::new(2).unwrap();
        let cases = [
            (
                OperationResult::Unit,
                OperationKind::CallUnit {
                    callee,
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: call_rows().0,
                    crash_continuations: call_rows().1,
                },
            ),
            (
                scalar_result(),
                OperationKind::CallStructuralScalar {
                    callee,
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: call_rows().0,
                    crash_continuations: call_rows().1,
                },
            ),
            (
                scalar_result(),
                OperationKind::Call {
                    callee,
                    arguments: Vec::new(),
                    requirement_obligations: call_rows().0,
                    crash_continuations: call_rows().1,
                },
            ),
        ];

        for (index, (result, kind)) in cases.into_iter().enumerate() {
            let lowered = lower(
                &Operation {
                    id: OperationId::new(index as u64 + 1).unwrap(),
                    result,
                    kind,
                },
                &machine,
                &psi_terminal::TerminalDynamicDispatchCatalog::default(),
                &[],
            )
            .unwrap();
            let (requirements, crashes) = match lowered {
                AbstractOperation::CallUnit {
                    requirement_obligations,
                    crash_continuations,
                    ..
                }
                | AbstractOperation::CallStructuralScalar {
                    requirement_obligations,
                    crash_continuations,
                    ..
                }
                | AbstractOperation::CallDynamicScalar {
                    requirement_obligations,
                    crash_continuations,
                    ..
                }
                | AbstractOperation::Call {
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => (requirement_obligations, crash_continuations),
                _ => unreachable!(),
            };
            assert_eq!(requirements, call_rows().0);
            assert_eq!(crashes, call_rows().1);
        }
    }
}

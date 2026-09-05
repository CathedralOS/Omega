use abstract_operations::{AbstractOperation, AbstractResult, CompletionClaimSource};
use terminal_psi::{
    ClosedConformanceApplication, Operation, OperationKind, TerminalDynamicDispatchCatalog,
    TerminalMachine,
};

use crate::lowering::LoweringError;

mod dynamic_dispatch;

pub(super) use dynamic_dispatch::lower_stored_descriptor;
use dynamic_dispatch::{
    lower_dynamic_arguments, lower_parameter_dynamic_dispatch, lower_rebound_dynamic_dispatch,
    lower_stored_dynamic_dispatch,
};

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::CallUnit {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let dynamic_arguments = lower_dynamic_arguments(
                machine,
                operation,
                callee,
                dynamic_dispatch,
                closed_conformance_applications,
            )?;
            if dynamic_arguments.is_empty() {
                AbstractOperation::CallUnit {
                    psi_operation: operation.id,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
            } else {
                if !arguments.is_empty() {
                    return Err(LoweringError::UnsupportedUnitCallScalarAndDynamicArguments(
                        operation.id,
                    ));
                }
                AbstractOperation::CallUnitWithDynamicArguments {
                    psi_operation: operation.id,
                    callee,
                    structural_arguments,
                    dynamic_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
            }
        }
        OperationKind::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let result = operation.result.expect_scalar();
            let dynamic_arguments = lower_dynamic_arguments(
                machine,
                operation,
                callee,
                dynamic_dispatch,
                closed_conformance_applications,
            )?;
            if dynamic_arguments.is_empty() {
                AbstractOperation::CallStructuralScalar {
                    psi_operation: operation.id,
                    result: AbstractResult {
                        value: result.id,
                        scalar_type: result.scalar_type,
                    },
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
            } else {
                AbstractOperation::CallStructuralScalarWithDynamicArguments {
                    psi_operation: operation.id,
                    result: AbstractResult {
                        value: result.id,
                        scalar_type: result.scalar_type,
                    },
                    callee,
                    structural_arguments,
                    dynamic_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
            }
        }
        OperationKind::CallDynamicScalar {
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        } => {
            let result = operation.result.expect_scalar();
            let result = AbstractResult {
                value: result.id,
                scalar_type: result.scalar_type,
            };
            if dynamic_dispatch
                .stored_dispatches
                .iter()
                .any(|dispatch| dispatch.owner == machine.id && dispatch.operation == operation.id)
            {
                AbstractOperation::CallStoredDynamicScalar {
                    psi_operation: operation.id,
                    result,
                    dynamic_dispatch: lower_stored_dynamic_dispatch(
                        machine,
                        operation,
                        descriptor_ordinal,
                        Some(result.scalar_type),
                        dynamic_dispatch,
                        closed_conformance_applications,
                    )?,
                    requirement_obligations,
                    crash_continuations,
                }
            } else {
                AbstractOperation::CallDynamicScalar {
                    psi_operation: operation.id,
                    result,
                    dynamic_dispatch: lower_rebound_dynamic_dispatch(
                        machine,
                        operation,
                        descriptor_ordinal,
                        Some(result.scalar_type),
                        dynamic_dispatch,
                        closed_conformance_applications,
                    )?,
                    requirement_obligations,
                    crash_continuations,
                }
            }
        }
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal,
            requirement_slot,
            requirement_obligations,
            crash_continuations,
        } => {
            let result = operation.result.expect_scalar();
            AbstractOperation::CallDynamicParameterScalar {
                psi_operation: operation.id,
                result: AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                },
                dynamic_dispatch: lower_parameter_dynamic_dispatch(
                    machine,
                    operation,
                    parameter_ordinal,
                    requirement_slot,
                    Some(result.scalar_type),
                    dynamic_dispatch,
                )?,
                requirement_obligations,
                crash_continuations,
            }
        }
        OperationKind::CallDynamicUnit {
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        } => AbstractOperation::CallDynamicUnit {
            psi_operation: operation.id,
            dynamic_dispatch: lower_rebound_dynamic_dispatch(
                machine,
                operation,
                descriptor_ordinal,
                None,
                dynamic_dispatch,
                closed_conformance_applications,
            )?,
            requirement_obligations,
            crash_continuations,
        },
        OperationKind::CallDynamicParameterUnit {
            parameter_ordinal,
            requirement_slot,
            requirement_obligations,
            crash_continuations,
        } => AbstractOperation::CallDynamicParameterUnit {
            psi_operation: operation.id,
            dynamic_dispatch: lower_parameter_dynamic_dispatch(
                machine,
                operation,
                parameter_ordinal,
                requirement_slot,
                None,
                dynamic_dispatch,
            )?,
            requirement_obligations,
            crash_continuations,
        },
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
                arguments: Vec::new(),
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
            }
        }
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedStructuralResult(machine.id));
            };
            AbstractOperation::CallStructural {
                psi_operation: operation.id,
                result,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence: Vec::new(),
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
                result: match &operation.result {
                    terminal_psi::OperationResult::Unit => {
                        abstract_operations::AbstractBoundaryResult::Unit
                    }
                    terminal_psi::OperationResult::Scalar(result) => {
                        abstract_operations::AbstractBoundaryResult::Scalar(AbstractResult {
                            value: result.id,
                            scalar_type: result.scalar_type,
                        })
                    }
                    terminal_psi::OperationResult::Structural(result) => {
                        abstract_operations::AbstractBoundaryResult::Structural(result.clone())
                    }
                },
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
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::{
        BlockId, ContractId, IntegerSign, IntegerType, MachineId, ObligationId, OperationId,
        ScalarType, ValueId,
    };
    use terminal_psi::{
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
                    arguments: Vec::new(),
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
                    arguments: Vec::new(),
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
                &terminal_psi::TerminalDynamicDispatchCatalog::default(),
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
                | AbstractOperation::CallUnitWithDynamicArguments {
                    requirement_obligations,
                    crash_continuations,
                    ..
                }
                | AbstractOperation::CallStructuralScalarWithDynamicArguments {
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

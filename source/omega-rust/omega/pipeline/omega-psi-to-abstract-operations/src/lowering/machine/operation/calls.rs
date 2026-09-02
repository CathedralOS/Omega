use omega_abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource, AbstractOperation,
    AbstractParameterDynamicDispatch, AbstractReboundDynamicDispatch, AbstractResult,
    CompletionClaimSource,
};
use psi_terminal::{
    ClosedConformanceApplication, Operation, OperationKind, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalMachine,
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
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
            } else {
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
            AbstractOperation::CallDynamicScalar {
                psi_operation: operation.id,
                result: AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                },
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

fn lower_rebound_dynamic_dispatch(
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor_ordinal: u32,
    expected_result: Option<psi_core::ScalarType>,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractReboundDynamicDispatch, LoweringError> {
    let descriptors = dynamic_dispatch
        .rebound_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let selections = |ordinal| {
        dynamic_dispatch
            .selections
            .iter()
            .filter(|selection| selection.owner == machine.id && selection.ordinal == ordinal)
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
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let initial_applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == initial.conformance_application_report_fingerprint
                && application.commitment == initial.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == rebound.conformance_application_report_fingerprint
                && application.commitment == rebound.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let ([initial_application], [application]) =
        (initial_applications.as_slice(), applications.as_slice())
    else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
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
                && closed_result_scalar(callable.result) == expected_result
        })
        .collect::<Vec<_>>();
    if !matches!(rows.as_slice(), [_]) || !matches!(callables.as_slice(), [_]) {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }
    Ok(AbstractReboundDynamicDispatch {
        initial: (*initial).clone(),
        rebound: (*rebound).clone(),
        descriptor: (*descriptor).clone(),
        initial_application: (*initial_application).clone(),
        application: (*application).clone(),
        dispatch: (*dispatch).clone(),
    })
}

fn lower_parameter_dynamic_dispatch(
    machine: &TerminalMachine,
    operation: &Operation,
    parameter_ordinal: u32,
    requirement_slot: u32,
    expected_result: Option<psi_core::ScalarType>,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
) -> Result<AbstractParameterDynamicDispatch, LoweringError> {
    let parameters = dynamic_dispatch
        .parameters
        .iter()
        .filter(|parameter| parameter.owner == machine.id && parameter.ordinal == parameter_ordinal)
        .collect::<Vec<_>>();
    let dispatches = dynamic_dispatch
        .parameter_dispatches
        .iter()
        .filter(|dispatch| {
            dispatch.owner == machine.id
                && dispatch.operation == operation.id
                && dispatch.parameter_ordinal == parameter_ordinal
                && dispatch.requirement_slot == requirement_slot
        })
        .collect::<Vec<_>>();
    let ([parameter], [dispatch]) = (parameters.as_slice(), dispatches.as_slice()) else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let requirements = parameter
        .requirements
        .iter()
        .filter(|requirement| {
            requirement.slot == requirement_slot
                && closed_result_scalar(requirement.result) == expected_result
        })
        .collect::<Vec<_>>();
    if !matches!(requirements.as_slice(), [_]) {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }
    Ok(AbstractParameterDynamicDispatch {
        parameter: (*parameter).clone(),
        dispatch: (*dispatch).clone(),
    })
}

fn closed_result_scalar(
    result: psi_terminal::ClosedConformanceCallableResult,
) -> Option<psi_core::ScalarType> {
    match result {
        psi_terminal::ClosedConformanceCallableResult::Unit => None,
        psi_terminal::ClosedConformanceCallableResult::I32 => Some(psi_core::ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                .expect("the closed i32 result is valid"),
        )),
        psi_terminal::ClosedConformanceCallableResult::Bool => Some(psi_core::ScalarType::Boolean),
    }
}

fn lower_dynamic_arguments(
    machine: &TerminalMachine,
    operation: &Operation,
    callee: psi_core::MachineId,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<Vec<AbstractDynamicDescriptorArgument>, LoweringError> {
    let mut parameters = dynamic_dispatch
        .parameters
        .iter()
        .filter(|parameter| parameter.owner == callee)
        .collect::<Vec<_>>();
    parameters.sort_by_key(|parameter| parameter.ordinal);
    let arguments = dynamic_dispatch
        .arguments
        .iter()
        .filter(|argument| argument.owner == machine.id && argument.operation == operation.id)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }

    parameters
        .into_iter()
        .map(|parameter| {
            let matches = arguments
                .iter()
                .filter(|argument| argument.parameter_ordinal == parameter.ordinal)
                .collect::<Vec<_>>();
            let [argument] = matches.as_slice() else {
                return Err(LoweringError::InvalidDynamicCall(operation.id));
            };
            let source = match argument.source {
                TerminalDynamicDescriptorSource::Selection { ordinal } => {
                    let selections = dynamic_dispatch
                        .selections
                        .iter()
                        .filter(|selection| {
                            selection.owner == machine.id && selection.ordinal == ordinal
                        })
                        .collect::<Vec<_>>();
                    let [selection] = selections.as_slice() else {
                        return Err(LoweringError::InvalidDynamicCall(operation.id));
                    };
                    let applications = closed_conformance_applications
                        .iter()
                        .filter(|application| {
                            application.owner == machine.id
                                && application.report_fingerprint
                                    == selection.conformance_application_report_fingerprint
                                && application.commitment
                                    == selection.conformance_application_commitment
                        })
                        .collect::<Vec<_>>();
                    let [application] = applications.as_slice() else {
                        return Err(LoweringError::InvalidDynamicCall(operation.id));
                    };
                    AbstractDynamicDescriptorSource::Selection {
                        selection: (**selection).clone(),
                        application: (**application).clone(),
                    }
                }
                TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
                    lower_rebound_argument_source(
                        machine,
                        operation,
                        ordinal,
                        dynamic_dispatch,
                        closed_conformance_applications,
                    )?
                }
                TerminalDynamicDescriptorSource::Parameter { ordinal } => {
                    let sources = dynamic_dispatch
                        .parameters
                        .iter()
                        .filter(|source| source.owner == machine.id && source.ordinal == ordinal)
                        .collect::<Vec<_>>();
                    let [source] = sources.as_slice() else {
                        return Err(LoweringError::InvalidDynamicCall(operation.id));
                    };
                    AbstractDynamicDescriptorSource::Parameter((*source).clone())
                }
            };
            Ok(AbstractDynamicDescriptorArgument {
                argument: (**argument).clone(),
                target: parameter.clone(),
                source,
            })
        })
        .collect()
}

fn lower_rebound_argument_source(
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor_ordinal: u32,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractDynamicDescriptorSource, LoweringError> {
    let descriptors = dynamic_dispatch
        .rebound_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let selections = |ordinal| {
        dynamic_dispatch
            .selections
            .iter()
            .filter(|selection| selection.owner == machine.id && selection.ordinal == ordinal)
            .collect::<Vec<_>>()
    };
    let initial = selections(descriptor.initial_selection_ordinal);
    let rebound = selections(descriptor.rebound_selection_ordinal);
    let ([initial], [rebound]) = (initial.as_slice(), rebound.as_slice()) else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let initial_applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == initial.conformance_application_report_fingerprint
                && application.commitment == initial.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == rebound.conformance_application_report_fingerprint
                && application.commitment == rebound.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let ([initial_application], [application]) =
        (initial_applications.as_slice(), applications.as_slice())
    else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    Ok(AbstractDynamicDescriptorSource::Rebound {
        initial: (*initial).clone(),
        rebound: (*rebound).clone(),
        descriptor: (*descriptor).clone(),
        initial_application: (*initial_application).clone(),
        application: (*application).clone(),
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

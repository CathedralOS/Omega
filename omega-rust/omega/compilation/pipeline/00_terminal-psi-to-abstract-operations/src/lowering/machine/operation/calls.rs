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
            erased_arguments: _,
            erased_proof_arguments: _,
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
            erased_arguments: _,
            erased_proof_arguments: _,
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
            erased_arguments: _,
            erased_proof_arguments: _,
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
            erased_arguments: _,
            erased_proof_arguments: _,
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
mod tests {}

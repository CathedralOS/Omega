use abstract_operations::{
    AbstractDynamicCallResult, AbstractDynamicDispatch, AbstractOperation, AbstractResult,
    AbstractStructuralCallResult, CompletionClaimSource,
};
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
            if !dynamic_arguments.is_empty() && !arguments.is_empty() {
                return Err(LoweringError::UnsupportedUnitCallScalarAndDynamicArguments(
                    operation.id,
                ));
            }
            AbstractOperation::CallStructural {
                psi_operation: operation.id,
                callee,
                arguments,
                structural_arguments,
                dynamic_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                result: AbstractStructuralCallResult::Unit,
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
            // A call that forwards descriptors drops its scalar arguments
            // instead of rejecting them as the Unit call does.
            let arguments = if dynamic_arguments.is_empty() {
                arguments
            } else {
                Vec::new()
            };
            AbstractOperation::CallStructural {
                psi_operation: operation.id,
                result: AbstractStructuralCallResult::Scalar(AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                }),
                callee,
                arguments,
                structural_arguments,
                dynamic_arguments,
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
            let result = operation.result.expect_scalar();
            let result = AbstractResult {
                value: result.id,
                scalar_type: result.scalar_type,
            };
            // A published stored dispatch reloads the descriptor from its
            // aggregate field; every other scalar descriptor call is rebound.
            let dispatch =
                if dynamic_dispatch.stored_dispatches.iter().any(|dispatch| {
                    dispatch.owner == machine.id && dispatch.operation == operation.id
                }) {
                    AbstractDynamicDispatch::Stored(lower_stored_dynamic_dispatch(
                        machine,
                        operation,
                        descriptor_ordinal,
                        Some(result.scalar_type),
                        dynamic_dispatch,
                        closed_conformance_applications,
                    )?)
                } else {
                    AbstractDynamicDispatch::Rebound(lower_rebound_dynamic_dispatch(
                        machine,
                        operation,
                        descriptor_ordinal,
                        Some(result.scalar_type),
                        dynamic_dispatch,
                        closed_conformance_applications,
                    )?)
                };
            AbstractOperation::CallDynamic {
                psi_operation: operation.id,
                dispatch,
                result: AbstractDynamicCallResult::Scalar(result),
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
            AbstractOperation::CallDynamic {
                psi_operation: operation.id,
                dispatch: AbstractDynamicDispatch::Parameter(lower_parameter_dynamic_dispatch(
                    machine,
                    operation,
                    parameter_ordinal,
                    requirement_slot,
                    Some(result.scalar_type),
                    dynamic_dispatch,
                )?),
                result: AbstractDynamicCallResult::Scalar(AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                }),
                requirement_obligations,
                crash_continuations,
            }
        }
        // A Unit descriptor call is always rebound: stored custody is admitted
        // only with a scalar result.
        OperationKind::CallDynamicUnit {
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        } => AbstractOperation::CallDynamic {
            psi_operation: operation.id,
            dispatch: AbstractDynamicDispatch::Rebound(lower_rebound_dynamic_dispatch(
                machine,
                operation,
                descriptor_ordinal,
                None,
                dynamic_dispatch,
                closed_conformance_applications,
            )?),
            result: AbstractDynamicCallResult::Unit,
            requirement_obligations,
            crash_continuations,
        },
        OperationKind::CallDynamicParameterUnit {
            parameter_ordinal,
            requirement_slot,
            requirement_obligations,
            crash_continuations,
        } => AbstractOperation::CallDynamic {
            psi_operation: operation.id,
            dispatch: AbstractDynamicDispatch::Parameter(lower_parameter_dynamic_dispatch(
                machine,
                operation,
                parameter_ordinal,
                requirement_slot,
                None,
                dynamic_dispatch,
            )?),
            result: AbstractDynamicCallResult::Unit,
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
                result: AbstractStructuralCallResult::Structural {
                    result,
                    returned_claim_transfers,
                    selected_evidence,
                },
                callee,
                arguments: Vec::new(),
                structural_arguments,
                dynamic_arguments: Vec::new(),
                claim_transfers,
                requirement_obligations,
                crash_continuations,
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
                result: AbstractStructuralCallResult::Structural {
                    result,
                    returned_claim_transfers,
                    selected_evidence: Vec::new(),
                },
                callee,
                arguments,
                structural_arguments,
                dynamic_arguments: Vec::new(),
                claim_transfers,
                requirement_obligations,
                crash_continuations,
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

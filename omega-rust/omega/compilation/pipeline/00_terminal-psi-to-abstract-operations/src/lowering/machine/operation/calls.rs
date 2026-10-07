use abstract_operations::{
    AbstractOperation, AbstractResult, AbstractStructuralCallResult, CompletionClaimSource,
};
use terminal_psi::{Operation, OperationKind, TerminalMachine};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
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
        } => AbstractOperation::CallStructural {
            psi_operation: operation.id,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            result: AbstractStructuralCallResult::Unit,
        },
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
            AbstractOperation::CallStructural {
                psi_operation: operation.id,
                result: AbstractStructuralCallResult::Scalar(AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                }),
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            }
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
                result: AbstractStructuralCallResult::Structural {
                    result,
                    returned_claim_transfers,
                    selected_evidence,
                },
                callee,
                arguments: Vec::new(),
                structural_arguments,
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

use omega_abstract_operations::{AbstractOperation, AbstractResult, CompletionClaimSource};
use psi_terminal::{Operation, OperationKind, TerminalMachine};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => AbstractOperation::CallUnit {
            psi_operation: operation.id,
            callee,
            structural_arguments,
            claim_transfers,
        },
        OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
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
            callee, arguments, ..
        } => AbstractOperation::Call {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            scalar_type: operation.result.expect_scalar().scalar_type,
            callee,
            arguments,
        },
        _ => unreachable!("call router is exhaustive"),
    })
}

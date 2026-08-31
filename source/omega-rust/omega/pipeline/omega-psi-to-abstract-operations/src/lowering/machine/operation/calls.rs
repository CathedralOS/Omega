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

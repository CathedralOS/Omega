//! Ordinary-machine control-flow and terminal-edge projection.
use super::LoweringError;
use abstract_operations::{AbstractOperation, AbstractSuccessor, ValueBinding};
use semantic_vocabulary::BlockId;
use std::collections::BTreeMap;
use terminal_psi::{TerminalAffineCleanupAction, TerminalMachine, Terminator};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_terminator(
    module: &terminal_psi::TerminalModule,
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    result: Option<terminal_psi::ValueDeclaration>,
    operations: &mut Vec<AbstractOperation>,
) -> Result<(), LoweringError> {
    match &block.terminator {
        Terminator::InvokeNoNormal {
            edge,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => operations.push(AbstractOperation::InvokeNoNormal {
            psi_edge: *edge,
            callee: *callee,
            arguments: arguments.clone(),
            structural_arguments: structural_arguments.clone(),
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        }),
        Terminator::Jump {
            edge,
            target,
            arguments,
            cleanup_actions,
            structural_arguments,
            // Proof-only lanes; runtime lowering has no erased actuals to read.
            erased_arguments: _,
            erased_proof_arguments: _,
        } => {
            let target_block =
                blocks
                    .get(target)
                    .copied()
                    .ok_or(LoweringError::VerifiedBlockMissing {
                        machine: machine.id,
                        block: *target,
                    })?;
            if target_block.parameters.len() != arguments.len()
                || target_block.structural_parameters.len() != structural_arguments.len()
            {
                return Err(LoweringError::VerifiedJumpArityMismatch { edge: *edge });
            }
            operations.push(AbstractOperation::Jump {
                psi_edge: *edge,
                target: *target,
                structural_bindings: target_block
                    .structural_parameters
                    .iter()
                    .zip(structural_arguments)
                    .map(
                        |(parameter, argument)| abstract_operations::AbstractStructuralBinding {
                            parameter: parameter.place,
                            argument: argument.clone(),
                        },
                    )
                    .collect(),
                bindings: target_block
                    .parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| ValueBinding {
                        parameter: parameter.id,
                        argument: *argument,
                        scalar_type: parameter.scalar_type,
                    })
                    .collect(),
                cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
            });
        }
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let lower_successor = |successor: &terminal_psi::SuccessorEdge| {
                let target_block = blocks.get(&successor.target).copied().ok_or(
                    LoweringError::VerifiedBlockMissing {
                        machine: machine.id,
                        block: successor.target,
                    },
                )?;
                if target_block.parameters.len() != successor.arguments.len()
                    || target_block.structural_parameters.len()
                        != successor.structural_arguments.len()
                {
                    return Err(LoweringError::VerifiedJumpArityMismatch {
                        edge: successor.edge,
                    });
                }
                Ok(AbstractSuccessor {
                    psi_edge: successor.edge,
                    target: successor.target,
                    structural_bindings: target_block
                        .structural_parameters
                        .iter()
                        .zip(&successor.structural_arguments)
                        .map(|(parameter, argument)| {
                            abstract_operations::AbstractStructuralBinding {
                                parameter: parameter.place,
                                argument: argument.clone(),
                            }
                        })
                        .collect(),
                    bindings: target_block
                        .parameters
                        .iter()
                        .zip(&successor.arguments)
                        .map(|(parameter, argument)| ValueBinding {
                            parameter: parameter.id,
                            argument: *argument,
                            scalar_type: parameter.scalar_type,
                        })
                        .collect(),
                    cleanup_actions: runtime_cleanup_actions(module, &successor.cleanup_actions),
                })
            };
            operations.push(AbstractOperation::Conditional {
                condition: *condition,
                when_true: lower_successor(when_true)?,
                when_false: lower_successor(when_false)?,
            });
        }
        Terminator::StructuralCase {
            source,
            path,
            cases,
        } => {
            let cases = cases
                .iter()
                .map(|successor| {
                    let target_block = blocks.get(&successor.target).copied().ok_or(
                        LoweringError::VerifiedBlockMissing {
                            machine: machine.id,
                            block: successor.target,
                        },
                    )?;
                    if target_block.parameters.len() != successor.payload_fields.len() {
                        return Err(LoweringError::VerifiedJumpArityMismatch {
                            edge: successor.edge,
                        });
                    }
                    Ok(abstract_operations::AbstractStructuralCaseSuccessor {
                        psi_edge: successor.edge,
                        target: successor.target,
                        case: successor.case,
                        payloads: target_block
                            .parameters
                            .iter()
                            .zip(&successor.payload_fields)
                            .map(|(parameter, field)| {
                                abstract_operations::AbstractStructuralCasePayloadBinding {
                                    parameter: parameter.id,
                                    field: *field,
                                    scalar_type: parameter.scalar_type,
                                }
                            })
                            .collect(),
                        cleanup_actions: runtime_cleanup_actions(
                            module,
                            &successor.cleanup_actions,
                        ),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            operations.push(AbstractOperation::StructuralCase {
                source: *source,
                path: path.clone(),
                cases,
            });
        }
        Terminator::Return {
            edge,
            value,
            cleanup_actions,
        } => {
            let result = result.ok_or(LoweringError::ScalarReturnFromUnitMachine(machine.id))?;
            operations.push(AbstractOperation::Return {
                psi_edge: *edge,
                result: result.id,
                value: *value,
                scalar_type: result.scalar_type,
                cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
            });
        }
        Terminator::ReturnUnit {
            edge,
            cleanup_actions,
        } => {
            if result.is_some() {
                return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
            }
            operations.push(AbstractOperation::ReturnUnit {
                psi_edge: *edge,
                cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
            });
        }
        Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            cleanup_actions,
        } if machine.result.structural().is_some() => {
            operations.push(AbstractOperation::ReturnStructural {
                psi_edge: *edge,
                source: *source,
                returned_claims: returned_claims.clone(),
                trivial_affine_locals: Vec::new(),
                cleanup_actions: runtime_cleanup_actions(module, cleanup_actions),
            });
        }
        Terminator::ReturnStructural { edge, .. } => {
            return Err(LoweringError::UnsupportedStructuralReturn {
                machine: machine.id,
                edge: *edge,
            });
        }
        Terminator::ExternalCompletion {
            edge,
            boundary,
            kind,
            arguments,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
        } => operations.push(AbstractOperation::ExternalCompletion {
            psi_edge: *edge,
            boundary: *boundary,
            kind: *kind,
            arguments: arguments.clone(),
            structural_arguments: structural_arguments.clone(),
            requirement_obligations: requirement_obligations.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        }),
        Terminator::Crash {
            edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            operations.push(AbstractOperation::Crash {
                psi_edge: *edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            });
        }
    }
    Ok(())
}

/// All normal edges cross the same firewall: preserve action order and actual
/// receiver custody, but erase the proof-site identities owned by Psi.
fn runtime_cleanup_actions(
    module: &terminal_psi::TerminalModule,
    actions: &[TerminalAffineCleanupAction],
) -> Vec<TerminalAffineCleanupAction> {
    actions
        .iter()
        .cloned()
        .map(|action| match action {
            TerminalAffineCleanupAction::InvokeNominal(mut cleanup) => {
                cleanup.cleanup_receiver = runtime_cleanup_receiver(module, &cleanup);
                cleanup.requirement_obligations.clear();
                TerminalAffineCleanupAction::InvokeNominal(cleanup)
            }
            action => action,
        })
        .collect()
}

/// The verified hook's parameter is operational custody. A contextual-only
/// contract root has no runtime declaration and must disappear at the firewall.
fn runtime_cleanup_receiver(
    module: &terminal_psi::TerminalModule,
    cleanup: &terminal_psi::NominalAffineCleanup,
) -> Option<semantic_vocabulary::PlaceId> {
    module
        .machine(cleanup.cleanup_machine)
        .and_then(|machine| machine.structural_parameters.first())
        .map(|parameter| parameter.place)
}

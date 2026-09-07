//! Ordinary-machine control-flow and terminal-edge projection.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_terminator(
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    result: Option<terminal_psi::ValueDeclaration>,
    lowered_unit_affine_locals: &[LoweredAffineLocal],
    retain_payloadless_for_optimization: bool,
    operations: &mut Vec<AbstractOperation>,
) -> Result<(), LoweringError> {
    match &block.terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            trivial_affine_discards,
            residual_affine_discards,
        } => {
            if !residual_affine_discards.is_empty() {
                return Err(LoweringError::UnsupportedPartialAffineContinuation {
                    machine: machine.id,
                    edge: *edge,
                });
            }
            let target_block =
                blocks
                    .get(target)
                    .copied()
                    .ok_or(LoweringError::VerifiedBlockMissing {
                        machine: machine.id,
                        block: *target,
                    })?;
            if target_block.parameters.len() != arguments.len() {
                return Err(LoweringError::VerifiedJumpArityMismatch { edge: *edge });
            }
            operations.push(AbstractOperation::Jump {
                psi_edge: *edge,
                target: *target,
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
                trivial_affine_discards: trivial_affine_discards.clone(),
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
                if target_block.parameters.len() != successor.arguments.len() {
                    return Err(LoweringError::VerifiedJumpArityMismatch {
                        edge: successor.edge,
                    });
                }
                Ok(AbstractSuccessor {
                    psi_edge: successor.edge,
                    target: successor.target,
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
                    trivial_affine_discards: successor.trivial_affine_discards.clone(),
                })
            };
            operations.push(AbstractOperation::Conditional {
                condition: *condition,
                when_true: lower_successor(when_true)?,
                when_false: lower_successor(when_false)?,
            });
        }
        Terminator::StructuralCase { source, cases } => {
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
                        trivial_affine_discards: successor.trivial_affine_discards.clone(),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            operations.push(AbstractOperation::StructuralCase {
                source: *source,
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
                cleanup_actions: cleanup_actions
                    .iter()
                    .cloned()
                    .map(|action| match action {
                        TerminalAffineCleanupAction::InvokeNominal(mut cleanup) => {
                            // Psi has already verified these proof-site identities. They
                            // carry no native realization meaning and must not become a
                            // second semantic authority in Omega artifacts.
                            cleanup.cleanup_receiver = None;
                            cleanup.requirement_obligations.clear();
                            TerminalAffineCleanupAction::InvokeNominal(cleanup)
                        }
                        action => action,
                    })
                    .collect(),
            });
        }
        Terminator::ReturnUnit {
            edge,
            trivial_affine_discards,
        } => {
            if result.is_some() {
                return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
            }
            let consumed_locals = operations
                .iter()
                .flat_map(|operation| match operation {
                    AbstractOperation::CallUnit {
                        structural_arguments,
                        ..
                    } => structural_arguments
                        .iter()
                        .filter(|argument| {
                            argument.path.is_empty()
                                && argument.access == terminal_psi::StructuralAccess::Owned
                        })
                        .map(|argument| argument.place)
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                })
                .collect::<BTreeSet<_>>();
            let expected_locals = lowered_unit_affine_locals
                .iter()
                .rev()
                .filter(|(_, place, _)| !consumed_locals.contains(&place.id))
                .map(|(_, place, _)| place.id)
                .collect::<Vec<_>>();
            if !trivial_affine_discards.starts_with(&expected_locals) {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: *edge,
                });
            }
            operations.push(AbstractOperation::ReturnUnit {
                psi_edge: *edge,
                cleanup_actions: trivial_affine_discards
                    .iter()
                    .copied()
                    .map(TerminalAffineCleanupAction::DiscardRoot)
                    .collect(),
            });
        }
        Terminator::ReturnUnitPartialAffine {
            edge,
            trivial_affine_discards,
            residual_affine_discards,
        } => {
            if result.is_some() {
                return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
            }
            let expected_locals = lowered_unit_affine_locals
                .iter()
                .rev()
                .map(|(_, place, _)| place.id)
                .collect::<Vec<_>>();
            if !trivial_affine_discards.starts_with(&expected_locals) {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: *edge,
                });
            }
            operations.push(AbstractOperation::ReturnUnit {
                psi_edge: *edge,
                cleanup_actions: trivial_affine_discards
                    .iter()
                    .copied()
                    .map(TerminalAffineCleanupAction::DiscardRoot)
                    .chain(
                        residual_affine_discards
                            .iter()
                            .cloned()
                            .map(TerminalAffineCleanupAction::DiscardResidual),
                    )
                    .collect(),
            });
        }
        Terminator::ReturnUnitNominalAffine { edge, cleanups } => {
            if result.is_some() || !lowered_unit_affine_locals.is_empty() {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: *edge,
                });
            }
            operations.push(AbstractOperation::ReturnUnit {
                psi_edge: *edge,
                cleanup_actions: cleanups
                    .iter()
                    .cloned()
                    .map(|mut cleanup| {
                        // Psi has already verified these proof-site identities. They
                        // carry no native realization meaning and must not become a
                        // second semantic authority in Omega artifacts.
                        cleanup.cleanup_receiver = None;
                        cleanup.requirement_obligations.clear();
                        TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    })
                    .collect(),
            });
        }
        Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            trivial_affine_discards,
        } if retain_payloadless_for_optimization
            && machine.result.structural().is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
            }) =>
        {
            operations.push(AbstractOperation::ReturnStructural {
                psi_edge: *edge,
                source: *source,
                returned_claims: returned_claims.clone(),
                trivial_affine_locals: Vec::new(),
                trivial_affine_discards: trivial_affine_discards.clone(),
            });
        }
        Terminator::ReturnStructural { edge, .. } => {
            return Err(LoweringError::UnsupportedStructuralReturn {
                machine: machine.id,
                edge: *edge,
            });
        }
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

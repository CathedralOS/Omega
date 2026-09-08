//! Surviving obligation-owner and structural-frontier custody.

use super::*;

pub(super) fn validate_surviving_frontiers(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let context = input.context();
    let reconstructed = context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| (row.obligation.id, row))
        .collect::<BTreeMap<_, _>>();
    let accepted = context
        .accepted_facts()
        .iter()
        .map(|fact| (fact.obligation, fact))
        .collect::<BTreeMap<_, _>>();
    for function in &unit.functions {
        validate_surviving_byte_operations(context.module(), function)?;
        let Some(frontiers) = context.structural_frontiers().machine(function.machine) else {
            return Err(
                OptimizationUnitValidationError::MissingStructuralFrontierMachine(function.machine),
            );
        };
        for fact in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner_matches = reconstructed.get(obligation).is_some_and(|row| {
                row.owner
                    == terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            if !owner_matches || !accepted.contains_key(obligation) {
                return Err(
                    OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                );
            }
        }
        for site in function.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .flat_map(|node| node.provenance.iter().copied())
        }) {
            match site {
                PsiProvenance::Operation(operation)
                    if frontiers.operation_entry(operation).is_none()
                        || frontiers.operation_exit(operation).is_none() =>
                {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralOperationFrontier {
                            machine: function.machine,
                            operation,
                        },
                    );
                }
                PsiProvenance::Edge(edge) if frontiers.edge_entry(edge).is_none() => {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                            machine: function.machine,
                            edge,
                        },
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_surviving_byte_operations(
    module: &terminal_psi::TerminalModule,
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .map(|node| &node.operation)
    {
        let (psi_operation, obligation) = match operation {
            O::ByteSequenceRead {
                psi_operation,
                obligation,
                ..
            }
            | O::ByteSequenceSubslice {
                psi_operation,
                obligation,
                ..
            } => (*psi_operation, Some(*obligation)),
            O::ByteSequenceLength { psi_operation, .. } => (*psi_operation, None),
            _ => continue,
        };
        let original = module
            .machines
            .iter()
            .find(|machine| machine.id == function.machine)
            .and_then(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == psi_operation)
            });
        let matches = original.is_some_and(|original| {
            let (result, kind) = match operation {
                O::ByteSequenceSubslice {
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Structural(result.clone()),
                    terminal_psi::OperationKind::ByteSequenceSubslice {
                        source: *source,
                        start: *start,
                        end: *end,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::ByteSequenceRead {
                    result,
                    source,
                    index,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::ByteSequenceRead {
                        source: *source,
                        index: *index,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::ByteSequenceLength { result, source, .. } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::ByteSequenceLength { source: *source },
                ),
                _ => return false,
            };
            original.result == result && original.kind == kind
        });
        if !matches {
            return Err(if let Some(obligation) = obligation {
                OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                    machine: function.machine,
                    operation: psi_operation,
                    obligation,
                }
            } else {
                OptimizationUnitValidationError::StructuralCatalogMismatch {
                    machine: Some(function.machine),
                }
            });
        }
    }
    Ok(())
}

//! Admit whole shared or exclusive byte views and plain owned structural arrivals.

use terminal_psi::{TerminalModule, Terminator};

#[cfg(test)]
mod tests;

use super::LoweringError;

/// Check the complete module before selecting any machine lowering family.
/// Ranked, ordinary, optimizer, and provider replay share this entrance; a
/// descriptor-only transfer need not contain any fenced byte operation.
pub(super) fn validate_structural_block_bindings(
    module: &TerminalModule,
) -> Result<(), LoweringError> {
    for machine in &module.machines {
        for block in &machine.blocks {
            if block
                .structural_parameters
                .iter()
                .enumerate()
                .any(|(position, parameter)| {
                    block.id == machine.entry
                        || parameter.position as usize != position
                        || parameter.is_self
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                        || !module.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && match parameter.access {
                                    terminal_psi::StructuralAccess::Owned => matches!(
                                        parameter.multiplicity,
                                        terminal_psi::StructuralMultiplicity::Affine
                                            | terminal_psi::StructuralMultiplicity::Unrestricted
                                    ),
                                    terminal_psi::StructuralAccess::SharedBorrow
                                    | terminal_psi::StructuralAccess::MutableBorrow => {
                                        parameter.multiplicity
                                            == terminal_psi::StructuralMultiplicity::Unrestricted
                                            && matches!(
                                                declaration.shape,
                                                terminal_psi::StructuralTypeShape::ByteSequence(
                                                    terminal_psi::ByteSequenceCarrier::BorrowedView
                                                )
                                            )
                                    }
                                    terminal_psi::StructuralAccess::WriteOnlyBorrow => false,
                                }
                        })
                })
            {
                return Err(LoweringError::UnsupportedStructuralBlockParameters {
                    machine: machine.id,
                    block: block.id,
                });
            }
            let unsupported_edge = match &block.terminator {
                Terminator::Jump {
                    edge,
                    structural_arguments,
                    ..
                } => structural_arguments
                    .iter()
                    .any(unsupported_argument)
                    .then_some(*edge),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [when_true, when_false]
                    .into_iter()
                    .find(|successor| {
                        successor
                            .structural_arguments
                            .iter()
                            .any(unsupported_argument)
                    })
                    .map(|successor| successor.edge),
                Terminator::StructuralCase { .. }
                | Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnUnitPartialAffine { .. }
                | Terminator::ReturnUnitNominalAffine { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::Crash { .. } => None,
            };
            if let Some(edge) = unsupported_edge {
                return Err(LoweringError::UnsupportedStructuralSuccessorArguments {
                    machine: machine.id,
                    edge,
                });
            }
        }
    }
    Ok(())
}

fn unsupported_argument(argument: &terminal_psi::StructuralArgument) -> bool {
    !argument.path.is_empty()
        || !matches!(
            argument.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::Owned
        )
}

//! Admit only the retained whole immutable byte-view structural edge vocabulary.

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
                        || parameter.access != terminal_psi::StructuralAccess::SharedBorrow
                        || parameter.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                        || !module.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BorrowedView
                                    )
                                )
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
    !argument.path.is_empty() || argument.access != terminal_psi::StructuralAccess::SharedBorrow
}

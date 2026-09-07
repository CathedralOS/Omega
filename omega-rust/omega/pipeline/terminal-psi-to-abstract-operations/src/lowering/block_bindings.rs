//! Native and optimizer projections do not yet retain structural edge bindings.

use terminal_psi::{TerminalModule, Terminator};

#[cfg(test)]
mod tests;

use super::LoweringError;

/// Check the complete module before selecting any machine lowering family.
/// Ranked, ordinary, optimizer, and provider replay share this entrance; a
/// descriptor-only transfer need not contain any fenced byte operation.
pub(super) fn reject_structural_block_bindings(
    module: &TerminalModule,
) -> Result<(), LoweringError> {
    for machine in &module.machines {
        for block in &machine.blocks {
            if !block.structural_parameters.is_empty() {
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
                } => (!structural_arguments.is_empty()).then_some(*edge),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [when_true, when_false]
                    .into_iter()
                    .find(|successor| !successor.structural_arguments.is_empty())
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

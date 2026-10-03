//! Constructor operands use the ordinary partial-affine transfer frontier.
//! Stage custody before removing any runtime cells so malformed later operands
//! cannot consume an earlier child or its still-live siblings.
use crate::custody::{consume_affine_projection, remove_affine_root, resolve_structural_arguments};
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::StructuralRuntimePlace;
use std::collections::BTreeSet;
use terminal_psi::{StructuralAffineDiscard, StructuralArgument};

impl TerminalExecution {
    pub(crate) fn stage_constructor_operand(
        &self,
        argument: &StructuralArgument,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        consumed: &mut Vec<StructuralArgument>,
    ) -> Result<bool, TerminalInterpretError> {
        let affine = self
            .live_affine_frontier
            .iter()
            .any(|entry| entry.place == argument.place);
        if affine {
            if argument.path.is_empty() {
                if !remove_affine_root(frontier, argument.place) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            } else {
                consume_affine_projection(
                    &self.program.structural_types,
                    &self.structural_values,
                    frontier,
                    argument,
                )?;
            }
            consumed.push(argument.clone());
        } else if !argument.path.is_empty() {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        Ok(affine)
    }

    pub(crate) fn commit_constructor_operands(
        &mut self,
        frontier: BTreeSet<StructuralAffineDiscard>,
        consumed: &[StructuralArgument],
    ) -> Result<(), TerminalInterpretError> {
        // Resolve all selected homes before removing roots, including multiple
        // disjoint operands that consume the last siblings of one root.
        let homes = consumed
            .iter()
            .map(|argument| {
                if argument.path.is_empty() && self.scalar_case_values.contains_key(&argument.place)
                {
                    return Ok(None);
                }
                let child = resolve_structural_arguments(
                    &self.program.structural_types,
                    &self.structural_values,
                    std::slice::from_ref(argument),
                )?
                .pop()
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                Ok(Some(StructuralRuntimePlace::from(&child)))
            })
            .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
        for (argument, home) in consumed.iter().zip(homes) {
            if let Some(home) = home {
                self.prune_subtree(&home);
            }
            if !frontier.iter().any(|entry| entry.place == argument.place) {
                self.structural_values.remove(&argument.place);
                self.scalar_case_values.remove(&argument.place);
            }
        }
        self.live_affine_frontier = frontier;
        Ok(())
    }
}

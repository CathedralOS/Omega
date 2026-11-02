//! Materialize the selected arm's checked payload values before its operands.
//! They use the ordinary copied-value producer and expression-root namespace;
//! no live storage read may substitute the former source after sibling effects.

use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::stored_structural_identity;
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use crate::unit::attached_unit::composed_control::emission::{FrameValues, StateFrame};
use terminal_psi::{StructuralParameterDeclaration, ValueDeclaration};

/// One private copy born on this selected target. Its exact value handle
/// distinguishes the saved owner from every authored by-value read.
pub(super) struct CompletedCopy {
    binding: crate::emission::operation_emission::buffer::ComputedValueBinding,
}

impl StateGraphEmission<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture_dispatch_copies(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        statement_ordinal: u32,
        parameters: &[StructuralParameterDeclaration],
        evaluation: &mut Evaluation,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
    ) -> Result<Vec<CompletedCopy>, LoweringError> {
        let checked = self.checked;
        let state = &self.plan.states[position];
        let owner = self.admitted.authored[position].0.symbol;
        let roots = crate::expression_preparation::selected_values::roots(
            checked,
            owner,
            state.state,
            statement_ordinal,
        );
        let mut completed = Vec::new();
        for root in roots {
            let structural_type = crate::terminal_identities::lookup_type_id(
                &self.catalogs.type_ids,
                &stored_structural_identity(checked, root.type_reference),
            )?;
            StateFrame::new(checked, owner, state, &self.state_erased[position]).evaluate(
                self.catalogs,
                parameters,
                sequence,
                FrameValues {
                    source_value_count: values.len(),
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                },
                |frame| {
                    let (mut environment, emission) = frame.split();
                    environment.produce_value(
                        identities,
                        root.root,
                        structural_type,
                        emission.evaluation,
                        emission.values,
                        emission.next_value,
                        emission.next_block,
                        emission.next_edge,
                        emission.calls,
                        emission.operations,
                    )
                },
            )?;
            let binding = operations
                .computed_values
                .iter()
                .rev()
                .find(|(value, _)| *value == root.root)
                .map(|(_, binding)| binding.clone())
                .ok_or(LoweringError::Unsupported(
                    "selected copy producer did not publish its completed home",
                ))?;
            if binding.structural_type != structural_type
                || !binding.argument.path.is_empty()
                || binding.argument.access != terminal_psi::StructuralAccess::Owned
            {
                return unsupported("selected copied value did not establish its own whole home");
            }
            completed.push(CompletedCopy { binding });
        }
        Ok(completed)
    }

    /// Copy and DescriptorCleanup authorize this private value's normal death.
    /// Its saved producer has only copy/read consumers; addresses still name
    /// the original source. Name correspondence does not prove availability:
    /// ordinary frontier settlement and canonical validation independently
    /// require the actual whole home. No borrowed backing is disposed.
    pub(super) fn complete_dispatch_copies(
        &self,
        copies: &[CompletedCopy],
        evaluation: &Evaluation,
        operations: &OperationBuffer,
    ) -> Result<Vec<crate::unit::unit_cleanup::CleanupRegion>, LoweringError> {
        let mut regions = Vec::new();
        for copy in copies.iter().rev() {
            // Ordinary cleanup tracks the physical carrier's multiplicity and
            // selected residuals. Source Copy never grants disposal of another
            // root, a nominal hook, or a borrowed pointee.
            regions.extend(evaluation.normal_discard_regions(
                copy.binding.argument.place,
                &self.catalogs.structural_types,
                operations,
            )?);
        }
        Ok(regions)
    }
}

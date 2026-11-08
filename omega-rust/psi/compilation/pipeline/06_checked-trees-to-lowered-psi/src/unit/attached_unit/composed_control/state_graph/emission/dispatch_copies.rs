//! Materialize the selected arm's checked payload values before its operands.
//! They use the ordinary copied-value producer and expression-root namespace;
//! no live storage read may substitute the former source after sibling effects.

use super::super::super::{CheckedTrees, catalogs};
use super::super::CheckedComposedUnitControlStatePlan;
use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::shared::lowering_error::{LoweringError, unsupported};
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use crate::unit::attached_unit::composed_control::emission::{FrameValues, StateFrame};
use terminal_psi::{StructuralParameterDeclaration, ValueDeclaration};

/// One private copy born on this selected target. Its exact value handle
/// distinguishes the saved owner from every authored by-value read.
pub(in crate::unit::attached_unit::composed_control::state_graph) struct CompletedCopy {
    binding: crate::emission::operation_emission::buffer::ComputedValueBinding,
}

/// Selected edges share one payload producer, after their decision and before
/// their operands. The exact outgoing target keeps sibling captures separate.
pub(in crate::unit::attached_unit::composed_control::state_graph) struct DispatchCopies<'a, 'c> {
    checked: &'a CheckedTrees,
    state: &'a CheckedComposedUnitControlStatePlan,
    owner: symbols::SymbolHandle,
    catalogs: &'a mut catalogs::ComposedCatalogs<'c>,
    erased_parameters: &'a [ValueDeclaration],
}

impl<'a, 'c> DispatchCopies<'a, 'c> {
    pub(in crate::unit::attached_unit::composed_control::state_graph) fn new(
        checked: &'a CheckedTrees,
        state: &'a CheckedComposedUnitControlStatePlan,
        owner: symbols::SymbolHandle,
        catalogs: &'a mut catalogs::ComposedCatalogs<'c>,
        erased_parameters: &'a [ValueDeclaration],
    ) -> Self {
        Self {
            checked,
            state,
            owner,
            catalogs,
            erased_parameters,
        }
    }

    pub(in crate::unit::attached_unit::composed_control::state_graph) fn capture(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        copied_payloads: &[checked_trees::CheckedSelectedPayloadCopy],
        parameters: &[StructuralParameterDeclaration],
        frame: FrameValues<'_>,
        sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
    ) -> Result<Vec<CompletedCopy>, LoweringError> {
        let checked = self.checked;
        let state = self.state;
        let owner = self.owner;
        let FrameValues {
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            ..
        } = frame;
        let copies = crate::scalar_lowering::branch_states::structural_values::selected_copies(
            identities,
            checked,
            copied_payloads,
            &self.catalogs.structural_types,
        )?;
        let mut completed = Vec::new();
        for copy in copies {
            let structural_type = copy.structural_type;
            StateFrame::new(checked, owner, state, self.erased_parameters).evaluate(
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
                        copy.value,
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
            let binding = evaluation
                .saved_value_binding(copy.value, operations)?
                .clone();
            if binding.structural_type != structural_type {
                return unsupported("selected copy has a different realized carrier");
            }
            completed.push(CompletedCopy { binding });
        }
        Ok(completed)
    }
}

/// Copy and DescriptorCleanup authorize only this private value's death.
pub(in crate::unit::attached_unit::composed_control::state_graph) fn complete_dispatch_copies(
    copies: &[CompletedCopy],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<crate::unit::unit_cleanup::CleanupRegion>, LoweringError> {
    let mut regions = Vec::new();
    for copy in copies.iter().rev() {
        // Ordinary cleanup tracks the physical carrier's multiplicity and
        // selected residuals. Source Copy never grants disposal of another
        // root, a nominal hook, or a borrowed pointee.
        regions.extend(evaluation.normal_discard_regions(
            copy.binding.argument.place,
            types,
            operations,
        )?);
    }
    Ok(regions)
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
        let state = &self.plan.states[position];
        let Some(transition) = self
            .checked
            .facts
            .executable_bindings
            .transition(state.state, statement_ordinal as usize)
        else {
            return Ok(Vec::new());
        };
        let key = (
            statement_ordinal,
            transition.primary.arena_index(),
            transition.primary.generation(),
        );
        let selected = state
            .selected_copies
            .binary_search_by_key(&key, |row| {
                (
                    row.statement_ordinal,
                    row.transition_target.arena_index(),
                    row.transition_target.generation(),
                )
            })
            .ok()
            .and_then(|index| state.selected_copies.get(index))
            .ok_or(LoweringError::Unsupported(
                "selected edge lost its checked payload births",
            ))?;
        DispatchCopies::new(
            self.checked,
            state,
            self.admitted.authored[position].0.symbol,
            self.catalogs,
            &self.state_erased[position],
        )
        .capture(
            identities,
            &selected.values,
            parameters,
            FrameValues {
                source_value_count: values.len(),
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            },
            sequence,
        )
    }

    pub(super) fn complete_dispatch_copies(
        &self,
        copies: &[CompletedCopy],
        evaluation: &Evaluation,
        operations: &OperationBuffer,
    ) -> Result<Vec<crate::unit::unit_cleanup::CleanupRegion>, LoweringError> {
        complete_dispatch_copies(
            copies,
            evaluation,
            operations,
            &self.catalogs.structural_types,
        )
    }
}

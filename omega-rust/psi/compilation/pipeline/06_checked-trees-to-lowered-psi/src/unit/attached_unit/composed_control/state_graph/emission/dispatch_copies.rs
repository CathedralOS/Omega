//! Materialize the selected arm's checked payload values before its operands.
//! They use the ordinary copied-value producer and expression-root namespace;
//! no live storage read may substitute the former source after sibling effects.

use super::super::{CheckedComposedUnitControlStatePlan, CheckedTrees, catalogs};
use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::shared::checked_facts::{flow_facts, value_facts};
use crate::shared::lowering_error::{LoweringError, unsupported};
use crate::shared::terminal_identities::stored_structural_identity;
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

    /// The checked return selects its original target, including continuation.
    /// Neither source syntax nor the primary arm can substitute that identity.
    pub(in crate::unit::attached_unit::composed_control::state_graph) fn capture_return(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        destination: &checked_trees::CheckedScalarBranchDestination,
        parameters: &[StructuralParameterDeclaration],
        frame: FrameValues<'_>,
        sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
    ) -> Result<Vec<CompletedCopy>, LoweringError> {
        let checked_trees::CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            return unsupported("selected return has no checked return destination");
        };
        let Some(transition) = self
            .checked
            .facts
            .executable_bindings
            .transition(self.state.state, *statement_ordinal as usize)
        else {
            if *is_continuation {
                return unsupported("selected continuation lost its checked transition");
            }
            // A body's final expression has no selected transition payloads.
            return Ok(Vec::new());
        };
        let target = if *is_continuation {
            transition.continuation
        } else {
            transition.primary
        };
        if !target.is_valid() {
            return unsupported("selected return lost its exact checked target");
        }
        self.capture(
            identities,
            *statement_ordinal,
            target,
            parameters,
            frame,
            sequence,
        )
    }

    pub(in crate::unit::attached_unit::composed_control::state_graph) fn capture(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_ordinal: u32,
        target: typed_trees::statement::TransitionTargetHandle,
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
        let Some(flow_state) = checked.facts.flow.control.unique_state(owner, state.state) else {
            return Ok(Vec::new());
        };
        let mut outgoing_rows = flow_facts(checked)
            .control
            .outgoing
            .span_or_empty(flow_state.outgoing)
            .iter()
            .filter(|outgoing| {
                outgoing.statement_index == statement_ordinal as usize
                    && outgoing.transition_target == target
            });
        let Some(outgoing) = outgoing_rows.next() else {
            return Ok(Vec::new());
        };
        if outgoing_rows.next().is_some() {
            return unsupported("selected payload entry has ambiguous completed flow");
        }
        let copies = flow_facts(checked)
            .control
            .operands
            .span_or_empty(outgoing.copied_payloads);
        let mut completed = Vec::new();
        for copy in copies {
            if checked
                .primitive_type_reference(copy.type_reference)
                .is_some()
                && validation::reference_shell(&checked.typed, copy.type_reference).is_none()
            {
                continue;
            }
            let root = value_facts(checked)
                .structural_values
                .root_for_captured_copy(
                    state.state,
                    statement_ordinal,
                    match copy.completion {
                        checked_trees::FlowOperandCompletion::Copied { copy, .. } => copy,
                        _ => {
                            return unsupported(
                                "selected payload lost its completed copy identity",
                            );
                        }
                    },
                )
                .ok_or(LoweringError::Unsupported(
                    "selected structural payload has no checked copied-value root",
                ))?;
            if !validation::type_satisfies_declared_property(
                &checked.typed,
                &[],
                root.type_reference,
                validation::DeclaredPropertyRequirement::Copy,
            ) || !validation::has_contents(
                &checked.typed,
                root.type_reference,
                &[],
                validation::ContentsRequirement::DescriptorCleanup,
            ) {
                return unsupported(
                    "selected copied value lost its independent copy/discard capability",
                );
            }
            let structural_type = crate::shared::terminal_identities::lookup_type_id(
                &self.catalogs.type_ids,
                &stored_structural_identity(checked, root.type_reference),
            )?;
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
        DispatchCopies::new(
            self.checked,
            state,
            self.admitted.authored[position].0.symbol,
            self.catalogs,
            &self.state_erased[position],
        )
        .capture(
            identities,
            statement_ordinal,
            transition.primary,
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

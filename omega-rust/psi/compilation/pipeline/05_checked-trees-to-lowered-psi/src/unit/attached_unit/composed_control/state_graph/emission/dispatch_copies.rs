//! Materialize the selected arm's checked payload values before its operands.
//! They use the ordinary copied-value producer and expression-root namespace;
//! no live storage read may substitute the former source after sibling effects.

use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::lowering_error::{LoweringError, unsupported};
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use terminal_psi::{StructuralParameterDeclaration, ValueDeclaration};

/// One private copy born on this selected target. Its exact value handle
/// distinguishes the saved owner from every authored by-value read.
pub(super) struct CompletedCopy {
    value: checked_trees::CheckedStructuralValueHandle,
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
        let Some(transition) = checked
            .statement_table
            .statement_at(
                checked
                    .state_by_symbol(state.state)
                    .ok_or(LoweringError::Unsupported(
                        "selected payload state disappeared",
                    ))?,
                statement_ordinal as usize,
            )
            .and_then(|statement| statement.as_transition())
        else {
            return Ok(Vec::new());
        };
        let Some(flow_state) = checked.facts.flow.control.unique_state(owner, state.state) else {
            return Ok(Vec::new());
        };
        let mut outgoing_rows = checked
            .facts
            .flow
            .control
            .outgoing
            .span_or_empty(flow_state.outgoing)
            .iter()
            .filter(|outgoing| {
                outgoing.statement_index == statement_ordinal as usize
                    && outgoing.transition_target == transition.target
            });
        let Some(outgoing) = outgoing_rows.next() else {
            return Ok(Vec::new());
        };
        if outgoing_rows.next().is_some() {
            return unsupported("selected payload entry has ambiguous completed flow");
        }
        let copies = checked
            .facts
            .flow
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
            let root = checked
                .facts
                .values
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
            if root.machine != owner || root.type_reference != copy.type_reference {
                return unsupported(
                    "selected structural payload root changed its completed carrier",
                );
            }
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
            let structural_type = crate::terminal_identities::lookup_type_id(
                &self.catalogs.type_ids,
                &validation::stored_structural_type_identity(
                    &checked.typed,
                    root.type_reference,
                    &[],
                    &[],
                ),
            )?;
            let (mut operation_catalogs, scalar_calls) = self.catalogs.operation_catalogs();
            let mut calls = scalar_calls.emission_context();
            {
                let (mut environment, emission) = operation_catalogs
                    .operation_frame(
                        checked,
                        owner,
                        state,
                        parameters,
                        &self.state_erased[position],
                        sequence,
                        crate::unit::attached_unit::operation_frame::OperationEmission {
                            source_value_count: values.len(),
                            evaluation,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            calls: &mut calls,
                            operations,
                        },
                    )
                    .split();
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
                )?;
            }
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
            completed.push(CompletedCopy {
                value: root.root,
                binding,
            });
            self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
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
            let current = evaluation.saved_value_binding(copy.value, operations)?;
            if current.argument != copy.binding.argument
                || current.structural_type != copy.binding.structural_type
            {
                return unsupported("selected copied value changed its completed producer name");
            }
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

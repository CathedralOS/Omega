//! Result completion: the value-join that commits one selected place to
//! its continuation, and whole-loan completion. Only the selected value moves;
//! unselected candidate occurrences retain their original homes and death order.

use super::super::super::super::{StructuralAccess, StructuralArgument};
use super::super::super::{LoweringError, PlaceId, unsupported};
use super::{
    CompletionOccurrence, Emission, NormalDeathReceipt, ValueCompletion, ValueContinuation,
};
use crate::expression_preparation::bindings::LoanAccess;

impl Emission<'_, '_, '_> {
    /// Finish the actual value operation once: retain its produced custody,
    /// apply its checked content transaction, capture its exact claims, then
    /// end the checked source complement before any consuming edge.
    pub(super) fn finish_value_completion(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: checked_trees::CheckedStructuralValueHandle,
        produced: ValueCompletion,
    ) -> Result<
        (
            super::super::operand_claims::CompletedOperand,
            NormalDeathReceipt,
        ),
        LoweringError,
    > {
        let (mut argument, occurrence, previous) = match produced {
            ValueCompletion::NoNormal => {
                return super::unsupported("normal value completion has no arriving operand");
            }
            ValueCompletion::Awaiting {
                argument,
                occurrence,
            } => (argument, occurrence, None),
            ValueCompletion::Complete { argument, receipt } => {
                let occurrence = receipt.occurrence;
                (argument, occurrence, Some(receipt))
            }
        };
        let node = self.structural_value_node(value);
        if occurrence.value != value
            || occurrence.first_new_place > *self.environment.next_place
            || node.machine != self.machine
            || node.state != self.state
            || node.statement_index != self.statement
        {
            return unsupported("normal death belongs to another completed value occurrence");
        }
        if self
            .evaluation
            .retired_structural_homes
            .contains(&argument.place)
        {
            return unsupported("normal death has a retired completed value source");
        }
        if let Some(receipt) = previous {
            if receipt.source != argument || receipt.block != self.evaluation.current {
                return unsupported("normal-death receipt names another source or execution frame");
            }
            // A receiving join has current names of its own. The receipt only
            // prevents replay of death; it cannot replace this custody query.
            let completed = self.capture_operand(
                identities,
                argument,
                node.expression,
                occurrence.first_new_place,
            )?;
            return Ok((completed, receipt));
        }
        if argument.access == StructuralAccess::Owned && argument.path.is_empty() {
            self.completed(identities, argument.place, occurrence, None)?;
        }
        if let Some(transaction) = self.content_transformation(&node)? {
            argument = self.transform_content(identities, argument, transaction)?;
            self.completed(identities, argument.place, occurrence, None)?;
        }
        let completed = self.capture_operand(
            identities,
            argument.clone(),
            node.expression,
            occurrence.first_new_place,
        )?;
        self.complete_value_cleanup(identities, node.expression)?;
        if self
            .evaluation
            .retired_structural_homes
            .contains(&argument.place)
        {
            return unsupported("normal death retired its completed result source");
        }
        let receipt = NormalDeathReceipt {
            occurrence,
            source: argument,
            block: self.evaluation.current,
        };
        Ok((completed, receipt))
    }

    /// Forward a completed normal-death acknowledgement only through the
    /// actual edge that transports its captured operand. The receiving home
    /// is recorded with that edge's block, never inferred from equal types.
    pub(super) fn finish_value_arrival(
        &mut self,
        completed: super::super::operand_claims::CompletedOperand,
        receipt: NormalDeathReceipt,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<ValueCompletion, LoweringError> {
        if receipt.source != completed.argument
            || receipt.block != self.evaluation.current
            || self
                .evaluation
                .retired_structural_homes
                .contains(&completed.argument.place)
        {
            return unsupported("completed value arrival lost its normal-death source");
        }
        let (argument, block) = if let Some(continuation) = continuation {
            let receiving_access = continuation
                .structural_parameters
                .first()
                .ok_or(LoweringError::Unsupported(
                    "completed value arrival has no receiving source slot",
                ))?
                .access;
            self.complete_value(completed, continuation)?;
            (
                StructuralArgument::whole(continuation.place, receiving_access),
                continuation.block,
            )
        } else {
            (completed.argument, self.evaluation.current)
        };
        Ok(ValueCompletion::Complete {
            receipt: NormalDeathReceipt {
                occurrence: receipt.occurrence,
                source: argument.clone(),
                block,
            },
            argument,
        })
    }

    /// End the exact checked owners after capturing an observation or moving
    /// an owned subtree, before handing its result to the value continuation.
    /// Scalar observations use this same roster, including backing hooks.
    pub(super) fn complete_value_cleanup(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Result<(), LoweringError> {
        let control = &self
            .checked
            .facts
            .flow
            .terminal_unit_effects
            .cleanup_control;
        let Some(completion) =
            control.value_completion(self.machine, self.state, self.statement, expression)
        else {
            return Ok(());
        };
        let actions = crate::unit::unit_cleanup::expression_cleanup_regions(
            identities,
            self.checked,
            control.actions.span_or_empty(completion.actions),
            &self.evaluation.structural_locals,
            |place| self.evaluation.carrier_multiplicity(place, self.operations),
            self.calls,
        )?;
        let mut retired = Vec::new();
        for action in &actions {
            let place = action.action.place();
            if action.action.path().is_empty() && !retired.contains(&place) {
                retired.push(place);
            }
        }
        self.evaluation.continue_cleanup_regions(
            identities,
            self.checked,
            actions,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )?;
        for place in retired {
            self.evaluation.retire_structural_home(place);
        }
        Ok(())
    }

    /// Publication is attached to this actual completed value occurrence.
    /// The immutable output catalog can name a fresh producer only inside the
    /// allocator interval this occurrence itself emitted; it cannot fill a
    /// missing old/sibling frame home.
    pub(super) fn admit_fresh_operand(
        &mut self,
        place: PlaceId,
        completion: CompletionOccurrence,
    ) -> Result<(), LoweringError> {
        if self.evaluation.current_claims.contains(place) {
            return Ok(());
        }
        if place.get() < completion.first_new_place {
            return super::super::super::unsupported(
                "completed operand is absent from its execution frame",
            );
        }
        let argument = StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        };
        let claims = match self.operations.declared_source_claims(&argument)? {
            Some(claims) => claims,
            None if crate::emission::claim_bindings::completed_value_paths(
                self.checked,
                completion.value,
            )?
            .is_empty() =>
            {
                Vec::new()
            }
            None => {
                return super::super::super::unsupported(
                    "fresh completed operand has no producer custody namespace",
                );
            }
        };
        self.evaluation.current_claims.publish(place, claims)
    }

    /// Completes `place` against `continuation` when one is armed and hands
    /// the place back — the tail every already-built structural value shares.
    pub(super) fn completed(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        place: PlaceId,
        completion: CompletionOccurrence,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        self.admit_fresh_operand(place, completion)?;
        // Forwarded homes retain their original birth. Only an operation
        // result emitted by this completion receives this source coordinate;
        // owned join arrivals register at their actual continuation owner.
        if place.get() >= completion.first_new_place
            && self.operations.structural_result_contract(place).is_some()
        {
            self.operations.register_structural_materialization(
                place,
                self.state,
                self.statement,
            )?;
        }
        if let Some(continuation) = continuation {
            self.complete_operand(
                identities,
                StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: self.result_access,
                },
                completion,
                continuation,
            )?;
        }
        Ok(place)
    }

    /// Capture this exact completed operand before transporting its owned
    /// value. Existing homes use their consuming event; fresh producers supply
    /// their emitted custody. A declaration roster cannot replace that query.
    pub(super) fn complete_operand(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        selected: StructuralArgument,
        completion: CompletionOccurrence,
        continuation: &mut ValueContinuation,
    ) -> Result<(), LoweringError> {
        let expression = self.structural_value_node(completion.value).expression;
        let completed =
            self.capture_operand(identities, selected, expression, completion.first_new_place)?;
        self.complete_value(completed, continuation)
    }

    pub(super) fn complete_value(
        &mut self,
        completed: super::super::operand_claims::CompletedOperand,
        continuation: &mut ValueContinuation,
    ) -> Result<(), LoweringError> {
        let super::super::operand_claims::CompletedOperand {
            argument: selected,
            claims,
        } = completed;
        // Only the selected subtree crosses this edge. Its untouched siblings
        // remain at their original homes until the checked normal-death graph;
        // a value join does not replace nominal hooks with an early discard.
        let structural_arguments = vec![selected];
        if structural_arguments.len() != continuation.structural_parameters.len() {
            return unsupported(
                "structural value has unequal source and receiving slots at its join",
            );
        }
        if let Some(common) = &mut continuation.common_claims {
            common.retain(|claim| claims.contains(claim));
        } else {
            continuation.common_claims = Some(claims);
        }
        self.join_memberships(&structural_arguments, continuation)?;
        let edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            structural_arguments,
        )?;
        self.bind_continuation_names(continuation, edge.edge)?;
        self.evaluation
            .retire_consumed_structural_arguments(&edge.structural_arguments, self.operations)?;
        self.finish_jump(edge, Vec::new());
        Ok(())
    }

    /// A borrowed branch joins one referent place at the result parameter
    /// under its loan. The referent's owner stays live; only the reference
    /// carrier is selected. The single structural parameter must be the result
    /// slot of the loan's access this edge's argument fills. A shared slot is
    /// also `Unrestricted`; an exclusive slot keeps the multiplicity the
    /// binding declares.
    pub(super) fn complete_loan(
        &mut self,
        argument: StructuralArgument,
        continuation: &mut ValueContinuation,
        loan: LoanAccess,
    ) -> Result<(), LoweringError> {
        if continuation.structural_parameters.len() != 1 {
            return unsupported(match loan {
                LoanAccess::Shared => "borrowed selection joined residual ownership",
                LoanAccess::Mutable => "mutable borrowed selection joined residual ownership",
            });
        }
        self.join_memberships(std::slice::from_ref(&argument), continuation)?;
        let edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            vec![argument],
        )?;
        self.bind_continuation_names(continuation, edge.edge)?;
        self.finish_jump(edge, Vec::new());
        Ok(())
    }

    /// Stationary storage receives the checked edge's destination aliases
    /// before the private frame meet. The completed value's separate receiving
    /// slot still binds only the actual operand claims captured above.
    fn bind_continuation_names(
        &mut self,
        continuation: &mut ValueContinuation,
        edge: semantic_vocabulary::EdgeId,
    ) -> Result<(), LoweringError> {
        let aliases = crate::emission::claim_bindings::selections::edge_bindings(
            self.checked,
            &continuation.claim_destinations,
            &self.selection_route,
            edge,
            &self.calls.claims,
        )?;
        self.evaluation.current_claims =
            self.evaluation.current_claims.with_edge_aliases(&aliases)?;
        if let Some(current) = &mut continuation.current_claims {
            current.meet(&self.evaluation.current_claims)?;
        } else {
            continuation.current_claims = Some(self.evaluation.current_claims.clone());
        }
        self.operations.claim_edge_bindings.extend(aliases);
        Ok(())
    }

    /// Meet actual incoming contracts at their receiving slots. Whole moves,
    /// projections, and borrowed joins use the same occurrence-relative
    /// transport; no destination annotation supplies a missing predicate.
    /// The permission frontier and cleanup remain the separate edge judgment.
    fn join_memberships(
        &self,
        arguments: &[StructuralArgument],
        continuation: &mut ValueContinuation,
    ) -> Result<(), LoweringError> {
        if arguments.len() != continuation.structural_parameters.len() {
            return unsupported("membership join has unequal source and receiving slots");
        }
        for (argument, parameter) in arguments
            .iter()
            .zip(&mut continuation.structural_parameters)
        {
            let incoming = self.transported_parameter(
                argument,
                parameter.place,
                parameter.position,
                parameter.structural_type,
                parameter.multiplicity,
            )?;
            if continuation.has_incoming_memberships {
                parameter
                    .qualifications
                    .retain(|domain| incoming.qualifications.contains(domain));
                parameter
                    .projected_qualifications
                    .retain(|domain| incoming.projected_qualifications.contains(domain));
            } else {
                parameter.qualifications = incoming.qualifications;
                parameter.projected_qualifications = incoming.projected_qualifications;
            }
            if let Some(owner) = continuation
                .remaining_owners
                .iter_mut()
                .find(|owner| owner.value.place == parameter.place)
            {
                owner.value.qualifications = parameter.qualifications.clone();
                owner.value.projected_qualifications = parameter.projected_qualifications.clone();
            }
        }
        continuation.has_incoming_memberships = true;
        Ok(())
    }
}

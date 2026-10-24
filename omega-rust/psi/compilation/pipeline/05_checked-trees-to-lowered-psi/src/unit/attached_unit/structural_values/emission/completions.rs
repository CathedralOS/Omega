//! Result completion: the value-join that commits one selected place to
//! its continuation, and whole-loan completion. Only the selected value moves;
//! unselected candidate occurrences retain their original homes and death order.

use super::super::super::super::StructuralArgument;
use super::super::super::{LoweringError, PlaceId, unsupported};
use super::{Emission, ValueContinuation};
use crate::expression_preparation::bindings::LoanAccess;

impl Emission<'_, '_, '_> {
    /// End the exact checked owners after the observation completes, before
    /// handing its result to the value continuation. Scalar observations use
    /// the same action roster, including nominal hooks on backing owners.
    pub(super) fn complete_observation(
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
            if !retired.contains(&place) {
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

    /// Completes `place` against `continuation` when one is armed and hands
    /// the place back — the tail every already-built structural value shares.
    pub(super) fn completed(
        &mut self,
        place: PlaceId,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        // These callers just emitted storage or an owned result arrival. An
        // alias uses complete_value/complete_loan without asserting a new birth.
        self.operations
            .register_structural_materialization(place, self.state, self.statement)?;
        if let Some(continuation) = continuation {
            let completed = self.published_operand(StructuralArgument {
                place,
                path: Vec::new(),
                access: self.result_access,
            })?;
            self.complete_value(completed, continuation)?;
        }
        Ok(place)
    }

    pub(super) fn complete_value(
        &mut self,
        completed: super::super::operand_claims::CompletedOperand,
        continuation: &mut ValueContinuation,
    ) -> Result<(), LoweringError> {
        // Only the selected subtree crosses the join. Its original owner's
        // complement stays resident until the checked contextual death; shape
        // geometry cannot substitute an early discard for nominal hooks.
        let unchanged = completed.claims.len() == continuation.destination_claims.len()
            && completed.claims.iter().all(|incoming| {
                continuation
                    .destination_claims
                    .iter()
                    .any(|destination| incoming == destination)
            });
        continuation.requires_claim_bindings |= !unchanged;
        let structural_arguments = vec![completed.argument];
        if structural_arguments.len() != continuation.structural_parameters.len() {
            return unsupported(
                "structural value has unequal source and receiving slots at its join",
            );
        }
        self.join_memberships(&structural_arguments, continuation)?;
        let edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            structural_arguments,
        )?;
        self.operations.claim_edge_bindings.extend(
            crate::emission::claim_bindings::selections::edge_bindings(
                self.checked,
                &continuation.claim_destinations,
                &self.selection_route,
                edge.edge,
                &self.calls.claims,
            )?,
        );
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
        self.operations.claim_edge_bindings.extend(
            crate::emission::claim_bindings::selections::edge_bindings(
                self.checked,
                &continuation.claim_destinations,
                &self.selection_route,
                edge.edge,
                &self.calls.claims,
            )?,
        );
        self.finish_jump(edge, Vec::new());
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

//! A final call must finish using local data before that data is released.
//! Follow each returning branch separately: a call can consume a holder on
//! one branch while another branch leaves it for checked exit cleanup.

use super::{BorrowLoanFact, Handle, Retirement, RetirementNode};
use checked_trees::{
    CheckedScalarBranchDestination, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, FlowClaimJoinExit, FlowClaimJoinExitKind,
};
use language_semantics::PermissionEventSource;
use typed_trees::types::TypeReferenceHandle;

impl Retirement<'_> {
    pub(super) fn completion_retires_holder(
        &self,
        loan: Handle<BorrowLoanFact>,
        source: CheckedUnitStructuralArgumentSourcePlan,
        formation: u32,
        selection: &[CheckedUnitStructuralPathSegment],
        reference: TypeReferenceHandle,
        active: &mut Vec<RetirementNode>,
    ) -> bool {
        let Some(completion) = self.scalar_completion.filter(|_| self.at_exit) else {
            return false;
        };
        let transfers = match crate::execution::terminal_unit::storage_transfers::at_completion(
            self.program,
            self.facts,
            self.prepared_calls,
            self.results,
            source,
            formation,
            selection,
            self.operations,
            completion,
        ) {
            Ok(transfers) => transfers,
            Err(reason) => {
                self.trace.phase(reason);
                return false;
            }
        };
        if transfers.returns.is_empty() {
            self.trace
                .phase("completion cleanup: the return plan has no branches");
            return false;
        }
        let Ok(exits) = crate::execution::terminal_unit::cleanup::normal_edges::state_exits(
            self.program,
            self.facts,
            self.machine,
            self.state,
        ) else {
            self.trace
                .phase("completion cleanup: the body has no checked return exits");
            return false;
        };
        transfers.returns.iter().all(|(destination, returned)| {
            let CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = destination
            else {
                return false;
            };
            let mut matching = exits.iter().filter(|exit| {
                exit.statement_index == *statement_ordinal as usize
                    && if *is_continuation {
                        exit.kind == FlowClaimJoinExitKind::TransitionContinuation
                    } else {
                        matches!(
                            exit.kind,
                            FlowClaimJoinExitKind::TransitionTarget
                                | FlowClaimJoinExitKind::FinalExpression
                        )
                    }
            });
            let Some(exit) = matching.next() else {
                self.trace
                    .phase("completion cleanup: the selected return has no checked exit");
                return false;
            };
            matching.next().is_none()
                && self.transferred_holder_retired(
                    loan,
                    reference,
                    &[&transfers.prefix, returned],
                    Some(*exit),
                    active,
                )
        })
    }

    /// A value can remain in storage after its last read. Use the completed
    /// storage snapshot to identify it at cleanup; the active-loan inventory
    /// has already expired loans that no later expression needs to read.
    pub(super) fn exit_cleanup_retires_holder(
        &self,
        handle: Handle<BorrowLoanFact>,
        reference: TypeReferenceHandle,
        exit: FlowClaimJoinExit,
    ) -> bool {
        if !self.at_exit {
            return false;
        }
        let loan = self.facts.borrow.loans.get(handle);
        if loan.storage_owner == self.uncommitted_owner {
            self.trace
                .phase("completion cleanup: this owner still needs its saved access");
            return false;
        }
        let Some(path) = super::transport::carrier_path(
            self.program,
            reference,
            self.facts.borrow.loan_owner_path(loan),
        ) else {
            return false;
        };
        let Some(path) = super::loan_place_segments(self.program, reference, &path) else {
            return false;
        };
        let Some(state) = self
            .facts
            .flow
            .control
            .state(self.machine, self.state.symbol)
        else {
            self.trace
                .phase("completion cleanup: the body has no retained flow state");
            return false;
        };
        let mut completed = self
            .facts
            .flow
            .control
            .exits
            .span_or_empty(state.exits)
            .iter()
            .filter(|row| {
                row.machine_symbol == self.machine
                    && row.state_symbol == exit.state_symbol
                    && row.statement_index == exit.statement_index
                    && row.is_continuation
                        == (exit.kind == FlowClaimJoinExitKind::TransitionContinuation)
            })
            .peekable();
        let control = &self.facts.flow.control;
        let storage = validation::CanonicalPlace {
            root: loan.storage_owner,
            segments: path.clone(),
        };
        if completed.peek().is_none()
            || !completed.all(|row| {
                if !control.operands.is_valid(row.returned_value) {
                    return false;
                }
                let value = control.operands.get(row.returned_value);
                let Some(stored) = validation::completed_storage::at_snapshot(
                    self.program,
                    control,
                    value.storage_snapshot,
                    &storage,
                    validation::completed_storage::StoredObservation::Binding,
                )
                .and_then(|stored| stored.captures()) else {
                    return false;
                };
                !stored.is_empty()
                    && stored.iter().all(|stored| {
                        validation::completed_storage::descriptor_formation_matches_loan(
                            self.program,
                            control,
                            &self.facts.borrow,
                            self.state.symbol,
                            state.statements,
                            loan,
                            stored.capture,
                            &stored.projection,
                        ) == Some(true)
                    })
            })
        {
            self.trace.phase("completion cleanup: the selected return has no matching stored occurrence of this saved access");
            self.trace
                .statement(u32::try_from(exit.statement_index).ok());
            return false;
        }
        let covered = self.facts.contextual_cleanup.any_state_disposition(
            self.machine,
            self.state.symbol,
            |_, disposition| {
                if disposition.source != PermissionEventSource::StateExit
                    || disposition.exit != exit
                    || disposition.root != loan.storage_owner
                    || disposition.zero_image
                {
                    return false;
                }
                let Some(selected) = self.facts.flow.ownership.segments.span(disposition.path) else {
                    return false;
                };
                if selected.len() > path.len()
                    || !selected.iter().zip(&path).all(|(left, right)| {
                        validation::storage_path_relations::canonical_place_segments_equal(self.program, *left, *right)
                    })
                {
                    return false;
                }
                let Some(excluded) = self.facts.contextual_cleanup.exclusions.span(disposition.exclusions) else {
                    return false;
                };
                excluded.iter().all(|excluded| {
                    excluded.root != loan.storage_owner
                        || self.facts.contextual_cleanup.segments.span(excluded.segments)
                            .is_some_and(|segments| {
                                !validation::storage_path_relations::canonical_place_segments_may_overlap(self.program, segments, &path)
                            })
                })
            },
        );
        if !covered {
            self.trace
                .phase("completion cleanup: no checked cleanup covers this saved access");
            self.trace
                .statement(u32::try_from(exit.statement_index).ok());
        }
        covered
    }
}

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

struct CompletedDescriptorSources {
    formations: Vec<Handle<BorrowLoanFact>>,
    call_values: Vec<validation::completed_storage::StoredCapture>,
}

impl Retirement<'_> {
    /// The final slot can contain a descriptor from any completed branch,
    /// including the old value on a skipped-call path. Follow the exact
    /// predecessors recorded by checking; equal backing is not identity.
    fn completed_descriptor_sources(
        &self,
        handle: Handle<BorrowLoanFact>,
    ) -> Option<CompletedDescriptorSources> {
        let borrow = &self.facts.borrow;
        let loan = borrow.loans.get(handle);
        let owner_path = borrow.owner_segments.span(loan.owner_path)?;
        let mut pending = self
            .resource_loans()?
            .filter_map(|(handle, candidate)| {
                (candidate.storage_owner == loan.storage_owner
                    && candidate.statement_index == loan.statement_index
                    && borrow.owner_segments.span(candidate.owner_path) == Some(owner_path))
                .then_some((handle, false))
            })
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return None;
        }
        let Some(Some(state)) =
            super::unique_borrow_state(self.facts, self.machine, self.state.symbol)
        else {
            return None;
        };
        let mut active = Vec::new();
        let mut completed = Vec::new();
        let mut output = CompletedDescriptorSources {
            formations: Vec::new(),
            call_values: Vec::new(),
        };
        while let Some((source, leaving)) = pending.pop() {
            if leaving {
                active.pop();
                completed.push(source);
                continue;
            }
            if completed.contains(&source) {
                continue;
            }
            if active.contains(&source) || !borrow.state_owns_loan(state, source) {
                return None;
            }
            active.push(source);
            pending.push((source, true));
            let occurrence = borrow.loans.get(source);
            if occurrence.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                // Lifetime requirements still keep their parents alive, but
                // they cannot identify the pointer stored in this slot.
                continue;
            }
            if occurrence.established_by_call.is_valid() {
                output
                    .call_values
                    .extend(self.call_descriptor_values(source)?);
            } else if occurrence.formation_expression.is_valid()
                && occurrence.formation_owner_path.is_some()
            {
                output.formations.push(source);
            } else {
                let sources = borrow.loan_suppliers.span(occurrence.supplying_loans)?;
                if sources.is_empty() {
                    return None;
                }
                for source in sources {
                    if !borrow.state_owns_loan(state, *source) {
                        return None;
                    }
                    let previous = borrow.loans.get(*source);
                    if previous.storage_owner != occurrence.storage_owner
                        || borrow.owner_segments.span(previous.owner_path)
                            != borrow.owner_segments.span(occurrence.owner_path)
                        || previous.kind != occurrence.kind
                    {
                        return None;
                    }
                    pending.push((*source, false));
                }
            }
        }
        Some(output)
    }

    /// A call-written descriptor is the value in that exact invocation's
    /// completed slot, not an expression with the same statement number.
    fn call_descriptor_values(
        &self,
        handle: Handle<BorrowLoanFact>,
    ) -> Option<Vec<validation::completed_storage::StoredCapture>> {
        let borrow = &self.facts.borrow;
        let loan = borrow.loans.get(handle);
        if !borrow.calls.is_valid(loan.established_by_call) {
            return None;
        }
        let call = borrow.calls.get(loan.established_by_call);
        if borrow.call_at(
            self.machine,
            self.state.symbol,
            call.statement_index,
            call.call_ordinal,
        ) != Some(loan.established_by_call)
            || !borrow.loans_contain(call.completed_reference_loans, handle)
            || !borrow
                .reference_slot_effects
                .span(call.reference_slot_effects)?
                .iter()
                .any(|slot| {
                    slot.root == loan.storage_owner
                        && borrow
                            .owner_segments
                            .span(slot.owner_path)
                            .is_some_and(|prefix| borrow.loan_owner_path(loan).starts_with(prefix))
                })
        {
            return None;
        }
        let control = &self.facts.flow.control;
        let state = control.state(self.machine, self.state.symbol)?;
        let mut calls = control.calls.span(state.calls)?.iter().filter(|observed| {
            observed.statement_index == call.statement_index
                && observed.call_ordinal == call.call_ordinal
                && observed.authored_expression == call.authored_expression
        });
        let completed = calls.next()?;
        if calls.next().is_some() {
            return None;
        }
        let reference = super::storage_owner_type_reference(
            self.program,
            self.state,
            loan.owner_symbol,
            u32::try_from(loan.statement_index).ok()?,
        )?;
        let path =
            super::transport::carrier_path(self.program, reference, borrow.loan_owner_path(loan))?;
        let segments = super::loan_place_segments(self.program, reference, &path)?;
        validation::completed_storage::at_snapshot(
            self.program,
            control,
            completed.completed_storage,
            &validation::CanonicalPlace {
                root: loan.storage_owner,
                segments,
            },
            validation::completed_storage::StoredObservation::Binding,
        )?
        .captures()
    }

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
        let Some(sources) = self.completed_descriptor_sources(handle) else {
            return false;
        };
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
        let call_descriptors = if loan.formation_owner_path.is_none() {
            match validation::completed_storage::loan_call_descriptors(
                self.program,
                self.facts,
                self.machine,
                self.state.symbol,
                handle,
            ) {
                Ok(Some(descriptors)) => Some(descriptors),
                Ok(None) => return false,
                Err(reason) => {
                    self.trace.phase(reason);
                    return false;
                }
            }
        } else {
            None
        };
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
                let Some(stored) = validation::completed_storage::sources_at_snapshot(
                    self.program,
                    control,
                    value.storage_snapshot,
                    &storage,
                    validation::completed_storage::StoredObservation::Binding,
                ) else {
                    return false;
                };
                if let Some(descriptors) = &call_descriptors {
                    return validation::completed_storage::matches_call_descriptor_frontier(
                        self.program,
                        stored,
                        descriptors,
                    ) == Ok(true);
                }
                let Some(stored) = stored.captures() else {
                    return false;
                };
                !stored.is_empty()
                    && stored.iter().all(|stored| {
                        sources.call_values.iter().any(|source| {
                            source.capture == stored.capture
                                && source.projection == stored.projection
                        }) || sources.formations.iter().any(|source| {
                            validation::completed_storage::descriptor_formation_matches_loan(
                                self.program,
                                control,
                                &self.facts.borrow,
                                self.state.symbol,
                                state.statements,
                                self.facts.borrow.loans.get(*source),
                                stored.capture,
                                &stored.projection,
                            ) == Some(true)
                        })
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
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE saved-exit-cleanup-uncovered machine={:?} state={:?} loan={handle:?} row={loan:?} path={path:?} exit={exit:?}",
                    self.machine, self.state.symbol
                );
                let mut count = 0;
                let mut printed = 0;
                self.facts.contextual_cleanup.any_state_disposition(self.machine, self.state.symbol, |disposition_handle, disposition| {
                    if count < 12 || disposition.root == loan.storage_owner {
                        eprintln!("CHECKSITE saved-exit-cleanup-candidate loan={handle:?} disposition={disposition_handle:?} row={disposition:?} path={:?} exclusions={:?}", self.facts.flow.ownership.segments.span(disposition.path), self.facts.contextual_cleanup.exclusions.span(disposition.exclusions));
                        printed += 1;
                    }
                    count += 1;
                    false
                });
                eprintln!(
                    "CHECKSITE saved-exit-cleanup-candidate-count loan={handle:?} total={count} printed={printed} all_matching_owners=true"
                );
            }
            self.trace
                .phase("completion cleanup: no checked cleanup covers this saved access");
            self.trace
                .statement(u32::try_from(exit.statement_index).ok());
        }
        covered
    }
}

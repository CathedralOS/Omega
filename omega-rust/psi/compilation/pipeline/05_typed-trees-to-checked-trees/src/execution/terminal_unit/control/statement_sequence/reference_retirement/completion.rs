//! A final call must finish using local data before that data is released.
//! Follow each returning branch separately: a call can consume a holder on
//! one branch while another branch leaves it for checked exit cleanup.

use super::{BorrowLoanFact, Handle, Retirement, RetirementNode};
use checked_trees::{
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment, FlowClaimJoinExit,
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
    ) -> Result<CompletedDescriptorSources, &'static str> {
        let borrow = &self.facts.borrow;
        let loan = borrow.loans.get(handle);
        let owner_path = borrow
            .owner_segments
            .span(loan.owner_path)
            .ok_or("completion cleanup: the descriptor has no retained owner path")?;
        let mut pending = self
            .resource_loans()
            .ok_or("completion cleanup: the body has no unique resource roster")?
            .filter_map(|(handle, candidate)| {
                (candidate.storage_owner == loan.storage_owner
                    && candidate.statement_index == loan.statement_index
                    && borrow.owner_segments.span(candidate.owner_path) == Some(owner_path))
                .then_some((handle, false))
            })
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Err("completion cleanup: the descriptor has no exact slot alternatives");
        }
        let Some(Some(state)) =
            super::unique_borrow_state(self.facts, self.machine, self.state.symbol)
        else {
            return Err("completion cleanup: the body has no unique borrow state");
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
                return Err(
                    "completion cleanup: descriptor predecessors cycle or leave this state",
                );
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
                let sources = borrow
                    .loan_suppliers
                    .span(occurrence.supplying_loans)
                    .ok_or("completion cleanup: descriptor predecessors are unavailable")?;
                if sources.is_empty() {
                    return Err(
                        "completion cleanup: the descriptor has no call or formation predecessor",
                    );
                }
                for source in sources {
                    if !borrow.state_owns_loan(state, *source) {
                        return Err(
                            "completion cleanup: a descriptor predecessor belongs to another state",
                        );
                    }
                    let previous = borrow.loans.get(*source);
                    if previous.storage_owner != occurrence.storage_owner
                        || borrow.owner_segments.span(previous.owner_path)
                            != borrow.owner_segments.span(occurrence.owner_path)
                        || previous.kind != occurrence.kind
                    {
                        return Err(
                            "completion cleanup: a predecessor names different storage or access",
                        );
                    }
                    pending.push((*source, false));
                }
            }
        }
        Ok(output)
    }

    /// A call-written descriptor is the value in that exact invocation's
    /// completed slot, not an expression with the same statement number.
    fn call_descriptor_values(
        &self,
        handle: Handle<BorrowLoanFact>,
    ) -> Result<Vec<validation::completed_storage::StoredCapture>, &'static str> {
        let borrow = &self.facts.borrow;
        let loan = borrow.loans.get(handle);
        if !borrow.calls.is_valid(loan.established_by_call) {
            return Err("completion cleanup: the descriptor has no checked establishing call");
        }
        let call = borrow.calls.get(loan.established_by_call);
        if borrow.call_at(
            self.machine,
            self.state.symbol,
            call.statement_index,
            call.call_ordinal,
        ) != Some(loan.established_by_call)
            || !borrow.loans_contain(call.completed_reference_loans, handle)
        {
            return Err(
                "completion cleanup: the descriptor does not belong to its exact checked call",
            );
        }
        let slots = borrow
            .reference_slot_effects
            .span(call.reference_slot_effects)
            .ok_or("completion cleanup: the checked call has no retained slot effects")?;
        if !slots.iter().any(|slot| {
            slot.root == loan.storage_owner
                && borrow
                    .owner_segments
                    .span(slot.owner_path)
                    .is_some_and(|prefix| borrow.loan_owner_path(loan).starts_with(prefix))
        }) {
            return Err(
                "completion cleanup: the checked call does not establish this descriptor slot",
            );
        }
        let control = &self.facts.flow.control;
        let state = control
            .state(self.machine, self.state.symbol)
            .ok_or("completion cleanup: the body has no retained flow state")?;
        let mut calls = control
            .calls
            .span(state.calls)
            .ok_or("completion cleanup: the body has no retained flow calls")?
            .iter()
            .filter(|observed| {
                observed.statement_index == call.statement_index
                    && observed.call_ordinal == call.call_ordinal
                    && observed.authored_expression == call.authored_expression
            });
        let completed = calls
            .next()
            .ok_or("completion cleanup: the checked call has no exact flow occurrence")?;
        if calls.next().is_some() {
            return Err("completion cleanup: the checked call has ambiguous flow occurrences");
        }
        let statement = u32::try_from(loan.statement_index)
            .map_err(|_| "completion cleanup: the descriptor statement is out of range")?;
        let reference = super::storage_owner_type_reference(
            self.program,
            self.state,
            loan.owner_symbol,
            statement,
        )
        .ok_or("completion cleanup: the descriptor owner type is unavailable")?;
        let path =
            super::transport::carrier_path(self.program, reference, borrow.loan_owner_path(loan))
                .ok_or("completion cleanup: the descriptor owner path is unavailable")?;
        let segments = super::loan_place_segments(self.program, reference, &path)
            .ok_or("completion cleanup: the descriptor storage projection is unavailable")?;
        validation::completed_storage::at_snapshot(
            self.program,
            control,
            completed.completed_storage,
            &validation::CanonicalPlace {
                root: loan.storage_owner,
                segments,
            },
            validation::completed_storage::StoredObservation::Binding,
        )
        .and_then(|stored| stored.captures())
        .ok_or(
            "completion cleanup: the establishing call has no complete stored descriptor captures",
        )
    }

    fn completion_refusal(&self, reason: &'static str, loan: Handle<BorrowLoanFact>) {
        self.trace.phase(reason);
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
            return;
        }
        eprintln!(
            "CHECKSITE completion-cleanup-refusal machine={:?} state={:?} loan={loan:?} reason={reason}",
            self.machine, self.state.symbol
        );
        let borrow = &self.facts.borrow;
        let mut pending = vec![loan];
        let mut visited = Vec::new();
        while let Some(handle) = pending.pop() {
            if visited.contains(&handle) || !borrow.loans.is_valid(handle) {
                continue;
            }
            visited.push(handle);
            let row = borrow.loans.get(handle);
            let suppliers = borrow.loan_suppliers.span(row.supplying_loans);
            eprintln!(
                "CHECKSITE completion-cleanup-predecessor loan={handle:?} occurrence={row:?} owner_path={:?} suppliers={suppliers:?}",
                borrow.owner_segments.span(row.owner_path)
            );
            if let Some(suppliers) = suppliers {
                pending.extend_from_slice(suppliers);
            }
        }
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
        let Some(completion) = self.completion.filter(|_| self.at_exit) else {
            if self.at_exit {
                self.completion_refusal(
                    "completion cleanup: the body has no prepared completion plan",
                    loan,
                );
            }
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
        if transfers.exits.is_empty() {
            self.trace
                .phase("completion cleanup: the completion plan has no normal exits");
            return false;
        }
        let Ok(exits) = crate::execution::terminal_unit::cleanup::normal_edges::state_exits(
            self.program,
            self.facts,
            self.machine,
            self.state,
        ) else {
            self.trace
                .phase("completion cleanup: the body has no checked normal exits");
            return false;
        };
        transfers.exits.iter().all(|(exit, selected)| {
            if !exits.contains(exit) {
                self.trace
                    .phase("completion cleanup: the selected completion has no exact checked exit");
                return false;
            }
            self.transferred_holder_retired(
                loan,
                reference,
                &[&transfers.prefix, selected],
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
        let sources = match self.completed_descriptor_sources(handle) {
            Ok(sources) => sources,
            Err(reason) => {
                self.completion_refusal(reason, handle);
                return false;
            }
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
        let completed =
            match crate::execution::terminal_unit::cleanup::normal_edges::state_exit_storage(
                self.program,
                self.facts,
                self.machine,
                self.state,
                exit,
            ) {
                Ok(snapshots) => snapshots,
                Err(reason) => {
                    self.completion_refusal(reason, handle);
                    return false;
                }
            };
        let control = &self.facts.flow.control;
        let storage = validation::CanonicalPlace {
            root: loan.storage_owner,
            segments: path.clone(),
        };
        if !completed.iter().all(|snapshot| {
            let Some(stored) = validation::completed_storage::at_snapshot(
                self.program,
                control,
                *snapshot,
                &storage,
                validation::completed_storage::StoredObservation::Binding,
            )
            .and_then(|stored| stored.captures()) else {
                if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    eprintln!(
                        "CHECKSITE completion-cleanup-storage-unavailable loan={handle:?} exit={exit:?} snapshot={snapshot:?} storage={storage:?}"
                    );
                }
                return false;
            };
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE completion-cleanup-storage loan={handle:?} exit={exit:?} snapshot={snapshot:?} captures={:?} call_sources={:?} formations={:?}",
                    stored.iter().map(|value| (value.capture, &value.projection)).collect::<Vec<_>>(),
                    sources.call_values.iter().map(|value| (value.capture, &value.projection)).collect::<Vec<_>>(),
                    sources.formations,
                );
            }
            !stored.is_empty()
                && stored.iter().all(|stored| {
                    sources.call_values.iter().any(|source| {
                        source.capture == stored.capture && source.projection == stored.projection
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
        }) {
            self.completion_refusal("completion cleanup: the selected return has no matching stored occurrence of this saved access", handle);
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
            self.completion_refusal(
                "completion cleanup: no checked cleanup covers this saved access",
                handle,
            );
            self.trace
                .statement(u32::try_from(exit.statement_index).ok());
        }
        covered
    }
}

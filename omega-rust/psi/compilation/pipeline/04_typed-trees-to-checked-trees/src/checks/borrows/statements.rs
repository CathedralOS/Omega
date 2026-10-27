use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use validation::find_state_in_machine;

use crate::flow::{
    StateMutationSummaryCache, StateReceiverReadCache, call_write_accesses, statement_mutated_place,
};
use crate::labels::symbol_name;

use super::details::active_loan_detail;
use super::overlap::{
    StatedOrderingPremise, borrow_loan_compatibility, canonical_place_loan_compatibility,
};

pub(super) fn check_statement_borrows<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_state: &checked_trees::StateBorrowFact,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    state_mutation_summaries: &StateMutationSummaryCache,
    receiver_reads: &StateReceiverReadCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    let Some(state) =
        find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return;
    };
    let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
        return;
    };
    // Use the same occurrence inventory as ownership replay: an extraction
    // invalidates borrowed storage even when it is a local initializer rather
    // than an assignment. Production destinations are not source accesses.
    let move_segments = &facts.flow.ownership.segments;
    let moves = facts
        .flow
        .ownership
        .storage_transfers
        .span_or_empty(state_flow.storage_transfers);
    for statement in facts
        .flow
        .control
        .statements
        .span_or_empty(state_flow.statements)
    {
        let mut available_premises = stated_premises.to_vec();
        super::overlap::append_statement_premises(
            program,
            facts,
            state_flow,
            statement.statement_index,
            call_frames,
            &mut available_premises,
            bound_lookup,
        );
        let stated_premises = available_premises.as_slice();
        let Some(statement_node) = program
            .statement_table
            .statement_at(state, statement.statement_index)
        else {
            continue;
        };

        for (loan_handle, loan) in facts
            .borrow
            .state_loans(borrow_state)
            .filter(|(_, loan)| loan.statement_index == statement.statement_index)
        {
            let supplying_accesses = super::access_sources::resolve_all(
                program,
                facts,
                state_flow,
                statement,
                loan.formation_expression,
            );
            let retained_lineage =
                super::aliases::formation_lineage(&facts.borrow, borrow_state, loan_handle);
            let formation_constraints = if loan.established_by_call.is_valid() {
                if !facts.borrow.calls.is_valid(loan.established_by_call) {
                    diagnostics.push(Diagnostic::error(
                        "reference-slot custody names an unavailable establishing invocation",
                    ));
                    continue;
                }
                let establishing = facts.borrow.calls.get(loan.established_by_call);
                let mut actual = facts
                    .flow
                    .control
                    .calls
                    .span_or_empty(state_flow.calls)
                    .iter()
                    .filter(|call| {
                        call.statement_index == statement.statement_index
                            && call.call_ordinal == establishing.call_ordinal
                            && call.target_symbol == establishing.target_symbol
                            && call.authored_expression == establishing.authored_expression
                    });
                let Some(call) = actual.next() else {
                    // The checking query inventories syntactic alternatives;
                    // an unevaluated one cannot establish current custody.
                    continue;
                };
                if actual.next().is_some() {
                    diagnostics.push(Diagnostic::error(
                        "reference-slot custody has ambiguous normal-exit invocation evidence",
                    ));
                    continue;
                }
                if !facts
                    .flow
                    .borrow_loan_constraints(call.exit_constraints)
                    .any(|active| active == loan_handle)
                {
                    diagnostics.push(Diagnostic::error(
                        "reference-slot custody is absent from its invocation normal exit",
                    ));
                    continue;
                }
                call.exit_constraints
            } else {
                statement.entry_constraints
            };
            for active_loan_handle in facts.flow.borrow_loan_constraints(formation_constraints) {
                if super::aliases::same_descriptor(&facts.borrow, loan_handle, active_loan_handle) {
                    continue;
                }
                let active_loan = facts.borrow.loans.get(active_loan_handle);
                let compatibility = borrow_loan_compatibility(
                    program,
                    facts,
                    loan,
                    active_loan,
                    stated_premises,
                    bound_lookup,
                );
                if compatibility.non_interfering
                    || supplier_preserves_loan(
                        &facts.borrow, loan, active_loan_handle, active_loan, &compatibility,
                    )
                    // Copies retain the actual supplier's ancestors, including
                    // an exclusive parent above a shared child. No ancestry is
                    // inferred from the copied pointer's backing geometry.
                    || (active_loan.kind.is_exclusive()
                        && active_loan.kind.direct_reborrow_effect(&loan.kind).is_some()
                        && retained_lineage.as_ref().is_some_and(|lineage| {
                            lineage.iter().any(|ancestor| super::aliases::same_descriptor(
                                &facts.borrow, *ancestor, active_loan_handle,
                            ))
                        }))
                    || supplying_accesses.as_ref().is_some_and(|sources| {
                        // A source-derived selected result retains each exact
                        // input lineage. Reborrowing it cannot skip those
                        // parents merely because its captured roots are joined.
                        active_loan.kind.is_exclusive()
                            && active_loan
                                .kind
                                .direct_reborrow_effect(&loan.kind)
                                .is_some()
                            && sources.iter().any(|source| {
                                source.place == compatibility.left
                                    && source.lineage.contains(&active_loan_handle)
                            })
                    })
                {
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "statement {} creates local borrow `{}` while local borrow `{}` is still active ({})",
                    statement.statement_index,
                    symbol_name(program, loan.owner_symbol),
                    symbol_name(program, active_loan.owner_symbol),
                    active_loan_detail(
                        state_flow,
                        facts,
                        active_loan_handle,
                        statement.statement_index,
                    )
                    .unwrap_or_else(|| format!("borrowed at statement {}", active_loan.statement_index)),
                )));
            }
        }

        // Current descriptor custody follows the same selection cursor as
        // permission replay. A completed invocation replaces its exact slots;
        // sibling alternatives restart from entry and reconverge by loan union.
        let mut current_loans = facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
            .collect::<Vec<_>>();
        let mut selected = crate::checks::ownership_occurrences::SelectedEvaluation::new();
        let occurrences = crate::checks::ownership_occurrences::statement_steps(
            program,
            machine,
            state,
            statement.statement_index,
            statement_node,
            moves,
            &facts.operators,
        );
        super::staged_values::check(
            program,
            facts,
            state_flow,
            statement.statement_index,
            &occurrences,
            stated_premises,
            diagnostics,
            bound_lookup,
        );
        let final_holds = super::reads::final_holds_in_statement(
            program,
            facts,
            state_flow,
            statement,
            statement_node,
            &occurrences,
            false,
        );
        for (position, occurrence) in occurrences.iter().enumerate() {
            use crate::checks::ownership_occurrences::StatementStep;
            if let Err(error) = selected.enter(
                &occurrence.route,
                &mut current_loans,
                |_, _| Ok(()),
                |_, current, sibling| {
                    for loan in sibling {
                        if !current.contains(loan) {
                            current.push(*loan);
                        }
                    }
                    Ok(())
                },
            ) {
                diagnostics.push(error);
                break;
            }
            match &occurrence.step {
                StatementStep::Observe { place, expression } => {
                    let active = current_loans
                        .iter()
                        .copied()
                        .filter(|loan| {
                            !final_holds
                                .iter()
                                .any(|(ended, held_until)| ended == loan && *held_until < position)
                                && !super::selected_retirement::before_evaluation(
                                    program,
                                    facts,
                                    state_flow,
                                    statement.statement_index,
                                    occurrence.evaluation,
                                    *loan,
                                )
                        })
                        .collect::<Vec<_>>();
                    super::reads::check_storage_read(
                        program,
                        facts,
                        super::reads::StatementRead {
                            state: state_flow,
                            statement,
                            place,
                            expression: *expression,
                            active_loans: &active,
                        },
                        stated_premises,
                        diagnostics,
                        bound_lookup,
                    );
                }
                StatementStep::Complete(expression) => {
                    for call in facts
                        .borrow
                        .calls
                        .span_or_empty(borrow_state.calls)
                        .iter()
                        .filter(|call| {
                            call.statement_index == statement.statement_index
                                && call.authored_expression == *expression
                                && (!call.reference_slot_effects.is_empty()
                                    || !call.completed_reference_loans.is_empty())
                        })
                    {
                        current_loans.retain(|loan| {
                            !crate::borrow::calls::replaces_reference_loan(
                                program,
                                &facts.borrow,
                                call,
                                *loan,
                            )
                        });
                        for handle in call.completed_reference_loans.handles() {
                            crate::collections::push_unique(&mut current_loans, handle);
                        }
                    }
                    for (handle, _) in facts.borrow.state_loans(borrow_state).filter(|(_, loan)| {
                        loan.statement_index == statement.statement_index
                            && !loan.established_by_call.is_valid()
                            && loan.formation_expression == *expression
                    }) {
                        crate::collections::push_unique(&mut current_loans, handle);
                    }
                }
                StatementStep::Move {
                    event,
                    ordered: true,
                    ..
                } => {
                    let event = &moves[*event];
                    super::calls::check_staged_operand_move(
                        program,
                        facts,
                        state_flow,
                        borrow_state,
                        statement,
                        event,
                        &occurrences[..position],
                        &occurrence.route,
                        stated_premises,
                        diagnostics,
                        bound_lookup,
                    );
                    // Call transfers use their own checked occurrence and
                    // argument authority; production targets have no operand.
                    if !event.expression.is_valid()
                        || !matches!(
                            event.source,
                            crate::flow::FlowOwnershipEventSource::Statement { .. }
                        )
                    {
                        continue;
                    }
                    let active = current_loans.iter().copied();
                    super::moves::check_storage_move(
                        program,
                        facts,
                        state_flow,
                        statement,
                        event,
                        move_segments.span_or_empty(event.segments),
                        active,
                        stated_premises,
                        diagnostics,
                        bound_lookup,
                    );
                }
                _ => {}
            }
        }

        let Some(mutated_place) = statement_mutated_place(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            statement.statement_index,
            statement_node,
        ) else {
            continue;
        };

        let Some(assignment) = statement_node.as_assignment() else {
            continue;
        };
        let mut owned_frames = None;
        let frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
        let binding_replacement = frames.is_some_and(|frames| {
            frames.assignment_replaces_reference_binding(machine, statement_node) == Some(true)
        });
        let destinations = if binding_replacement {
            let Some(root) = mutated_place.root.as_symbol() else {
                continue;
            };
            Some(vec![super::aliases::ResolvedAlias {
                place: checked_trees::CapturedPlace {
                    incoming_reference: Default::default(),
                    root: facts::PlaceRoot::Symbol(root),
                    segments: mutated_place.segments.clone(),
                },
                lineage: Vec::new(),
            }])
        } else {
            super::access_sources::resolve_all(
                program,
                facts,
                state_flow,
                statement,
                assignment.target,
            )
        };
        let Some(destinations) = destinations else {
            diagnostics.push(Diagnostic::error(format!(
                "statement {} cannot resolve the complete borrowed destination of `{}`",
                statement.statement_index,
                mutated_place.label(program),
            )));
            continue;
        };
        // A borrowed formal denotes its ingress referent when read or lent.
        // Rebinding a mutable shared-reference formal instead writes the
        // callee's descriptor slot. Those are distinct storage occurrences;
        // the copied ingress pointer never borrowed that local slot. This
        // distinction does not cover fields, local descriptors or RHS effects.
        let ingress_slot = binding_replacement
            && mutated_place.segments.is_empty()
            && program
                .state_by_symbol(state_flow.state_symbol)
                .is_some_and(|state| {
                    mutated_place
                        .root
                        .as_symbol()
                        .and_then(|root| program.state_parameter_by_symbol(state, root))
                        .is_some_and(|parameter| {
                            parameter.is_mutable
                                && validation::reference_shell(program, parameter.type_reference)
                                    .is_some_and(|shell| {
                                        shell.access == language_semantics::ReferenceAccess::Shared
                                    })
                        })
                });
        // A write through an unselected parent still conflicts with the child.
        for loan_handle in facts
            .flow
            .borrow_loan_constraints(statement.pre_store_constraints)
        {
            if ingress_slot && facts.borrow.loans.get(loan_handle).root == mutated_place.root {
                continue;
            }
            if !super::place_access::access_interferes(
                program,
                &facts.borrow,
                &destinations,
                &checked_trees::BorrowAccessKind::Mutable,
                loan_handle,
                stated_premises,
                bound_lookup,
            ) {
                continue;
            }
            let loan = facts.borrow.loans.get(loan_handle);
            diagnostics.push(Diagnostic::error(format!(
                "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                statement.statement_index,
                mutated_place.label(program),
                symbol_name(program, loan.owner_symbol),
                active_loan_detail(state_flow, facts, loan_handle, statement.statement_index)
                    .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
            )));
        }
    }

    check_call_mutation_borrows(
        program,
        facts,
        state_flow,
        borrow_state,
        stated_premises,
        diagnostics,
        state_mutation_summaries,
        receiver_reads,
        call_frames,
        bound_lookup,
    );
}

/// Vec-views borrow rule (and, generally, owner-mutation-through-a-call vs a
/// live borrowed view).
///
/// A mutating call through an owner -- e.g. `Vec::push`/`Vec::index_mut` or any
/// explicit mutable-argument call that reallocates or writes the owner -- may
/// invalidate a borrowed slice/string view (`&[T]`/`&mut [T]`/`&string`) taken
/// from that owner. The owner-write rule already rejects this for *assignment*
/// statements; this extends the same conflict to *call* statements, whose
/// write accesses are computed by [`crate::flow::call_write_accesses`] (the receiver place
/// plus any mutable-argument places). A mutated place overlapping a loan that is
/// still live at the call point is rejected.
///
/// This reuses the existing loan-overlap engine, so it inherits the disjoint
/// subslice/element precision and stays conservative when a window is unknown.
/// A named same-machine jump is different from a returning call: its successor
/// writes happen after source-state scope exit. Replayed local-loan closure can
/// discharge that conflict when no argument carries a loan into the successor.
/// With authenticated source-binding closure, a forwarded child's held summary
/// instead follows its remaining successor lifetime. Guard/argument calls and
/// unclosed or stored carriers retain the full run; a syntactic jump alone is
/// not release evidence.
fn check_call_mutation_borrows<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_state: &checked_trees::StateBorrowFact,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    summary_cache: &StateMutationSummaryCache,
    receiver_reads: &StateReceiverReadCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
        let mutated_places = call_write_accesses(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &facts.borrow,
            borrow_call,
            summary_cache,
            None,
        );
        // A call that writes nothing can still read, through its receiver,
        // storage one of its exclusive arguments suspends.
        let retains_exclusive_argument = facts
            .borrow
            .argument_accesses
            .span_or_empty(borrow_call.accesses)
            .iter()
            .any(|argument| argument.kind.is_exclusive() && argument.end_before_access.is_none());
        let live_loans = if mutated_places.is_empty() {
            Vec::new()
        } else {
            loans_live_at_call(program, facts, state_flow, borrow_call, call_frames)
        };
        if mutated_places.is_empty() && !retains_exclusive_argument {
            continue;
        }
        let mut available_premises = stated_premises.to_vec();
        super::overlap::append_statement_premises(
            program,
            facts,
            state_flow,
            borrow_call.statement_index,
            call_frames,
            &mut available_premises,
            bound_lookup,
        );
        let stated_premises = available_premises.as_slice();

        // The call's own retained argument loans are not live at its entry,
        // yet the callee writes and reads through a whole receiver while
        // holding them.
        super::calls::check_receiver_footprint_against_argument_loans(
            program,
            facts,
            state_flow,
            borrow_call,
            stated_premises,
            super::calls::ReceiverSummaries {
                writes: summary_cache,
                reads: receiver_reads,
            },
            diagnostics,
            bound_lookup,
        );

        // Report at most one mutation conflict per call and live loan.
        let exiting_source = local_jump(program, state_flow, borrow_call).and_then(|_| {
            find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
        });
        for &loan_handle in &live_loans {
            let loan = facts.borrow.loans.get(loan_handle);
            // A local jump may carry this child beyond the source binding's
            // closure. First authenticate that exact closure, then use the
            // held footprint to follow the child's successor lifetime. Missing
            // closure and stored carrier leaves retain the complete write run;
            // spatial forwarding alone cannot prove their authority ended.
            let held_writes = exiting_source
                .filter(|source| {
                    local_loan_ends_before_successor(
                        program,
                        facts,
                        state_flow,
                        source,
                        loan_handle,
                        borrow_call,
                        call_frames,
                    )
                })
                .map(|_| {
                    let captured = validation::CanonicalPlace {
                        root: loan.root,
                        segments: facts.borrow.loan_segments(loan).to_vec(),
                    };

                    call_write_accesses(
                        program,
                        state_flow.machine_symbol,
                        state_flow.state_symbol,
                        &facts.borrow,
                        borrow_call,
                        summary_cache,
                        Some(&captured),
                    )
                });
            for mutated_place in held_writes.as_ref().unwrap_or(&mutated_places) {
                if canonical_place_loan_compatibility(
                    program,
                    mutated_place,
                    loan,
                    &facts.borrow,
                    stated_premises,
                    bound_lookup,
                )
                .non_interfering
                {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                    borrow_call.statement_index,
                    mutated_place.label(program),
                    symbol_name(program, loan.owner_symbol),
                    active_loan_detail(
                        state_flow,
                        facts,
                        loan_handle,
                        borrow_call.statement_index,
                    )
                    .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
                )));
                break;
            }
        }
    }
}

/// The loans live *at* a call: that invocation's actual entry constraints,
/// less the local observations a named jump's source-state
/// exit closes before its successor runs.
fn loans_live_at_call(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &checked_trees::BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<arena::Handle<checked_trees::BorrowLoanFact>> {
    let mut loans = facts
        .flow
        .borrow_loan_constraints(facts.flow.state_call_entry_constraints(
            state_flow,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
            borrow_call.target_symbol,
            borrow_call.receiver_symbol,
        ))
        .peekable();
    if loans.peek().is_none() {
        return Vec::new();
    }
    let exiting_source =
        source_exiting_without_local_loan_carriers(program, state_flow, borrow_call, call_frames);
    let boundary = super::calls::InvocationBoundary::at(program, facts, state_flow, borrow_call);
    let ended_before_call = super::reads::loans_ended_before_call(
        program,
        facts,
        state_flow,
        borrow_call,
        boundary.as_ref(),
    );
    loans
        .filter(|loan_handle| {
            !super::selected_retirement::before_call(
                program,
                facts,
                state_flow,
                borrow_call,
                *loan_handle,
            ) && !ended_before_call.contains(loan_handle)
                && !exiting_source.is_some_and(|state| {
                    local_loan_ends_before_successor(
                        program,
                        facts,
                        state_flow,
                        state,
                        *loan_handle,
                        borrow_call,
                        call_frames,
                    )
                })
        })
        .collect()
}

/// A named jump to a sibling state: the transition target names a state of the
/// caller's own machine with no receiver operand. Calls in the transition's
/// guard or arguments are ordinary calls, not the jump.
pub(super) struct LocalJump<'program> {
    pub(super) target: &'program typed_trees::state::State,
    pub(super) site: validation::CallSite<'program>,
}

pub(super) fn local_jump<'program>(
    program: &'program dyn ::validation::CheckingInput,
    flow: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
) -> Option<LocalJump<'program>> {
    let target = find_state_in_machine(program, flow.machine_symbol, call.target_symbol)?;
    let site = validation::find_call_site(
        program,
        flow.machine_symbol,
        flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let validation::CallSite::TransitionNamed { path, .. } = &site else {
        return None;
    };
    // A local state name has no explicit receiver; its head is the target
    // symbol, not a borrowed receiver alias.
    (path.members.count() == 1 && path.head_symbol == target.symbol)
        .then_some(LocalJump { target, site })
}

/// A named jump enters another state only after the caller's local scope ends.
/// Calls in its guard or arguments are ordinary pre-exit calls. Keep carried
/// loans out of this judgment: their successor use needs its own correspondence.
pub(super) fn source_exiting_without_local_loan_carriers<'program>(
    program: &'program dyn ::validation::CheckingInput,
    flow: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<&'program typed_trees::state::State> {
    let source = find_state_in_machine(program, flow.machine_symbol, flow.state_symbol)?;
    let LocalJump { target, site } = local_jump(program, flow, call)?;
    let machine = program.machine_by_symbol(flow.machine_symbol)?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    let parameters = program.state_parameters(target);
    let mut positional = parameters.iter();
    for argument in arguments {
        let parameter = positional.next()?;
        let actual =
            validation::expression_result_type_reference(program, machine, source, *argument)?;
        if [parameter.type_reference, actual]
            .into_iter()
            .all(|reference| {
                crate::borrow::view_link::substituted_result_is_view_free(program, reference, &[])
            })
        {
            continue;
        }
        // Forwarding backing storage does not carry a local observation of it.
        // Incoming references already outlive the source scope. A local owned
        // home instead needs the complete successor retention relation, including
        // escape and repeated-establishment checks. Reference aliases and homes
        // containing reference leaves retain their separate loan obligations.
        for reference in [parameter.type_reference, actual] {
            let typed_trees::types::TypeReferenceNode::Reference { .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
        }
        let place = crate::borrow::argument_borrow_loan_place(
            program,
            source.symbol,
            call.statement_index,
            machine.symbol,
            *argument,
        )?;
        let root = place.root.as_symbol()?;
        if let Some(incoming) = program
            .state_parameters(source)
            .iter()
            .find(|candidate| candidate.symbol == root)
        {
            let typed_trees::types::TypeReferenceNode::Reference { .. } = program
                .type_reference_table
                .type_reference(incoming.type_reference)
            else {
                return None;
            };
        } else {
            let (_, local) = program.state_local_declaration(source, root)?;
            if !crate::borrow::view_link::substituted_result_is_view_free(
                program,
                local.type_reference,
                &[],
            ) || !call_frames?.local_backing_survives_state_jump(
                machine,
                source,
                call.statement_index,
                target,
                arguments,
                root,
            ) {
                return None;
            }
        }
    }
    positional.next().is_none().then_some(source)
}

/// Whether `forming_loan` is recorded as deriving its authority through this
/// exact active loan occurrence.
///
/// A retained reborrow names its unique parent loan handle; the edge is
/// replayed independently from the typed statement by lineage replay. Its
/// inherited parent path retains the parent's captured selectors, not their
/// current mutable storage values. An exact inherited prefix therefore proves
/// containment without equating independently evaluated selectors. An
/// unretained transfer (aggregate-carried or helper-derived) keeps only its
/// rebasing source owner local, so it may rejoin an active loan of that owner
/// only when the replayed containment verdict proves the forming place still
/// sits inside the active loan's captured place. Either way the active loan
/// must be exclusive and permit the child's access: sharing a read parent does
/// not suspend anything, and a write-only parent cannot grant observation.
///
/// This judgment only reads already-recorded loan rows and the independently
/// replayed captured places. It creates no authority: the forming loan
/// was itself produced by rebasing through the named source, and the pair's
/// compatibility certificate still retains the honest spatial conclusion.
pub(super) fn supplier_preserves_loan(
    borrow: &checked_trees::BorrowFacts,
    forming_loan: &checked_trees::BorrowLoanFact,
    active_loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
    active_loan: &checked_trees::BorrowLoanFact,
    compatibility: &checked_trees::CapturedPlaceCompatibility,
) -> bool {
    // Moving a lifetime-only carrier transports the same exclusive debt; it
    // does not form another reference or lend spatial access. Only its exact
    // recorded supplier can be exempt, with unchanged backing and access.
    if forming_loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
        return active_loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
            && forming_loan.kind == active_loan.kind
            && forming_loan.source_owner_symbol == active_loan.owner_symbol
            && matches!(
                compatibility.containment,
                checked_trees::CapturedPlaceContainment::Same
            )
            && borrow
                .loan_suppliers
                .span(forming_loan.supplying_loans)
                .is_some_and(|suppliers| suppliers.contains(&active_loan_handle));
    }
    if !active_loan.kind.is_exclusive()
        || active_loan
            .kind
            .direct_reborrow_effect(&forming_loan.kind)
            .is_none()
    {
        return false;
    }
    let proven_containment = matches!(
        compatibility.containment,
        checked_trees::CapturedPlaceContainment::Same
            | checked_trees::CapturedPlaceContainment::RightContainsLeft
    );
    match forming_loan.lineage {
        checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => {
            parent_loan == active_loan_handle
                && forming_loan.source_owner_symbol == active_loan.owner_symbol
                && (proven_containment
                    || (compatibility.right.root.is_valid()
                        && compatibility.left.root == compatibility.right.root
                        && compatibility
                            .left
                            .segments
                            .starts_with(&compatibility.right.segments)))
        }
        checked_trees::BorrowLoanLineage::UnretainedDerived => {
            proven_containment && forming_loan.source_owner_symbol == active_loan.owner_symbol
        }
        checked_trees::BorrowLoanLineage::DirectRoot
        | checked_trees::BorrowLoanLineage::SuppliedOutput
        | checked_trees::BorrowLoanLineage::LifetimeDependency => false,
    }
}

pub(super) fn local_loan_ends_before_successor(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    source: &typed_trees::state::State,
    loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
    call: &checked_trees::BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let loan = facts.borrow.loans.get(loan_handle);
    let Some((_, local)) = program.state_local_declaration(source, loan.owner_symbol) else {
        return false;
    };
    if !loan.owner_path.is_empty()
        || !matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        )
    {
        // A record's carried reference survives lexical last use: its nominal
        // hook still receives the intact record. Only that leaf's selected
        // structural disposition closes it. No carried child can be restored
        // by this rule; each surviving loan remains in the conflict frontier.
        return source_exiting_without_local_loan_carriers(program, flow, call, call_frames)
            .is_some()
            && super::successor_cleanup::disposes_loan(program, facts, flow, call, loan);
    }
    // Source correspondence authenticates every loan's formation and last use,
    // including derived views. Scope closure ends a non-carried local binding;
    // it does not promote the view's lineage or restore parent authority inside
    // the source state. Reassignment remains a separate evaluation boundary.
    let execution_order = program
        .statement_table
        .admitted_execution_order(source.statement_nodes);
    facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .any(|weakening| {
            weakening.loan == loan_handle
                && match (weakening.reason, weakening.source) {
                    (
                        checked_trees::FlowBorrowWeakeningReason::StateExit,
                        checked_trees::FlowInvalidationSource::Statement { statement_index },
                    ) => statement_index == source.statement_nodes.count() as usize,
                    (
                        checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                        checked_trees::FlowInvalidationSource::Statement { statement_index },
                    ) => {
                        Some(statement_index)
                            == execution_order.successor(loan.last_use_statement_index)
                            && statement_index <= source.statement_nodes.count() as usize
                    }
                    _ => false,
                }
        })
}

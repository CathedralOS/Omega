//! Statement facts for one state. Per statement, `append_state_statement_flow_facts`
//! records entry contexts, runs a transition's arms (`exits`) or its calls
//! (`expression`), retires and activates borrow loans, drops facts over written
//! places, adds operator `ensures`, and propagates the written value's facts.
use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::append_flow_contexts_for_points;
use crate::flow::append_operator_statement_ensures;
use crate::flow::filter_contexts_after_place_mutations;
use crate::flow::filter_expired_borrow_loans;
use crate::flow::filter_reassigned_borrow_loans;
use crate::flow::operator_statement_call_mutated_places;
use crate::flow::project_constraint_refs_to_active_contexts;
use crate::flow::propagate_statement_transfers;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::{Handle, HandleSpan};
use checked_trees::statement::StatementNode;
use checked_trees::{
    BorrowFacts, DomainFacts, FlowBorrowActivationFact, FlowBorrowWeakeningReason, FlowCallFact,
    FlowConstraintKind, FlowConstraintRef, FlowInvalidationSource, FlowSemanticContextRef,
    FlowStatementFact, ProofFacts, StateBorrowFact,
};
use facts::{FactPlan, ProgramPoint};

#[allow(clippy::too_many_arguments)]
pub(super) fn append_state_statement_flow_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    active_contexts: &mut arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut arena::HandleSpan<FlowConstraintRef>,
    borrow_state: &StateBorrowFact,
    execution_order: &typed_trees::statement::StatementExecutionOrder,
) -> arena::HandleSpan<FlowCallFact> {
    let mut state_calls = arena::HandleSpan::empty();
    build.state_statements_start = build.control.statements.frontier();
    let borrow_calls = borrow.calls.span_or_empty(borrow_state.calls);
    let borrow_loans = borrow.loans.span_or_empty(borrow_state.loans);
    let source_count = state.statement_nodes.len();
    let call_ranges = statement_record_ranges(
        source_count,
        borrow_calls.iter().map(|call| call.statement_index),
    );
    let loan_ranges = statement_record_ranges(
        source_count,
        borrow_loans.iter().map(|loan| loan.statement_index),
    );

    for statement_index in execution_order.indices().iter().copied() {
        let statement = &program.statement_table.statements(state.statement_nodes)[statement_index];
        let statement_calls = &borrow_calls[call_ranges[statement_index].clone()];
        let operand_capture_start = build.control.operands.frontier();
        append_proof_output_ensures(
            proof,
            semantic,
            build,
            machine.symbol,
            state.symbol,
            statement_index,
            active_contexts,
            active_constraints,
        );
        *active_constraints = filter_expired_borrow_loans(
            &mut build.borrow_lifetimes.weakenings,
            &mut build.contexts.constraint_refs,
            *active_constraints,
            borrow,
            statement_index,
            execution_order,
            FlowBorrowWeakeningReason::LastUseExpired,
        );
        let previous_statements = super::reference_spans::appended_span_since(
            &build.control.statements,
            build.state_statements_start,
        );
        let storage_snapshot = build
            .control
            .statements
            .span_or_empty(previous_statements)
            .last()
            .map_or_else(Handle::invalid, |statement| statement.storage_snapshot);
        let statement_fact = build.control.statements.append(FlowStatementFact {
            statement_index,
            storage_snapshot,
            value_observations: HandleSpan::empty(),
            stored_value: Handle::invalid(),
            store_target: Handle::invalid(),
            stored_destination: Handle::invalid(),
            completed_writes: HandleSpan::empty(),
            completed_unknown_write: false,
            completed_content_writes: HandleSpan::empty(),
            completed_content_unknown_write: false,
            entry_semantic_contexts: *active_contexts,
            exit_semantic_contexts: HandleSpan::empty(),
            entry_constraints: *active_constraints,
            pre_store_constraints: HandleSpan::empty(),
        });
        if matches!(statement, StatementNode::Transition(_))
            && proof.has_outcome_specific_arm(machine.symbol, state.symbol, statement_index)
        {
            let point = ProgramPoint::Statement {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
            };
            *active_contexts =
                retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
            *active_constraints =
                retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
            append_flow_contexts_for_points(
                semantic,
                &mut build.contexts.semantic_context_refs,
                active_contexts,
                &mut build.contexts.constraint_refs,
                active_constraints,
                &[point],
            );
        }
        // A multi-arm transition desugars to a SEQUENCE of guarded transition
        // statements (an if/elseif chain): statement N is taken when its guard
        // holds (and the state exits), else control falls through to statement
        // N+1. A guarded transition's TARGET call exits the state, so threading
        // its (empty) exit context to the next statement is wrong -- the next
        // statement is the FALLTHROUGH (this guard was false), which never ran the
        // branch, so it must keep the PRE-transition context. Without this, an
        // else-arm call (`false -> consume(text)`) saw 0 entry contexts and a
        // forwarded domain fact never reached it. Snapshot the entry context so we
        // can restore it for the fallthrough after this statement is flowed.
        let mut fallthrough_contexts = *active_contexts;
        let mut fallthrough_constraints = *active_constraints;
        if let Some(transition) = statement.as_transition() {
            let writes = super::exits::append_transition_flow_facts(
                program,
                borrow,
                proof,
                semantic,
                domains,
                build,
                machine,
                state,
                statement_index,
                statement_fact,
                transition,
                statement_calls,
                &mut state_calls,
                active_contexts,
                active_constraints,
            );
            let retained = build.control.statements.get_mut(statement_fact);
            retained.storage_snapshot = writes.storage_snapshot;
            retained.completed_writes = writes.writes;
            retained.completed_unknown_write = writes.unknown;
            fallthrough_contexts = *active_contexts;
            fallthrough_constraints = *active_constraints;
        }
        if !matches!(statement, StatementNode::Transition(_)) {
            let evaluated = super::expression::append_statement_calls(
                program,
                borrow,
                proof,
                semantic,
                domains,
                build,
                machine,
                state,
                statement_index,
                statement_fact,
                statement,
                statement_calls,
                &mut state_calls,
                active_contexts,
                active_constraints,
            );
            let retained = build.control.statements.get_mut(statement_fact);
            retained.storage_snapshot = evaluated.writes.storage_snapshot;
            retained.stored_value = evaluated.stored_value;
            retained.store_target = evaluated.store_target;
            retained.completed_writes = evaluated.writes.writes;
            retained.completed_unknown_write = evaluated.writes.unknown;
        }

        // Assignment evaluates its RHS under the entry loans above, then
        // overwrites the target. Retire loans carried by the old value before
        // activating loans carried by the replacement below; otherwise the
        // replacement spuriously conflicts with the value it is replacing.
        let has_active_borrow_loans = build
            .contexts
            .constraint_refs
            .span_or_empty(*active_constraints)
            .iter()
            .any(|constraint| matches!(constraint.kind, FlowConstraintKind::BorrowLoan { .. }));
        if has_active_borrow_loans {
            let reassigned_place = if let Some(assignment) = statement.as_assignment() {
                build.canonical_place_at(program, state.symbol, statement_index, assignment.target)
            } else {
                None
            };
            *active_constraints = filter_reassigned_borrow_loans(
                &mut build.borrow_lifetimes.weakenings,
                &mut build.contexts.constraint_refs,
                *active_constraints,
                borrow,
                program,
                machine,
                reassigned_place,
                statement_index,
                statement,
                build.call_frames,
            );
        }

        for loan_index in loan_ranges[statement_index].clone() {
            let loan = &borrow_loans[loan_index];
            let loan_handle = borrow_state.loans.handle_at(loan_index as u32);

            let formation_value = {
                let captures_span = super::reference_spans::appended_span_since(
                    &build.control.operands,
                    operand_capture_start,
                );
                let mut captures = build
                    .control
                    .operands
                    .span_or_empty(captures_span)
                    .iter()
                    .enumerate()
                    .filter(|(_, operand)| {
                        operand.expression == loan.formation_expression
                            && operand.state_symbol == state.symbol
                            && operand.statement_index == statement_index
                    });
                let first = captures
                    .next()
                    .map(|(offset, _)| captures_span.handle_at(offset as u32))
                    .unwrap_or_default();
                if captures.next().is_none() {
                    first
                } else {
                    Handle::invalid()
                }
            };
            build
                .borrow_lifetimes
                .activations
                .append(FlowBorrowActivationFact {
                    source: FlowInvalidationSource::Statement { statement_index },
                    loan: loan_handle,
                    formation_value,
                });

            append_constraint_ref(
                &mut build.contexts.constraint_refs,
                active_constraints,
                FlowConstraintKind::BorrowLoan { loan: loan_handle },
            );
        }

        // Prepare the completed store once, before retiring its destination.
        // Invalidation and establishment must consume the same captured
        // address, including a view's base and a pinned selector. A coarse
        // frame path remains a may-write set, never an exact value origin.
        let mut write = super::transfers::statement_write(
            program,
            semantic,
            build,
            machine.symbol,
            state.symbol,
            statement_index,
            statement,
            build.control.statements.get(statement_fact).store_target,
        );
        if let Some(write) = &mut write {
            let target_segments = semantic
                .place_segments
                .span_or_empty(semantic.places.get(write.target_place).segments);
            let (target, stable) = if target_segments.is_empty() {
                (write.target_place, true)
            } else {
                let captured_destination =
                    build.control.statements.get(statement_fact).store_target;
                let contexts = if target_segments
                    .iter()
                    .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
                {
                    if build.control.operands.is_valid(captured_destination) {
                        build
                            .contexts
                            .constraint_refs
                            .span_or_empty(
                                build.control.operands.get(captured_destination).constraints,
                            )
                            .iter()
                            .filter_map(|constraint| match constraint.kind {
                                FlowConstraintKind::SemanticContext { context } => Some(context),
                                _ => None,
                            })
                            .collect()
                    } else {
                        crate::flow::active_context_handles(build, *active_contexts)
                    }
                } else {
                    Vec::new()
                };
                super::transfers::narrow_pinned_selectors(
                    program,
                    semantic,
                    state.symbol,
                    statement_index,
                    &contexts,
                    write.target_place,
                )
            };
            write.target_place = target;
            write.stable_value_target = stable && write.target_is_exact;
            if statement.as_assignment().is_some() {
                let mut owned_frames = None;
                let frames =
                    super::shared_call_frames_or(build.call_frames, program, &mut owned_frames);
                write.destination_is_current = frames.is_some_and(|frames| {
                    if write.target_is_exact {
                        super::canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(target),
                        )
                        .is_some_and(|destination| {
                            frames.captured_assignment_destination_remains_current(
                                machine,
                                statement,
                                &destination,
                            )
                        })
                    } else {
                        write.candidate_targets.iter().all(|destination| {
                            frames.captured_assignment_destination_remains_current(
                                machine,
                                statement,
                                destination,
                            )
                        })
                    }
                });
            }
        }
        let storage_writes = if let Some(write) = write.as_ref()
            && statement.as_assignment().is_some()
        {
            let target = super::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(write.target_place),
            );
            target.map(|target| {
                if write.target_is_exact {
                    vec![target]
                } else {
                    write.candidate_targets.clone()
                }
            })
        } else if statement.as_assignment().is_some() {
            // A failed address capture supplies no exact destination and no
            // preservation theorem. Clear live observations conservatively.
            None
        } else {
            build
                .statement_storage_writes_at(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    statement,
                )
                .map(|places| (*places).clone())
        };
        // Scalar observations require an exact logical coordinate. Binding
        // invalidation only needs a complete physical may-write enclosure;
        // losing the former must not withdraw every disjoint descriptor.
        let final_storage_writes = if statement.as_assignment().is_some() {
            super::storage_footprints::assignment(
                program,
                machine,
                statement,
                &build.control,
                build.control.statements.get(statement_fact).store_target,
                build.call_frames,
            )
        } else {
            storage_writes.clone()
        };
        let operand_writes = build
            .control
            .statements
            .get(statement_fact)
            .completed_writes;
        let operand_unknown_write = build
            .control
            .statements
            .get(statement_fact)
            .completed_unknown_write;
        if let Some(writes) = &storage_writes {
            let mut completed = build
                .control
                .statements
                .get(statement_fact)
                .completed_writes;
            for place in writes.iter() {
                let segments = build
                    .control
                    .operand_referent_segments
                    .insert_many(place.segments.iter().copied());
                build.control.operand_origin_writes.append_to_span(
                    &mut completed,
                    checked_trees::FlowOperandReferent {
                        root: place.root,
                        segments,
                    },
                );
            }
            build
                .control
                .statements
                .get_mut(statement_fact)
                .completed_writes = completed;
        } else {
            build
                .control
                .statements
                .get_mut(statement_fact)
                .completed_unknown_write = true;
        }
        // Keep the captured store footprint distinct from its content aliases.
        // Historical predicates need the expanded names too, but recording an
        // alias as a physical store would falsely replace its descriptor and
        // lose the captured address on the next write through that view.
        let content_writes = if statement.as_assignment().is_some() {
            storage_writes.as_ref().map(|places| {
                build
                    .call_frames
                    .and_then(|frames| {
                        frames.local_write_origins_before_store(
                            machine,
                            statement,
                            |symbol, segments| {
                                let Some(mut alias) = super::canonical_place_from_symbol(symbol)
                                else {
                                    return false;
                                };
                                alias.segments.extend_from_slice(segments);
                                super::operand_contexts::binding_origin_is_stable(
                                    program,
                                    &build.control,
                                    operand_writes,
                                    operand_unknown_write,
                                    &alias,
                                )
                            },
                        )
                    })
                    .and_then(|origins| {
                        super::mutation::close_storage_places_over_origins(
                            program,
                            state,
                            statement_index,
                            places.clone(),
                            &origins,
                        )
                    })
                    // Without an origin inventory every earlier local that can
                    // hold a reference is an unknown alias. Retire each one's
                    // whole footprint, as an unknown-source origin is retired,
                    // and keep facts held under every other name.
                    .unwrap_or_else(|| {
                        unknown_alias_footprints(
                            program,
                            build.call_frames,
                            state,
                            statement_index,
                            places,
                        )
                    })
            })
        } else {
            storage_writes.clone()
        };
        let mut completed_contents = build
            .control
            .statements
            .get(statement_fact)
            .completed_writes;
        if let Some(places) = &content_writes {
            for place in places.iter().filter(|place| {
                storage_writes
                    .as_ref()
                    .is_none_or(|physical| !physical.contains(place))
            }) {
                let segments = build
                    .control
                    .operand_referent_segments
                    .insert_many(place.segments.iter().copied());
                build.control.operand_origin_writes.append_to_span(
                    &mut completed_contents,
                    checked_trees::FlowOperandReferent {
                        root: place.root,
                        segments,
                    },
                );
            }
        }
        let completed = build.control.statements.get_mut(statement_fact);
        completed.completed_content_writes = completed_contents;
        completed.completed_content_unknown_write =
            completed.completed_unknown_write || content_writes.is_none();
        let final_content_writes = content_writes.clone();
        let storage_writes = content_writes;
        if storage_writes.is_none() {
            *active_contexts = HandleSpan::empty();
            *active_constraints = project_constraint_refs_to_active_contexts(
                &mut build.contexts.constraint_refs,
                *active_constraints,
                *active_contexts,
                &build.contexts.semantic_context_refs,
            );
        }
        // RHS calls have already contributed their effects. Preserve only
        // their surviving facts while the assignment still reads its old
        // source value, before invalidating the overwritten destination.
        let assignment_source_contexts = *active_contexts;
        build
            .control
            .statements
            .get_mut(statement_fact)
            .pre_store_constraints = *active_constraints;
        let mut mutated_places = storage_writes.unwrap_or_default();
        // A runtime `Index` selector whose scalar value is pinned in the live
        // contexts names exactly one element. Narrow it to that `FixedIndex`
        // before overlap matching so the write retires only the element it
        // actually reaches — the same narrowing the write-target transfer
        // applies when it records the established fact.
        if statement.as_assignment().is_none()
            && mutated_places.iter().any(|place| {
                place
                    .segments
                    .iter()
                    .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
            })
        {
            let context_handles: Vec<facts::FactContextHandle> = build
                .contexts
                .semantic_context_refs
                .span_or_empty(assignment_source_contexts)
                .iter()
                .map(|context_ref| context_ref.context)
                .collect();
            for place in &mut mutated_places {
                crate::flow::narrow_pinned_index_segments(
                    program,
                    semantic,
                    state.symbol,
                    statement_index,
                    &context_handles,
                    &mut place.segments,
                );
            }
        }
        if let Some(call) = statement.as_call() {
            mutated_places.extend(operator_statement_call_mutated_places(
                program,
                build,
                machine.symbol,
                state.symbol,
                statement_index,
                call,
            ));
        }
        if !mutated_places.is_empty() {
            super::entry_origins::extend_invocation_observations(
                program,
                build,
                machine,
                state,
                statement_index,
                &mut mutated_places,
            );
            *active_contexts = filter_contexts_after_place_mutations(
                program,
                semantic,
                domains,
                &mut build.contexts.semantic_context_refs,
                &mut build.invalidations.segments,
                &mut build.invalidations.events,
                *active_contexts,
                &mutated_places,
                FlowInvalidationSource::Statement { statement_index },
            );
            *active_constraints = project_constraint_refs_to_active_contexts(
                &mut build.contexts.constraint_refs,
                *active_constraints,
                *active_contexts,
                &build.contexts.semantic_context_refs,
            );
        }

        if let Some(call) = statement.as_call() {
            append_operator_statement_ensures(
                program,
                semantic,
                build,
                machine.symbol,
                state.symbol,
                statement_index,
                call,
                active_contexts,
                active_constraints,
            );
        }
        propagate_statement_transfers(
            program,
            borrow,
            semantic,
            build,
            machine.symbol,
            state.symbol,
            statement_index,
            statement,
            statement_fact,
            write,
            assignment_source_contexts,
            active_contexts,
            active_constraints,
        );
        // Calls have already published their ordered writes and replacements.
        // Only the enclosing statement's own store follows RHS evaluation.
        let final_write = match statement {
            StatementNode::Assignment(_) | StatementNode::Assembly(_) => Some((
                final_storage_writes.as_deref(),
                final_content_writes.as_deref(),
            )),
            StatementNode::LocalData(local) => {
                let destination = validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(local.symbol),
                    segments: Vec::new(),
                };
                let snapshot = build
                    .control
                    .statements
                    .get(statement_fact)
                    .storage_snapshot;
                let snapshot = super::stored_values::append_place_writes(
                    &mut build.control,
                    snapshot,
                    Some(std::slice::from_ref(&destination)),
                    Some(std::slice::from_ref(&destination)),
                );
                build
                    .control
                    .statements
                    .get_mut(statement_fact)
                    .storage_snapshot = snapshot;
                None
            }
            _ => None,
        };
        if let Some((bindings, contents)) = final_write {
            let snapshot = build
                .control
                .statements
                .get(statement_fact)
                .storage_snapshot;
            let snapshot = super::stored_values::append_place_writes(
                &mut build.control,
                snapshot,
                bindings,
                contents,
            );
            build
                .control
                .statements
                .get_mut(statement_fact)
                .storage_snapshot = snapshot;
        }
        let completed = build.control.statements.get(statement_fact);
        if semantic.places.is_valid(completed.stored_destination)
            && build.control.operands.is_valid(completed.stored_value)
            && let Some(destination) = super::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(completed.stored_destination),
            )
        {
            let snapshot = completed.storage_snapshot;
            let value = completed.stored_value;
            let snapshot = super::stored_values::append_store(
                &mut build.control,
                snapshot,
                &destination,
                value,
                &[],
            );
            build
                .control
                .statements
                .get_mut(statement_fact)
                .storage_snapshot = snapshot;
        }
        super::atomic_effects::append(
            program,
            semantic,
            build,
            machine,
            state,
            statement_index,
            statement,
            active_contexts,
            active_constraints,
        );
        build
            .control
            .statements
            .get_mut(statement_fact)
            .exit_semantic_contexts = *active_contexts;

        // Restore the pre-transition context for the fallthrough path (see the
        // snapshot above). A guarded transition either takes its branch and exits
        // -- so its exit context belongs to the target state's flow, not to the
        // sibling fallthrough -- or its guard is false and control continues to the
        // next statement with the context unchanged by the (untaken) branch.
        if matches!(
            statement,
            typed_trees::statement::StatementNode::Transition(_)
        ) {
            *active_contexts = fallthrough_contexts;
            *active_constraints = fallthrough_constraints;
        }
    }
    append_proof_output_ensures(
        proof,
        semantic,
        build,
        machine.symbol,
        state.symbol,
        program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        active_contexts,
        active_constraints,
    );

    state_calls
}

#[allow(clippy::too_many_arguments)]
fn append_proof_output_ensures(
    proof: &ProofFacts,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    active_contexts: &mut arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut arena::HandleSpan<FlowConstraintRef>,
) {
    let has_proof_only_binding =
        proof.has_proof_only_output_call(machine_symbol, state_symbol, statement_index);
    if !has_proof_only_binding {
        return;
    }
    *active_contexts =
        retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
    *active_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
    let point = ProgramPoint::CallEnsures {
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal: 0,
    };
    append_flow_contexts_for_points(
        semantic,
        &mut build.contexts.semantic_context_refs,
        active_contexts,
        &mut build.contexts.constraint_refs,
        active_constraints,
        &[point],
    );
}

/// The physical writes plus every local declared before `statement_index`
/// that may hold a reference. Missing origin evidence does not make an owned,
/// reference-free value an alias: use the frame owner's complete structural
/// classification, not a primitive-versus-aggregate approximation. Opaque,
/// recursive and unresolved types remain conservative. This excludes aliases;
/// it grants no domain facts or exemption from the physical write footprint.
fn unknown_alias_footprints(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    state: &typed_trees::state::State,
    statement_index: usize,
    places: &[validation::CanonicalPlace],
) -> Vec<validation::CanonicalPlace> {
    let mut footprints = places.to_vec();
    let mut owned_frames = None;
    let frames = super::shared_call_frames_or(call_frames, program, &mut owned_frames);
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if frames.is_some_and(|frames| frames.value_is_reference_free(local.type_reference)) {
            continue;
        }
        if let Some(alias) = super::canonical_place_from_symbol(local.symbol)
            && !footprints.contains(&alias)
        {
            footprints.push(alias);
        }
    }
    footprints
}

/// Records stay in their producer's execution order. Their stable source
/// coordinates select contiguous per-statement cohorts without sorting or
/// changing the arena identities retained by loans and calls.
fn statement_record_ranges(
    source_count: usize,
    statements: impl Iterator<Item = usize>,
) -> Vec<std::ops::Range<usize>> {
    let mut ranges = vec![0..0; source_count + 1];
    for (ordinal, statement) in statements.enumerate() {
        let range = ranges
            .get_mut(statement)
            .expect("borrow record names its source frame or exit");
        if range.is_empty() {
            *range = ordinal..ordinal + 1;
        } else {
            assert_eq!(
                range.end, ordinal,
                "one statement owns one contiguous borrow cohort"
            );
            range.end += 1;
        }
    }
    ranges
}

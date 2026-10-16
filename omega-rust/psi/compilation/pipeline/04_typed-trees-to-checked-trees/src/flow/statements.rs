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
) -> arena::HandleSpan<FlowCallFact> {
    let mut state_calls = arena::HandleSpan::empty();
    build.state_statements_start = build.control.statements.len();
    let borrow_calls = borrow.calls.span_or_empty(borrow_state.calls);
    let borrow_loans = borrow.loans.span_or_empty(borrow_state.loans);
    let mut call_index = 0usize;
    let mut loan_index = 0usize;

    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        let operand_capture_start = build.control.operands.len();
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
            FlowBorrowWeakeningReason::LastUseExpired,
        );
        let statement_fact = build.control.statements.append(FlowStatementFact {
            statement_index,
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
            && proof.outcome_specific_arms.iter().any(|(_, arm)| {
                arm.caller_machine_symbol == machine.symbol
                    && arm.caller_state_symbol == state.symbol
                    && arm.statement_index == statement_index
            })
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
            let start = call_index;
            while borrow_calls
                .get(call_index)
                .is_some_and(|call| call.statement_index == statement_index)
            {
                call_index += 1;
            }
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
                transition,
                &borrow_calls[start..call_index],
                &mut state_calls,
                active_contexts,
                active_constraints,
            );
            let retained = build.control.statements.get_mut(statement_fact);
            retained.completed_writes = writes.writes;
            retained.completed_unknown_write = writes.unknown;
            fallthrough_contexts = *active_contexts;
            fallthrough_constraints = *active_constraints;
        }
        if !matches!(statement, StatementNode::Transition(_)) {
            let start = call_index;
            while borrow_calls
                .get(call_index)
                .is_some_and(|call| call.statement_index == statement_index)
            {
                call_index += 1;
            }
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
                statement,
                &borrow_calls[start..call_index],
                &mut state_calls,
                active_contexts,
                active_constraints,
            );
            let retained = build.control.statements.get_mut(statement_fact);
            retained.stored_value = evaluated.stored_value;
            retained.store_target = evaluated.store_target;
            retained.completed_writes = evaluated.writes.writes;
            retained.completed_unknown_write = evaluated.writes.unknown;
        }

        // Assignment evaluates its RHS under the entry loans above, then
        // overwrites the target. Retire loans carried by the old value before
        // activating loans carried by the replacement below; otherwise the
        // replacement spuriously conflicts with the value it is replacing.
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

        while let Some(loan) = borrow_loans.get(loan_index) {
            if loan.statement_index != statement_index {
                break;
            }
            loan_index += 1;
            let loan_handle = Handle::from_parts(
                borrow_state
                    .loans
                    .start()
                    .arena_index()
                    .saturating_add((loan_index - 1) as u32),
                borrow_state.loans.start().generation(),
            );

            let formation_value = {
                let mut captures = build
                    .control
                    .operands
                    .iter()
                    .skip(operand_capture_start)
                    .filter(|(_, operand)| {
                        operand.expression == loan.formation_expression
                            && operand.state_symbol == state.symbol
                            && operand.statement_index == statement_index
                    });
                let first = captures
                    .next()
                    .map(|(handle, _)| handle)
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
            let captured_destination = build.control.statements.get(statement_fact).store_target;
            let contexts = if build.control.operands.is_valid(captured_destination) {
                build
                    .contexts
                    .constraint_refs
                    .span_or_empty(build.control.operands.get(captured_destination).constraints)
                    .iter()
                    .filter_map(|constraint| match constraint.kind {
                        FlowConstraintKind::SemanticContext { context } => Some(context),
                        _ => None,
                    })
                    .collect()
            } else {
                crate::flow::active_context_handles(build, *active_contexts)
            };
            let (target, stable) = super::transfers::narrow_pinned_selectors(
                program,
                semantic,
                state.symbol,
                statement_index,
                &contexts,
                write.target_place,
            );
            write.target_place = target;
            write.stable_value_target = stable && write.target_is_exact;
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
                        unknown_alias_footprints(program, state, statement_index, places)
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
        {
            let context_handles: Vec<facts::FactContextHandle> = build
                .contexts
                .semantic_context_refs
                .span_or_empty(assignment_source_contexts)
                .iter()
                .map(|context_ref| context_ref.context)
                .collect();
            for place in mutated_places
                .iter_mut()
                .filter(|_| statement.as_assignment().is_none())
            {
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
    let has_proof_only_binding = proof.proof_output_calls.iter().any(|(_, invocation)| {
        invocation.caller_machine_symbol == machine_symbol
            && invocation.caller_state_symbol == state_symbol
            && invocation.statement_index == statement_index
            && invocation.runtime_call.is_none()
    });
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
/// that may hold a reference. Primitive locals hold no reference and stay
/// disjoint from the store; aggregates may embed one, so they are retired.
fn unknown_alias_footprints(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    places: &[validation::CanonicalPlace],
) -> Vec<validation::CanonicalPlace> {
    let mut footprints = places.to_vec();
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if program
            .primitive_type_reference(local.type_reference)
            .is_some()
        {
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

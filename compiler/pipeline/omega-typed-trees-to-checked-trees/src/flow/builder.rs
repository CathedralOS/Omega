use super::*;
use omega_facts::PlaceHandle;

pub(crate) fn build_flow_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
) -> FlowFacts {
    let mut ctx = FlowBuildContext::new(borrow, proof, semantic);

    for machine in program.machines() {
        let machine_effects = effects_machine(effects, machine.symbol);

        for state in program.machine_states(machine) {
            build_state_flow_fact(
                program,
                borrow,
                proof,
                semantic,
                domains,
                effects,
                &mut ctx,
                machine,
                state,
                machine_effects,
            );
        }
    }

    ctx.finish()
}

struct FlowBuildContext {
    state_mutation_summary_cache: StateMutationSummaryCache,
    semantic_context_refs: omega_core::arena::Arena<FlowSemanticContextRef>,
    constraint_refs: omega_core::arena::Arena<FlowConstraintRef>,
    invalidation_segments: omega_core::arena::Arena<omega_facts::PlaceSegment>,
    invalidations: omega_core::arena::Arena<FlowInvalidationFact>,
    borrow_activations: omega_core::arena::Arena<FlowBorrowActivationFact>,
    borrow_weakenings: omega_core::arena::Arena<FlowBorrowWeakeningFact>,
    statements: omega_core::arena::Arena<FlowStatementFact>,
    calls: omega_core::arena::Arena<FlowCallFact>,
    exits: omega_core::arena::Arena<FlowExitFact>,
    states: omega_core::arena::Arena<FlowStateFact>,
}

impl FlowBuildContext {
    fn new(borrow: &BorrowFacts, proof: &ProofFacts, semantic: &FactPlan) -> Self {
        Self {
            state_mutation_summary_cache: StateMutationSummaryCache::default(),
            semantic_context_refs: omega_core::arena::Arena::with_capacity(
                semantic.contexts.len().saturating_mul(2),
            ),
            constraint_refs: omega_core::arena::Arena::with_capacity(
                semantic.contexts.len().saturating_mul(3)
                    .saturating_add(borrow.states.len())
                    .saturating_add(borrow.calls.len())
                    .saturating_add(borrow.loans.len()),
            ),
            invalidation_segments: omega_core::arena::Arena::default(),
            invalidations: omega_core::arena::Arena::default(),
            borrow_activations: omega_core::arena::Arena::default(),
            borrow_weakenings: omega_core::arena::Arena::default(),
            statements: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            calls: omega_core::arena::Arena::with_capacity(borrow.calls.len()),
            exits: omega_core::arena::Arena::with_capacity(proof.contract_exits.len()),
            states: omega_core::arena::Arena::with_capacity(borrow.states.len()),
        }
    }

    fn finish(self) -> FlowFacts {
        FlowFacts {
            semantic_context_refs: self.semantic_context_refs,
            constraint_refs: self.constraint_refs,
            invalidation_segments: self.invalidation_segments,
            invalidations: self.invalidations,
            borrow_activations: self.borrow_activations,
            borrow_weakenings: self.borrow_weakenings,
            statements: self.statements,
            calls: self.calls,
            exits: self.exits,
            states: self.states,
        }
    }
}

fn build_state_flow_fact(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    machine_effects: Option<&omega_effects::MachineEffects>,
) {
    let Some((borrow_state_handle, borrow_state)) =
        borrow_state_fact(borrow, machine.symbol, state.symbol)
    else {
        return;
    };

    let state_effects = effects_state(effects, machine_effects, state.symbol);
    let mut state_contexts = omega_core::arena::HandleSpan::empty();
    let mut state_constraints = omega_core::arena::HandleSpan::empty();
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.semantic_context_refs,
        &mut state_contexts,
        &[
            ProgramPoint::Global,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
        ],
    );
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.constraint_refs,
        &mut state_constraints,
        &[
            ProgramPoint::Global,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
        ],
    );
    append_constraint_ref(
        &mut ctx.constraint_refs,
        &mut state_constraints,
        FlowConstraintKind::BorrowState {
            state: borrow_state_handle,
        },
    );
    append_contiguous_borrow_root_constraints(
        &mut ctx.constraint_refs,
        &mut state_constraints,
        borrow_state.writable_roots,
    );
    let mut active_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, state_contexts);
    let mut active_constraints =
        clone_constraint_refs(&mut ctx.constraint_refs, state_constraints);
    let state_invalidations_start = ctx.invalidations.len();
    let state_borrow_activations_start = ctx.borrow_activations.len();
    let state_borrow_weakenings_start = ctx.borrow_weakenings.len();
    let state_statements_start = ctx.statements.len();
    let state_calls = append_state_call_facts(
        program,
        borrow,
        proof,
        semantic,
        domains,
        effects,
        ctx,
        machine,
        state,
        state_effects,
        &mut active_contexts,
        &mut active_constraints,
        borrow_state,
    );
    active_constraints = filter_expired_borrow_loans(
        &mut ctx.borrow_weakenings,
        &mut ctx.constraint_refs,
        active_constraints,
        borrow,
        program.statement_table.statements(state.statement_nodes).len(),
        FlowBorrowWeakeningReason::StateExit,
    );
    let state_exits = append_state_exit_facts(
        proof,
        semantic,
        ctx,
        machine.symbol,
        state.symbol,
        active_contexts,
        active_constraints,
    );

    ctx.states.append(FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        writable_roots: borrow_state.writable_roots,
        mutable_parameter_count: borrow_state.mutable_parameter_count,
        entry_semantic_contexts: state_contexts,
        entry_constraints: state_constraints,
        invalidations: appended_span_since(&ctx.invalidations, state_invalidations_start),
        borrow_activations: appended_span_since(
            &ctx.borrow_activations,
            state_borrow_activations_start,
        ),
        borrow_weakenings: appended_span_since(
            &ctx.borrow_weakenings,
            state_borrow_weakenings_start,
        ),
        statements: appended_span_since(&ctx.statements, state_statements_start),
        calls: state_calls,
        exits: state_exits,
        direct_effects: state_effects
            .map(|state_effects| state_effects.direct)
            .unwrap_or_else(omega_effects::EffectSet::empty),
        transitive_effects: state_effects
            .map(|state_effects| state_effects.transitive)
            .unwrap_or_else(omega_effects::EffectSet::empty),
    });
}

#[allow(clippy::too_many_arguments)]
fn append_state_call_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    state_effects: Option<&omega_effects::StateEffects>,
    active_contexts: &mut omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow_state: &StateBorrowFact,
) -> omega_core::arena::HandleSpan<FlowCallFact> {
    let mut state_calls = omega_core::arena::HandleSpan::empty();
    let borrow_calls = borrow.calls.span_or_empty(borrow_state.calls);
    let borrow_loans = borrow.loans.span_or_empty(borrow_state.loans);
    let mut call_index = 0usize;
    let mut loan_index = 0usize;

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        *active_constraints = filter_expired_borrow_loans(
            &mut ctx.borrow_weakenings,
            &mut ctx.constraint_refs,
            *active_constraints,
            borrow,
            statement_index,
            FlowBorrowWeakeningReason::LastUseExpired,
        );
        ctx.statements.append(FlowStatementFact {
            statement_index,
            entry_semantic_contexts: *active_contexts,
            entry_constraints: *active_constraints,
        });

        while let Some(borrow_call) = borrow_calls.get(call_index) {
            if borrow_call.statement_index != statement_index {
                break;
            }
            call_index += 1;

            let call_flow = build_call_flow_fact(
                program,
                borrow,
                proof,
                semantic,
                domains,
                effects,
                ctx,
                machine,
                state,
                state_effects,
                active_contexts,
                active_constraints,
                borrow_call,
            );
            ctx.calls.append_to_span(&mut state_calls, call_flow);
        }

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

            ctx.borrow_activations.append(FlowBorrowActivationFact {
                source: FlowInvalidationSource::Statement { statement_index },
                loan: loan_handle,
            });

            append_constraint_ref(
                &mut ctx.constraint_refs,
                active_constraints,
                FlowConstraintKind::BorrowLoan {
                    loan: loan_handle,
                },
            );
        }

        if let Some(place) = statement_mutated_place(program, machine, statement) {
            *active_contexts = filter_contexts_after_place_mutations(
                program,
                semantic,
                domains,
                &mut ctx.semantic_context_refs,
                &mut ctx.invalidation_segments,
                &mut ctx.invalidations,
                *active_contexts,
                &[place],
                FlowInvalidationSource::Statement { statement_index },
            );
            *active_constraints = project_constraint_refs_to_active_contexts(
                &mut ctx.constraint_refs,
                *active_constraints,
                *active_contexts,
                &ctx.semantic_context_refs,
            );
        }

        propagate_statement_transfers(
            program,
            semantic,
            ctx,
            machine.symbol,
            state.symbol,
            statement_index,
            statement,
            active_contexts,
            active_constraints,
        );
    }

    state_calls
}

fn propagate_statement_transfers(
    program: &omega_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let (target_place, source_expression, source_place) = match statement {
        StatementNode::LocalData(local_data) => (
            semantic.append_symbol_place(local_data.symbol),
            local_data.initial_value,
            contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                local_data.initial_value,
            ),
        ),
        StatementNode::Assignment(assignment) => {
            let Some(target_place) = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                assignment.target,
            )
            else {
                return;
            };
            let source_place = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                assignment.value,
            );
            (target_place, assignment.value, source_place)
        }
        StatementNode::Call(_)
        | StatementNode::Expression(_)
        | StatementNode::Transition(_) => return,
    };
    let source_label = program.expression_table.display_name(source_expression);

    let mut refs = HandleSpan::empty();
    let context_handles: Vec<_> = ctx
        .semantic_context_refs
        .span_or_empty(*active_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();

    for context_handle in context_handles {
        let context = semantic.contexts.get(context_handle);
        let facts_to_transfer: Vec<_> = semantic
            .context_view(context)
            .facts()
            .filter_map(|fact| match fact.payload {
                FactPayload::DomainMembership { domain, domain_symbol, .. }
                | FactPayload::ContractDomainMembership {
                    domain,
                    domain_symbol,
                    ..
                } => {
                    let FactPlace::Place(fact_place) = fact.place else {
                        return None;
                    };
                    let fact_label = crate::labels::canonical_place_label(
                        program,
                        semantic,
                        semantic.places.get(fact_place),
                    );
                    (source_place.is_some_and(|source_place| {
                        semantic.places_match(program, fact_place, source_place)
                    }) || fact_label == source_label)
                        .then_some((domain, domain_symbol))
                }
                _ => None,
            })
            .collect();

        for (domain, domain_symbol) in facts_to_transfer {
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(target_place),
                point: ProgramPoint::Statement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                },
                origin: FactOrigin::StatementTransfer,
                payload: FactPayload::DomainMembership {
                    value: ExpressionHandle::invalid(),
                    domain,
                    domain_symbol,
                },
            });
            semantic.append_ref(&mut refs, fact);
        }
    }

    if refs.is_empty() {
        return;
    }

    let context = semantic.append_context(
        ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        },
        refs,
    );
    let mut next_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, *active_contexts);
    ctx.semantic_context_refs
        .append_to_span(&mut next_contexts, FlowSemanticContextRef { context });
    *active_contexts = next_contexts;
    let mut next_constraints = clone_constraint_refs(&mut ctx.constraint_refs, *active_constraints);
    append_constraint_ref(
        &mut ctx.constraint_refs,
        &mut next_constraints,
        FlowConstraintKind::SemanticContext { context },
    );
    *active_constraints = next_constraints;
}

fn contextual_expression_place(
    program: &omega_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    if let Some(place) = crate::semantic_places::canonical_place_to_fact_place(program, semantic, expression)
    {
        return Some(place);
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => contextual_expression_place(
            program,
            semantic,
            machine_symbol,
            state_symbol,
            statement_index,
            *inner,
        ),
        ExpressionNode::Name(path) => {
            let name = program.expression_table.display_name(expression);
            let state = crate::find_state(program, state_symbol)?;

            if name == "self" {
                let self_symbol = program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.is_self)
                    .map(|parameter| parameter.symbol)?;
                return Some(semantic.append_symbol_place(self_symbol));
            }

            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| !parameter.is_self && parameter.name.as_str() == name)
            {
                return Some(semantic.append_symbol_place(parameter.symbol));
            }

            for prior in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
            {
                let StatementNode::LocalData(local_data) = prior else {
                    continue;
                };
                if local_data.name.as_str() == name {
                    return Some(semantic.append_symbol_place(local_data.symbol));
                }
            }

            let _ = path;
            None
        }
        ExpressionNode::Member(member) => {
            let receiver = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                member.receiver,
            )?;
            let symbol = crate::flow::effective_member_symbol(program, member.receiver, member);
            let symbol = if symbol.is_valid() {
                symbol
            } else {
                crate::semantic_places::resolve_place_member_symbol(
                    program,
                    semantic,
                    receiver,
                    member.member.as_str(),
                )?
            };
            Some(crate::semantic_places::append_place_segment(
                semantic,
                receiver,
                omega_facts::PlaceSegment::Field { symbol },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                indexed.collection,
            )?;
            Some(crate::semantic_places::append_place_segment(
                semantic,
                collection,
                omega_facts::PlaceSegment::Index {
                    expression: indexed.index,
                },
            ))
        }
        _ => {
            let _ = machine_symbol;
            None
        }
    }
}

fn filter_expired_borrow_loans(
    borrow_weakenings: &mut omega_core::arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut omega_core::arena::Arena<FlowConstraintRef>,
    source: omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    statement_index: usize,
    reason: FlowBorrowWeakeningReason,
) -> omega_core::arena::HandleSpan<FlowConstraintRef> {
    common::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
        match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let keep = borrow.loans.get(loan).last_use_statement_index >= statement_index;
                if !keep {
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement { statement_index },
                        loan,
                        reason,
                    });
                }
                keep
            }
            FlowConstraintKind::Unknown
            | FlowConstraintKind::SemanticContext { .. }
            | FlowConstraintKind::BorrowState { .. }
            | FlowConstraintKind::BorrowCall { .. }
            | FlowConstraintKind::BorrowWritableRoot { .. }
            | FlowConstraintKind::BorrowAccess { .. } => true,
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn build_call_flow_fact(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    state_effects: Option<&omega_effects::StateEffects>,
    active_contexts: &mut omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
) -> FlowCallFact {
    let effect_call = effects_call(effects, state_effects, borrow_call);
    let contract_call = proof_contract_call(
        proof,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    let entry_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, *active_contexts);
    let mut entry_constraints = clone_constraint_refs(&mut ctx.constraint_refs, *active_constraints);
    if let Some((borrow_call_handle, _)) = borrow.calls.iter().find(|(_, call)| {
        call.statement_index == borrow_call.statement_index
            && call.call_ordinal == borrow_call.call_ordinal
            && call.target_symbol == borrow_call.target_symbol
            && call.receiver_symbol == borrow_call.receiver_symbol
    }) {
        append_constraint_ref(
            &mut ctx.constraint_refs,
            &mut entry_constraints,
            FlowConstraintKind::BorrowCall {
                call: borrow_call_handle,
            },
        );
    }
    append_contiguous_borrow_access_constraints(
        &mut ctx.constraint_refs,
        &mut entry_constraints,
        borrow_call.accesses,
    );
    let mut requires_contexts = omega_core::arena::HandleSpan::empty();
    let mut requires_constraints = omega_core::arena::HandleSpan::empty();
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.semantic_context_refs,
        &mut requires_contexts,
        &[ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.constraint_refs,
        &mut requires_constraints,
        &[ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    let mutated_places = call_mutated_places(
        program,
        machine.symbol,
        state.symbol,
        borrow,
        borrow_call,
        &mut ctx.state_mutation_summary_cache,
    );
    let call_invalidations_start = ctx.invalidations.len();
    let post_call_contexts = if call_may_mutate_contract_state(program, borrow, borrow_call) {
        if mutated_places.is_empty() {
            omega_core::arena::HandleSpan::empty()
        } else {
            filter_contexts_after_place_mutations(
                program,
                semantic,
                domains,
                &mut ctx.semantic_context_refs,
                &mut ctx.invalidation_segments,
                &mut ctx.invalidations,
                *active_contexts,
                &mutated_places,
                FlowInvalidationSource::Call {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    target_symbol: borrow_call.target_symbol,
                },
            )
        }
    } else {
        clone_flow_contexts(&mut ctx.semantic_context_refs, *active_contexts)
    };
    let post_call_constraints = project_constraint_refs_to_active_contexts(
        &mut ctx.constraint_refs,
        *active_constraints,
        post_call_contexts,
        &ctx.semantic_context_refs,
    );
    let call_invalidations = appended_span_since(&ctx.invalidations, call_invalidations_start);
    let mut exit_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, post_call_contexts);
    let mut exit_constraints =
        clone_constraint_refs(&mut ctx.constraint_refs, post_call_constraints);
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.semantic_context_refs,
        &mut exit_contexts,
        &[ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.constraint_refs,
        &mut exit_constraints,
        &[ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    *active_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, exit_contexts);
    *active_constraints = clone_constraint_refs(&mut ctx.constraint_refs, exit_constraints);

    FlowCallFact {
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
        receiver_symbol: borrow_call.receiver_symbol,
        target_symbol: borrow_call.target_symbol,
        has_receiver: borrow_call.has_receiver,
        accesses: borrow_call.accesses,
        entry_semantic_contexts: entry_contexts,
        entry_constraints,
        requires_contexts,
        requires_constraints,
        exit_semantic_contexts: exit_contexts,
        exit_constraints,
        invalidations: call_invalidations,
        requires: contract_call
            .map(|call| call.requires)
            .unwrap_or_else(HandleSpan::empty),
        ensures: contract_call
            .map(|call| call.ensures)
            .unwrap_or_else(HandleSpan::empty),
        direct_effects: effect_call
            .map(|call| call.direct)
            .unwrap_or_else(omega_effects::EffectSet::empty),
        transitive_effects: effect_call
            .map(|call| call.transitive)
            .unwrap_or_else(omega_effects::EffectSet::empty),
    }
}

fn append_contiguous_borrow_root_constraints(
    constraint_refs: &mut omega_core::arena::Arena<FlowConstraintRef>,
    refs: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    roots: omega_core::arena::HandleSpan<BorrowWritableRootFact>,
) {
    let start = roots.start();
    if !start.is_valid() {
        return;
    }

    for offset in 0..roots.count() {
        append_constraint_ref(
            constraint_refs,
            refs,
            FlowConstraintKind::BorrowWritableRoot {
                root: Handle::from_parts(
                    start.arena_index() + offset,
                    start.generation(),
                ),
            },
        );
    }
}

fn append_contiguous_borrow_access_constraints(
    constraint_refs: &mut omega_core::arena::Arena<FlowConstraintRef>,
    refs: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    accesses: omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
) {
    let start = accesses.start();
    if !start.is_valid() {
        return;
    }

    for offset in 0..accesses.count() {
        append_constraint_ref(
            constraint_refs,
            refs,
            FlowConstraintKind::BorrowAccess {
                access: Handle::from_parts(
                    start.arena_index() + offset,
                    start.generation(),
                ),
            },
        );
    }
}

fn append_state_exit_facts(
    proof: &ProofFacts,
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    active_contexts: omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: omega_core::arena::HandleSpan<FlowConstraintRef>,
) -> omega_core::arena::HandleSpan<FlowExitFact> {
    let mut state_exits = omega_core::arena::HandleSpan::empty();

    for contract_exit in proof.contract_exits.iter().filter_map(|(_, exit)| {
        (exit.machine_symbol == machine_symbol && exit.state_symbol == state_symbol).then_some(exit)
    }) {
        let entry_exit_contexts =
            clone_flow_contexts(&mut ctx.semantic_context_refs, active_contexts);
        let entry_constraints =
            clone_constraint_refs(&mut ctx.constraint_refs, active_constraints);
        let mut ensures_contexts = omega_core::arena::HandleSpan::empty();
        let mut ensures_constraints = omega_core::arena::HandleSpan::empty();
        append_flow_contexts_for_points(
            semantic,
            &mut ctx.semantic_context_refs,
            &mut ensures_contexts,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
            }],
        );
        append_semantic_constraints_for_points(
            semantic,
            &mut ctx.constraint_refs,
            &mut ensures_constraints,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
            }],
        );

        ctx.exits.append_to_span(
            &mut state_exits,
            FlowExitFact {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                entry_semantic_contexts: entry_exit_contexts,
                entry_constraints,
                ensures_contexts,
                ensures_constraints,
                ensures: contract_exit.ensures,
            },
        );
    }

    state_exits
}

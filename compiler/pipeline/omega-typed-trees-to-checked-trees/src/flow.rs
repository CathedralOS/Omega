use super::*;
use crate::lookup::{
    expression_root_symbol, first_valid_name_path_symbol, machine_by_symbol,
    machine_symbol_from_type_reference_handle, statement_call_receiver_path,
};

mod common;
mod mutation;

use common::{
    append_flow_contexts_for_points, append_place_segments, appended_span_since,
    borrow_state_fact, clone_flow_contexts, effects_call, effects_machine, effects_state,
    proof_contract_call,
};
pub(crate) use mutation::{call_mutated_places, StateMutationSummaryCache};
use mutation::{
    call_may_mutate_contract_state, statement_mutated_place,
};

pub(crate) fn build_domain_facts(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
) -> DomainFacts {
    let mut cache = DomainDependencyCache::default();
    let mut segments = omega_core::arena::Arena::new();
    let mut dependency_paths = omega_core::arena::Arena::new();
    let mut dependencies =
        omega_core::arena::Arena::with_capacity(program.domain_definitions().len());

    for domain in program.domain_definitions() {
        let dependency_segments =
            domain_dependency_segments(program, semantic, &mut cache, domain.symbol);
        let mut dependency_span = omega_core::arena::HandleSpan::empty();
        for dependency in dependency_segments {
            let mut segment_span = omega_core::arena::HandleSpan::empty();
            for segment in dependency {
                segments.append_to_span(&mut segment_span, *segment);
            }
            dependency_paths.append_to_span(
                &mut dependency_span,
                DomainDependencyPathFact {
                    segments: segment_span,
                },
            );
        }

        dependencies.append(DomainDependencyFact {
            domain_symbol: domain.symbol,
            dependencies: dependency_span,
        });
    }

    DomainFacts {
        segments,
        dependency_paths,
        dependencies,
    }
}

pub(crate) fn build_flow_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
) -> FlowFacts {
    let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
    let mut semantic_context_refs =
        omega_core::arena::Arena::with_capacity(semantic.contexts.len().saturating_mul(2));
    let mut invalidation_segments = omega_core::arena::Arena::default();
    let mut invalidations = omega_core::arena::Arena::default();
    let mut calls = omega_core::arena::Arena::with_capacity(borrow.calls.len());
    let mut exits = omega_core::arena::Arena::with_capacity(proof.contract_exits.len());
    let mut states = omega_core::arena::Arena::with_capacity(borrow.states.len());

    for machine in program.machines() {
        let machine_effects = effects_machine(effects, machine.symbol);

        for state in program.machine_states(machine) {
            let Some(borrow_state) = borrow_state_fact(borrow, machine.symbol, state.symbol) else {
                continue;
            };
            let state_effects = effects_state(effects, machine_effects, state.symbol);
            let mut state_contexts = omega_core::arena::HandleSpan::empty();
            append_flow_contexts_for_points(
                semantic,
                &mut semantic_context_refs,
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
            let mut active_contexts =
                clone_flow_contexts(&mut semantic_context_refs, state_contexts);
            let state_invalidations_start = invalidations.len();

            let mut state_calls = omega_core::arena::HandleSpan::empty();
            let borrow_calls = borrow.calls.span_or_empty(borrow_state.calls);
            let mut call_index = 0usize;
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                while let Some(borrow_call) = borrow_calls.get(call_index) {
                    if borrow_call.statement_index != statement_index {
                        break;
                    }
                    call_index += 1;

                    let effect_call = effects_call(effects, state_effects, borrow_call);
                    let contract_call = proof_contract_call(
                        proof,
                        machine.symbol,
                        state.symbol,
                        borrow_call.statement_index,
                        borrow_call.call_ordinal,
                    );
                    let entry_contexts =
                        clone_flow_contexts(&mut semantic_context_refs, active_contexts);
                    let mut requires_contexts = omega_core::arena::HandleSpan::empty();
                    append_flow_contexts_for_points(
                        semantic,
                        &mut semantic_context_refs,
                        &mut requires_contexts,
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
                        &mut state_mutation_summary_cache,
                    );
                    let call_invalidations_start = invalidations.len();
                    let post_call_contexts =
                        if call_may_mutate_contract_state(program, borrow, borrow_call) {
                            if mutated_places.is_empty() {
                                omega_core::arena::HandleSpan::empty()
                            } else {
                                filter_contexts_after_place_mutations(
                                    program,
                                    semantic,
                                    domains,
                                    &mut semantic_context_refs,
                                    &mut invalidation_segments,
                                    &mut invalidations,
                                    active_contexts,
                                    &mutated_places,
                                    FlowInvalidationSource::Call {
                                        statement_index: borrow_call.statement_index,
                                        call_ordinal: borrow_call.call_ordinal,
                                        target_symbol: borrow_call.target_symbol,
                                    },
                                )
                            }
                        } else {
                            clone_flow_contexts(&mut semantic_context_refs, active_contexts)
                        };
                    let call_invalidations =
                        appended_span_since(&invalidations, call_invalidations_start);
                    let mut exit_contexts =
                        clone_flow_contexts(&mut semantic_context_refs, post_call_contexts);
                    append_flow_contexts_for_points(
                        semantic,
                        &mut semantic_context_refs,
                        &mut exit_contexts,
                        &[ProgramPoint::CallEnsures {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            statement_index: borrow_call.statement_index,
                            call_ordinal: borrow_call.call_ordinal,
                        }],
                    );
                    active_contexts =
                        clone_flow_contexts(&mut semantic_context_refs, exit_contexts);

                    calls.append_to_span(
                        &mut state_calls,
                        FlowCallFact {
                            statement_index: borrow_call.statement_index,
                            call_ordinal: borrow_call.call_ordinal,
                            receiver_symbol: borrow_call.receiver_symbol,
                            target_symbol: borrow_call.target_symbol,
                            has_receiver: borrow_call.has_receiver,
                            accesses: borrow_call.accesses,
                            entry_semantic_contexts: entry_contexts,
                            requires_contexts,
                            exit_semantic_contexts: exit_contexts,
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
                        },
                    );
                }

                if let Some(place) =
                    statement_mutated_place(program, machine, statement)
                {
                    active_contexts = filter_contexts_after_place_mutations(
                        program,
                        semantic,
                        domains,
                        &mut semantic_context_refs,
                        &mut invalidation_segments,
                        &mut invalidations,
                        active_contexts,
                        &[place],
                        FlowInvalidationSource::Statement { statement_index },
                    );
                }
            }

            let mut state_exits = omega_core::arena::HandleSpan::empty();
            for contract_exit in proof.contract_exits.iter().filter_map(|(_, exit)| {
                (exit.machine_symbol == machine.symbol && exit.state_symbol == state.symbol)
                    .then_some(exit)
            }) {
                let entry_exit_contexts =
                    clone_flow_contexts(&mut semantic_context_refs, active_contexts);
                let mut ensures_contexts = omega_core::arena::HandleSpan::empty();
                append_flow_contexts_for_points(
                    semantic,
                    &mut semantic_context_refs,
                    &mut ensures_contexts,
                    &[ProgramPoint::Exit {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index: contract_exit.statement_index,
                    }],
                );

                exits.append_to_span(
                    &mut state_exits,
                    FlowExitFact {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index: contract_exit.statement_index,
                        entry_semantic_contexts: entry_exit_contexts,
                        ensures_contexts,
                        ensures: contract_exit.ensures,
                    },
                );
            }

            states.append(FlowStateFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: borrow_state.writable_roots,
                mutable_parameter_count: borrow_state.mutable_parameter_count,
                entry_semantic_contexts: state_contexts,
                invalidations: appended_span_since(&invalidations, state_invalidations_start),
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
    }

    FlowFacts {
        semantic_context_refs,
        invalidation_segments,
        invalidations,
        calls,
        exits,
        states,
    }
}

fn filter_contexts_after_place_mutations(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &DomainFacts,
    semantic_context_refs: &mut omega_core::arena::Arena<FlowSemanticContextRef>,
    invalidation_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    invalidations: &mut omega_core::arena::Arena<FlowInvalidationFact>,
    source: omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    mutated_places: &[CanonicalPlace],
    invalidation_source: FlowInvalidationSource,
) -> omega_core::arena::HandleSpan<FlowSemanticContextRef> {
    if mutated_places.is_empty() {
        return source;
    }

    let mut filtered = omega_core::arena::HandleSpan::empty();
    let mut removed_any = false;
    let copied: Vec<_> = semantic_context_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();
    for context_ref in copied {
        let context = semantic.contexts.get(context_ref.context);
        let mut invalidated_any = false;
        for fact_ref in semantic.refs.span_or_empty(context.facts) {
            let fact = semantic.facts.get(fact_ref.fact);
            let FactPlace::Place(place) = fact.place else {
                continue;
            };
            let Some((mutated_place, dependency_segments)) = matching_mutation_for_fact_place(
                program,
                semantic,
                domain_dependencies,
                fact,
                place,
                mutated_places,
            ) else {
                continue;
            };

            invalidated_any = true;
            removed_any = true;
            invalidations.append(FlowInvalidationFact {
                source: invalidation_source,
                context: context_ref.context,
                fact: fact_ref.fact,
                mutated_root: mutated_place.root,
                mutated_segments: append_place_segments(
                    invalidation_segments,
                    &mutated_place.segments,
                ),
                dependency_segments: append_place_segments(
                    invalidation_segments,
                    dependency_segments,
                ),
            });
        }

        if !invalidated_any {
            semantic_context_refs.append_to_span(&mut filtered, context_ref);
        }
    }

    if removed_any { filtered } else { source }
}

#[derive(Debug, Clone, Default)]
struct DomainDependencyCache {
    by_domain: Vec<DomainDependencyCacheEntry>,
}

#[derive(Debug, Clone)]
struct DomainDependencyCacheEntry {
    domain_symbol: SymbolHandle,
    dependencies: Vec<Vec<omega_facts::PlaceSegment>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalPlace {
    pub(crate) root: omega_facts::PlaceRoot,
    pub(crate) segments: Vec<omega_facts::PlaceSegment>,
}

impl CanonicalPlace {
    fn extend_segments(&mut self, segments: &[omega_facts::PlaceSegment]) {
        self.segments.extend(segments.iter().copied());
    }
}

pub(crate) fn canonical_place_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => canonical_place_from_expression(program, *inner),
        ExpressionNode::Name(path) => {
            let root_symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            let segments = program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .skip(1)
                .copied()
                .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                .collect();
            Some(CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(root_symbol),
                segments,
            })
        }
        ExpressionNode::Member(member) => {
            let mut place = canonical_place_from_expression(program, member.receiver)?;
            place.segments.push(omega_facts::PlaceSegment::Field {
                symbol: effective_member_symbol(program, member.receiver, member),
            });
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place = canonical_place_from_expression(program, indexed.collection)?;
            place.segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(place)
        }
        _ => Some(CanonicalPlace {
            root: omega_facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        }),
    }
}

fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
    symbol.is_valid().then_some(CanonicalPlace {
        root: omega_facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    })
}

fn canonical_place_from_semantic_place(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> Option<CanonicalPlace> {
    let mut canonical = match place.root {
        omega_facts::PlaceRoot::Unknown => return None,
        omega_facts::PlaceRoot::Symbol(symbol) => canonical_place_from_symbol(symbol)?,
        omega_facts::PlaceRoot::Expression(expression) => {
            canonical_place_from_expression(program, expression)?
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => CanonicalPlace {
            root: omega_facts::PlaceRoot::TypeReference(type_reference),
            segments: Vec::new(),
        },
    };
    canonical.extend_segments(semantic.place_segments.span_or_empty(place.segments));
    Some(canonical)
}

pub(crate) fn effective_member_symbol(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member: &omega_typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    if let Some(symbol) =
        resolve_member_symbol_from_receiver(program, receiver, member.member.as_str())
    {
        return symbol;
    }

    if member.member_symbol.is_valid() {
        return member.member_symbol;
    }

    SymbolHandle::invalid()
}

fn resolve_member_symbol_from_receiver(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = expression_type_symbol(program, receiver)?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = machine_by_symbol(program, type_symbol) {
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.name.as_str() == member_name {
                return Some(contained.symbol);
            }
        }
    }

    None
}

pub(crate) fn expression_type_symbol(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_type_symbol(program, *inner),
        ExpressionNode::Name(path) => {
            let symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            symbol_type_symbol(program, symbol)
        }
        ExpressionNode::Member(member) => {
            let symbol = effective_member_symbol(program, member.receiver, member);
            symbol_type_symbol(program, symbol)
        }
        _ => None,
    }
}

pub(crate) fn symbol_type_symbol(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !symbol.is_valid() {
        return None;
    }

    for machine in program.machines() {
        if machine.symbol == symbol {
            if let Some(attached_data) = machine.attached_data.as_deref() {
                if let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == attached_data)
                {
                    return Some(data.symbol);
                }
            }
        }
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return Some(machine_symbol_from_type_reference_handle(
                        program,
                        parameter.type_reference,
                    ));
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return Some(machine_symbol_from_type_reference_handle(
                    program,
                    owned.type_reference,
                ));
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.symbol == symbol {
                return Some(contained.type_symbol);
            }
        }
    }

    for data in program.data_definitions() {
        for member in program.data_members(data) {
            if let omega_typed_trees::data::DataMember::Field(field) = member
                && field.symbol == symbol
            {
                return Some(machine_symbol_from_type_reference_handle(
                    program,
                    field.type_reference,
                ));
            }
        }
    }

    None
}

fn canonical_place_segments_equal(
    left: omega_facts::PlaceSegment,
    right: omega_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            omega_facts::PlaceSegment::Field { symbol: left_symbol },
            omega_facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            omega_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            omega_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => left_expression == right_expression,
        _ => false,
    }
}

fn canonical_place_overlaps_segments(
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| {
            canonical_place_segments_equal(*left_segment, *right_segment)
        })
}

fn canonical_place_overlaps_joined_segments(
    prefix: &[omega_facts::PlaceSegment],
    suffix: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = prefix
        .len()
        .saturating_add(suffix.len())
        .min(right.len());

    (0..shared_len).all(|index| {
        let left_segment = if index < prefix.len() {
            prefix[index]
        } else {
            suffix[index - prefix.len()]
        };
        canonical_place_segments_equal(left_segment, right[index])
    })
}

fn matching_mutation_for_fact_place<'a, 'b>(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: omega_facts::PlaceHandle,
    mutated_places: &'b [CanonicalPlace],
) -> Option<(&'b CanonicalPlace, &'a [omega_facts::PlaceSegment])> {
    let place = semantic.places.get(fact_place);
    let fact_canonical_place = canonical_place_from_semantic_place(program, semantic, place)?;

    for mutated_place in mutated_places {
        let is_domain_membership = matches!(
            fact.payload,
            FactPayload::DomainMembership { .. } | FactPayload::ContractDomainMembership { .. }
        );
        if let Some(dependency_segments) = domain_membership_matching_dependency(
            domain_dependencies,
            fact,
            &fact_canonical_place,
            mutated_place,
        ) {
            return Some((mutated_place, dependency_segments));
        }

        if is_domain_membership {
            continue;
        }

        if fact_canonical_place.root == mutated_place.root
            && canonical_place_overlaps_segments(
                &fact_canonical_place.segments,
                &mutated_place.segments,
            )
        {
            return Some((mutated_place, &[]));
        }
    }

    None
}

fn domain_membership_matching_dependency<'a>(
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: &CanonicalPlace,
    mutated_place: &CanonicalPlace,
) -> Option<&'a [omega_facts::PlaceSegment]> {
    let domain_symbol = match fact.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
        _ => return None,
    };

    if fact_place.root != mutated_place.root {
        return None;
    }

    let Some(domain_dependency) = domain_dependencies.dependency_fact(domain_symbol) else {
        return canonical_place_overlaps_segments(&fact_place.segments, &mutated_place.segments)
            .then_some(&[]);
    };

    if domain_dependency.dependencies.is_empty() {
        return canonical_place_overlaps_segments(&fact_place.segments, &mutated_place.segments)
            .then_some(&[]);
    }

    domain_dependencies
        .dependency_paths(domain_dependency)
        .find(|dependency_segments| {
            canonical_place_overlaps_joined_segments(
                &fact_place.segments,
                dependency_segments,
                &mutated_place.segments,
            )
        })
}

fn domain_dependency_segments<'cache>(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &'cache mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
) -> &'cache [Vec<omega_facts::PlaceSegment>] {
    if !cache.by_domain.iter().any(|entry| entry.domain_symbol == domain_symbol) {
        let mut visiting = BTreeSet::new();
        let dependencies = compute_domain_dependency_segments(
            program,
            semantic,
            cache,
            domain_symbol,
            &mut visiting,
        );
        cache.by_domain.push(DomainDependencyCacheEntry {
            domain_symbol,
            dependencies,
        });
    }

    cache
        .by_domain
        .iter()
        .find(|entry| entry.domain_symbol == domain_symbol)
        .map(|entry| entry.dependencies.as_slice())
        .unwrap_or(&[])
}

fn compute_domain_dependency_segments(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
    visiting: &mut BTreeSet<u32>,
) -> Vec<Vec<omega_facts::PlaceSegment>> {
    if let Some(cached) = cache
        .by_domain
        .iter()
        .find(|entry| entry.domain_symbol == domain_symbol)
    {
        return cached.dependencies.clone();
    }
    let domain_key = domain_symbol.arena_index();
    if !visiting.insert(domain_key) {
        return vec![Vec::new()];
    }

    let mut dependencies = Vec::new();
    let self_type_symbol = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
        .map(|domain| machine_symbol_from_type_reference_handle(program, domain.target_type))
        .filter(|symbol| symbol.is_valid());
    for fact in semantic.facts_for_symbol(domain_symbol) {
        match fact.payload {
            FactPayload::BooleanExpression(expression) => {
                collect_dependency_paths_from_expression(
                    program,
                    expression,
                    self_type_symbol,
                    &mut dependencies,
                );
            }
            FactPayload::DomainMembership {
                domain_symbol: imported_domain,
                ..
            } => {
                let FactPlace::Place(place_handle) = fact.place else {
                    dependencies.push(Vec::new());
                    continue;
                };
                let place = semantic.places.get(place_handle);
                let base_segments: Vec<_> = semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .iter()
                    .copied()
                    .collect();
                let imported_dependencies = compute_domain_dependency_segments(
                    program,
                    semantic,
                    cache,
                    imported_domain,
                    visiting,
                );
                if imported_dependencies.is_empty() {
                    dependencies.push(base_segments);
                } else {
                    for imported in imported_dependencies {
                        let mut rebased = Vec::with_capacity(
                            base_segments.len().saturating_add(imported.len()),
                        );
                        rebased.extend(base_segments.iter().copied());
                        rebased.extend(imported);
                        dependencies.push(rebased);
                    }
                }
            }
            _ => {}
        }
    }

    visiting.remove(&domain_key);
    dedupe_dependency_segments(&mut dependencies);
    dependencies
}

fn collect_dependency_paths_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    self_type_symbol: Option<SymbolHandle>,
    dependencies: &mut Vec<Vec<omega_facts::PlaceSegment>>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_dependency_paths_from_expression(
                    program,
                    *value,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_dependency_paths_from_expression(
                program,
                binary.left,
                self_type_symbol,
                dependencies,
            );
            collect_dependency_paths_from_expression(
                program,
                binary.right,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_dependency_paths_from_expression(
                    program,
                    call.receiver,
                    self_type_symbol,
                    dependencies,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_dependency_paths_from_expression(
                    program,
                    *argument,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_dependency_paths_from_expression(
                program,
                cast.value,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    indexed.collection,
                    self_type_symbol,
                    dependencies,
                );
            }
            collect_dependency_paths_from_expression(
                program,
                indexed.index,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Member(member) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    member.receiver,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Mutable(inner) => {
            collect_dependency_paths_from_expression(
                program,
                *inner,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Name(_) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                collect_dependency_paths_from_expression(
                    program,
                    field.value,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

fn relative_place_segments_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    self_type_symbol: Option<SymbolHandle>,
) -> Option<Vec<omega_facts::PlaceSegment>> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            relative_place_segments_from_expression(program, *inner, self_type_symbol)
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let head = members.first()?.as_str();
            if head != "self" {
                return None;
            }

            Some(Vec::new())
        }
        ExpressionNode::Member(member) => {
            let mut segments = relative_place_segments_from_expression(
                program,
                member.receiver,
                self_type_symbol,
            )?;
            let member_symbol = if let Some(symbol) =
                resolve_member_symbol_from_type(program, self_type_symbol, member.member.as_str())
            {
                symbol
            } else {
                effective_member_symbol(program, member.receiver, member)
            };
            segments.push(omega_facts::PlaceSegment::Field {
                symbol: member_symbol,
            });
            Some(segments)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut segments = relative_place_segments_from_expression(
                program,
                indexed.collection,
                self_type_symbol,
            )?;
            segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(segments)
        }
        _ => None,
    }
}

fn resolve_member_symbol_from_type(
    program: &omega_typed_trees::TypedTrees,
    type_symbol: Option<SymbolHandle>,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = type_symbol?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = machine_by_symbol(program, type_symbol) {
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.name.as_str() == member_name {
                return Some(contained.symbol);
            }
        }
    }

    None
}

fn dedupe_dependency_segments(dependencies: &mut Vec<Vec<omega_facts::PlaceSegment>>) {
    let mut unique: Vec<Vec<omega_facts::PlaceSegment>> = Vec::with_capacity(dependencies.len());
    for dependency in dependencies.drain(..) {
        if !unique.iter().any(|existing| {
            existing.len() == dependency.len()
                && existing
                    .iter()
                    .zip(dependency.iter())
                    .all(|(left, right)| canonical_place_segments_equal(*left, *right))
        }) {
            unique.push(dependency);
        }
    }
    *dependencies = unique;
}

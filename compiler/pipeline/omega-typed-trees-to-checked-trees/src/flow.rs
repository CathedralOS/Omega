use super::*;
use crate::lookup::{
    expression_root_symbol, first_valid_name_path_symbol, machine_by_symbol,
    machine_symbol_from_type_reference_handle, statement_call_receiver_path,
};

mod common;
mod domain;
mod mutation;

use common::{
    append_flow_contexts_for_points, append_place_segments, appended_span_since,
    borrow_state_fact, clone_flow_contexts, effects_call, effects_machine, effects_state,
    proof_contract_call,
};
use domain::filter_contexts_after_place_mutations;
pub(crate) use domain::build_domain_facts;
pub(crate) use mutation::{call_mutated_places, StateMutationSummaryCache};
use mutation::{
    call_may_mutate_contract_state, statement_mutated_place,
};

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

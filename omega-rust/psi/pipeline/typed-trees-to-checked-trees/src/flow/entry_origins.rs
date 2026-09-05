//! Exact reference and immutable scalar origins across named-state arguments.

use super::*;
use typed_trees::statement::{TransitionExit, TransitionTargetNode};

fn reference_type(
    program: &typed_trees::TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            reference_type(program, *base_type)
        }
        _ => false,
    }
}

fn immutable_scalar(
    program: &typed_trees::TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    !parameter.is_mutable
        && !parameter.is_self
        && !parameter.is_const
        && program
            .primitive_type_reference(parameter.type_reference)
            .is_some_and(|primitive| primitive.accepts_integer_literal())
}

fn tracked_parameter(
    program: &typed_trees::TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    reference_type(program, parameter.type_reference) || immutable_scalar(program, parameter)
}

fn stable_parameter(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    symbol: SymbolHandle,
) -> bool {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .is_some_and(|parameter| {
            immutable_scalar(program, parameter)
                || validation::state_reference_parameter_binding_is_stable(
                    program, machine, state, symbol,
                )
        })
}

/// Scratch dataflow rows. An invalid source denotes an unknown origin; an
/// empty set means that no entry-reachable predecessor has supplied it yet.
/// Alternative origins are unioned, never chosen by predecessor order.
#[derive(Default)]
struct ParameterOrigins {
    parameter: SymbolHandle,
    sources: Vec<SymbolHandle>,
}

struct ParameterEdge {
    source: usize,
    target: usize,
    parameters: Vec<(SymbolHandle, SymbolHandle)>,
}

fn parameter_edges(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<ParameterEdge> {
    let states = program.machine_states(machine);
    let mut edges = Vec::new();
    for (source_index, source) in states.iter().enumerate() {
        for statement in program.statement_table.statements(source.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                let (target_index, arguments) =
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named {
                            path, arguments, ..
                        } => {
                            let Some(index) = states
                                .iter()
                                .position(|state| state.symbol == path.symbol)
                                .or_else(|| (path.symbol == machine.symbol).then_some(0))
                            else {
                                continue;
                            };
                            (
                                index,
                                Some(program.statement_table.expression_handles(*arguments)),
                            )
                        }
                        TransitionTargetNode::SelfTarget => (source_index, None),
                        _ => continue,
                    };
                let state = &states[target_index];
                let mut nonself_index = 0;
                let mut mapped = Vec::new();
                for parameter in program.state_parameters(state) {
                    let source_parameter = if arguments.is_none() {
                        Some(parameter)
                    } else if parameter.is_self {
                        program
                            .state_parameters(source)
                            .iter()
                            .find(|candidate| candidate.is_self)
                    } else {
                        let argument = arguments
                            .and_then(|arguments| arguments.get(nonself_index))
                            .copied();
                        nonself_index += 1;
                        argument.and_then(|argument| {
                            let ExpressionNode::Name(name) =
                                program.expression_table.expression(argument)
                            else {
                                return None;
                            };
                            if !name.head_symbol.is_valid()
                                || name.symbol != name.head_symbol
                                || program
                                    .expression_table
                                    .name_path_members(name.members)
                                    .len()
                                    != 1
                            {
                                return None;
                            }
                            program.state_parameters(source).iter().find(|candidate| {
                                candidate.symbol == name.head_symbol
                                    && ((reference_type(program, candidate.type_reference)
                                        && reference_type(program, parameter.type_reference))
                                        || (immutable_scalar(program, candidate)
                                            && immutable_scalar(program, parameter)
                                            && program.primitive_type_reference(
                                                candidate.type_reference,
                                            ) == program.primitive_type_reference(
                                                parameter.type_reference,
                                            )))
                            })
                        })
                    };
                    if !tracked_parameter(program, parameter) {
                        continue;
                    }
                    let source_symbol = source_parameter
                        .filter(|source_parameter| {
                            stable_parameter(program, machine, source, source_parameter.symbol)
                        })
                        .map(|parameter| parameter.symbol)
                        .unwrap_or_default();
                    mapped.push((parameter.symbol, source_symbol));
                }
                edges.push(ParameterEdge {
                    source: source_index,
                    target: target_index,
                    parameters: mapped,
                });
            }
        }
    }
    edges
}

fn state_origins(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Vec<(SymbolHandle, SymbolHandle)> {
    let states = program.machine_states(machine);
    let Some(state_index) = states
        .iter()
        .position(|candidate| candidate.symbol == state.symbol)
    else {
        return Vec::new();
    };
    let edges = parameter_edges(program, machine);
    let mut origins = states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            program
                .state_parameters(state)
                .iter()
                .filter(|parameter| tracked_parameter(program, parameter))
                .map(|parameter| ParameterOrigins {
                    parameter: parameter.symbol,
                    sources: if index == 0 {
                        vec![parameter.symbol]
                    } else {
                        Vec::new()
                    },
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut reachable = vec![false; states.len()];
    reachable[0] = true;
    // The domain is finite: each row only gains entry parameter identities or
    // the unknown marker. Identity-preserving cycles converge without losing
    // their seed, while swaps and unknown incoming values remain ambiguous.
    loop {
        let mut changed = false;
        for edge in &edges {
            if !reachable[edge.source] {
                continue;
            }
            changed |= !reachable[edge.target];
            reachable[edge.target] = true;
            for (target, source) in &edge.parameters {
                let sources = origins[edge.source]
                    .iter()
                    .find(|row| row.parameter == *source)
                    .map(|row| row.sources.clone())
                    .unwrap_or_else(|| vec![SymbolHandle::invalid()]);
                let Some(row) = origins[edge.target]
                    .iter_mut()
                    .find(|row| row.parameter == *target)
                else {
                    continue;
                };
                for source in sources {
                    if !row.sources.contains(&source) {
                        row.sources.push(source);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    origins[state_index]
        .iter()
        .filter_map(|row| {
            let [source] = row.sources.as_slice() else {
                return None;
            };
            (source.is_valid() && stable_parameter(program, machine, state, row.parameter))
                .then_some((*source, row.parameter))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rebase_contexts(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    contexts: HandleSpan<FlowSemanticContextRef>,
    assumptions: bool,
) -> (
    HandleSpan<FlowSemanticContextRef>,
    HandleSpan<checked_trees::FlowExitParameterOrigin>,
) {
    let Some(entry) = program.machine_states(machine).first() else {
        return (contexts, HandleSpan::empty());
    };
    if entry.symbol == state.symbol && assumptions {
        return (contexts, HandleSpan::empty());
    }
    let mut origins = if entry.symbol == state.symbol {
        // Entry can also be reached by a named backedge. Immutable scalar
        // bindings retain the original invocation value only when every
        // incoming argument preserves it, just as for other named states.
        let incoming_origins = state_origins(program, machine, state);
        // Exit substitution refers to the caller's original input referent,
        // even when the source gives its local reference binding `mut`.
        // Initial assumptions are still admitted above; this is only the
        // exact-root obligation used when exporting the final guarantee.
        program
            .state_parameters(entry)
            .iter()
            .filter(|parameter| {
                !reference_type(program, parameter.type_reference)
                    || validation::state_reference_parameter_binding_is_stable(
                        program,
                        machine,
                        state,
                        parameter.symbol,
                    )
            })
            .filter_map(|parameter| {
                if immutable_scalar(program, parameter) {
                    incoming_origins
                        .iter()
                        .find(|(_, target)| *target == parameter.symbol)
                        .copied()
                } else {
                    Some((parameter.symbol, parameter.symbol))
                }
            })
            .collect()
    } else {
        state_origins(program, machine, state)
    };
    // `self` is the machine attachment context, explicitly admitted by each
    // state's self parameter rather than an ordinary named jump argument.
    if let Some(entry_self) = program
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.is_self)
        && let Some(state_self) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
    {
        origins.retain(|(source, _)| *source != entry_self.symbol);
        origins.push((entry_self.symbol, state_self.symbol));
    }
    let mut rebased = HandleSpan::empty();
    let mut parameter_origins = HandleSpan::empty();
    let sources = ctx
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
        .to_vec();
    for source in sources {
        let context = semantic.contexts.get(source.context).clone();
        let source_facts = semantic
            .refs
            .span_or_empty(context.facts)
            .iter()
            .map(|reference| (reference.fact, *semantic.facts.get(reference.fact)))
            .collect::<Vec<_>>();
        if !source_facts.iter().any(|(_, fact)| {
            matches!(fact.point, ProgramPoint::Machine { machine_symbol } if machine_symbol == machine.symbol)
        }) {
            // Global and already state-local contexts are immutable shared
            // evidence. Republishing them at their original point makes the
            // next sibling collect every previous copy again.
            ctx.contexts
                .semantic_context_refs
                .append_to_span(&mut rebased, source);
            continue;
        }
        let scoped_point = if assumptions {
            ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            }
        } else {
            // Requirements remain at their exact Exit point. They must never
            // become State entry assumptions or Machine-wide evidence.
            context.point
        };
        let mut refs = HandleSpan::empty();
        let mut complete = true;
        for (source_fact, mut fact) in source_facts {
            if !matches!(fact.point, ProgramPoint::Machine { machine_symbol } if machine_symbol == machine.symbol)
            {
                semantic.append_ref(&mut refs, source_fact);
                continue;
            }
            fact.point = scoped_point;
            let contract = match fact.payload {
                FactPayload::ContractBooleanExpression { fact, .. }
                | FactPayload::ContractDomainMembership { fact, .. }
                | FactPayload::ContractPropositionApplication { fact, .. }
                | FactPayload::ContractCarryPermission { fact, .. } => Some(fact),
                _ => None,
            };
            let mut required_roots = Vec::new();
            if let Some(contract) = contract {
                for occurrence in
                    crate::contract_occurrences::fact_referenced_occurrences(program, contract)
                {
                    if let Some(place) = canonical_place_from_expression_in_state(
                        program,
                        entry.symbol,
                        0,
                        occurrence,
                    ) && let facts::PlaceRoot::Symbol(root) = place.root
                    {
                        required_roots.push(root);
                    }
                }
            }
            if let FactPlace::Place(place) = fact.place {
                let mut place = *semantic.places.get(place);
                if let facts::PlaceRoot::Symbol(root) = place.root {
                    required_roots.push(root);
                    if let Some((_, target)) = origins.iter().find(|(entry, _)| *entry == root) {
                        place.root = facts::PlaceRoot::Symbol(*target);
                        fact.place = FactPlace::Place(semantic.places.append(place));
                    }
                }
            }
            required_roots.sort_by_key(|root| (root.arena_index(), root.generation()));
            required_roots.dedup();
            for root in required_roots {
                if !program
                    .state_parameters(entry)
                    .iter()
                    .any(|parameter| parameter.symbol == root)
                {
                    continue;
                }
                let target = origins
                    .iter()
                    .find_map(|(entry, target)| (*entry == root).then_some(*target))
                    .unwrap_or_default();
                // Entry assumptions do not establish arrival invariants after
                // arbitrary predecessor writes. Only explicit state requires
                // are assumed here until graph fact transfer is available.
                let declared_field = matches!(fact.origin, FactOrigin::MachineFieldDomain { .. });
                complete &= target.is_valid() && (!assumptions || declared_field);
                if !assumptions && let Some(contract) = contract {
                    ctx.control.exit_parameter_origins.append_to_span(
                        &mut parameter_origins,
                        checked_trees::FlowExitParameterOrigin {
                            contract,
                            entry_parameter: root,
                            state_parameter: target,
                        },
                    );
                }
            }
            let fact = semantic.append_fact(fact);
            semantic.append_ref(&mut refs, fact);
        }
        if complete || !assumptions {
            let context = semantic.append_context(scoped_point, refs);
            ctx.contexts
                .semantic_context_refs
                .append_to_span(&mut rebased, FlowSemanticContextRef { context });
        }
    }
    (rebased, parameter_origins)
}

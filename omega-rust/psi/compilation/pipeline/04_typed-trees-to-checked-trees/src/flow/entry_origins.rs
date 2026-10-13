//! Exact reference and immutable scalar origins across named-state arguments.
use crate::flow::FlowBuildContext;
use crate::flow::reference_spans;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;

use facts::{FactPayload, FactPlace, FactPlan, ProgramPoint};
use symbols::SymbolHandle;
use validation::{CallFrameResolver, LocalWriteSource};

use typed_trees::statement::TransitionExit;

fn reference_type(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    (!parameter.is_mutable)
        && !parameter.is_const
        && program
            .primitive_type_reference(parameter.type_reference)
            .is_some_and(|primitive| primitive.accepts_integer_literal())
}

fn tracked_parameter(
    program: &dyn ::validation::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    reference_type(program, parameter.type_reference) || immutable_scalar(program, parameter)
}

fn stable_parameter(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    symbol: SymbolHandle,
) -> bool {
    program
        .state_parameter_by_symbol(state, symbol)
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
    storage: Vec<Option<LocalWriteSource>>,
}

struct ParameterEdge {
    source: usize,
    target: usize,
    parameters: Vec<(SymbolHandle, SymbolHandle)>,
    storage: Vec<(SymbolHandle, Option<Vec<LocalWriteSource>>)>,
}

/// Resolve a Named transition argument to the source parameter carrying its
/// exact origin. Immutable `let` locals keep custody of the parameter they
/// were bound from, so a `let held = input; ... -> next(held)` edge maps
/// through the local to `input`; a mutable local is opaque (it may be reseated
/// before the transition). Locals shadow same-named parameters, and each
/// local's initializer only sees bindings that precede it.
fn tracked_source_parameter<'a>(
    program: &'a dyn ::validation::CheckingInput,
    source: &'a typed_trees::state::State,
    locals: &[(
        SymbolHandle,
        Option<typed_trees::expression::ExpressionHandle>,
    )],
    argument: typed_trees::expression::ExpressionHandle,
    target: &typed_trees::signature::StateParameter,
) -> Option<&'a typed_trees::signature::StateParameter> {
    let mut expression = argument;
    let mut visible = locals.len();
    loop {
        let name = program.expression_table.name_path_node(expression)?;
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
        match locals[..visible]
            .iter()
            .rposition(|(symbol, _)| *symbol == name.head_symbol)
        {
            Some(index) => {
                visible = index;
                expression = locals[index].1?;
            }
            None => {
                return program.state_parameters(source).iter().find(|candidate| {
                    candidate.symbol == name.head_symbol
                        && ((reference_type(program, candidate.type_reference)
                            && reference_type(program, target.type_reference))
                            || (immutable_scalar(program, candidate)
                                && immutable_scalar(program, target)
                                && program.primitive_type_reference(candidate.type_reference)
                                    == program.primitive_type_reference(target.type_reference)))
                });
            }
        }
    }
}

fn parameter_edges<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    frames: Option<&CallFrameResolver<'program>>,
) -> Vec<ParameterEdge> {
    let states = program.machine_states(machine);
    let mut edges = Vec::new();
    for (source_index, source) in states.iter().enumerate() {
        let mut locals: Vec<(
            SymbolHandle,
            Option<typed_trees::expression::ExpressionHandle>,
        )> = Vec::new();
        for statement in program.statement_table.statements(source.statement_nodes) {
            if let Some(local) = statement.as_local_data() {
                locals.push((
                    local.symbol,
                    (!local.is_mutable).then_some(local.initial_value),
                ));
                continue;
            }
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            for target in transition.targets() {
                let Some((path, arguments)) =
                    program.statement_table.named_transition_target(target)
                else {
                    continue;
                };
                // A machine-entry target is a recursive invocation whose
                // activation owns fresh formals, not an arrival carrying this
                // activation's values: only subordinate states join the edges.
                let Some(target_index) =
                    validation::internal_state_index(program, machine, path.symbol)
                else {
                    continue;
                };
                let arguments = program.statement_table.expression_handles(arguments);
                let state = &states[target_index];
                let captured =
                    frames.and_then(|frames| frames.transition_write_origins(machine, target));
                let mut storage = Vec::new();
                let mut nonself_index = 0;
                let mut mapped = Vec::new();
                for parameter in program.state_parameters(state) {
                    if tracked_parameter(program, parameter) {
                        let sources = {
                            captured
                                .as_ref()
                                .and_then(|captured| captured.arguments.get(nonself_index))
                                .cloned()
                                .flatten()
                        };
                        storage.push((parameter.symbol, sources));
                    }
                    let source_parameter = {
                        let argument = arguments.get(nonself_index).copied();
                        nonself_index += 1;
                        argument.and_then(|argument| {
                            tracked_source_parameter(program, source, &locals, argument, parameter)
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
                    storage,
                });
            }
        }
    }
    edges
}

/// Every state's parameter origins in one machine: which entry parameters
/// each state parameter can carry, solved to a fixed point over the
/// machine's named parameter edges. The solution does not depend on which
/// state asks, so one solve serves every state of the machine.
pub(crate) struct MachineParameterOrigins {
    rows: Vec<Vec<ParameterOrigins>>,
    storage_available: bool,
}

impl MachineParameterOrigins {
    /// Whether this solve covers every state `machine` currently stores; a
    /// machine that gained states since must be solved again.
    pub(super) fn covers(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> bool {
        self.rows.len() == program.machine_states(machine).len()
    }
}

pub(crate) fn machine_parameter_origins(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> MachineParameterOrigins {
    solve_parameter_origins(program, machine, None)
}

/// Prepare once for all writes/calls in this machine. Values and addresses
/// share the same named edges and fixed point, but retain distinct transfer
/// semantics: changing referent contents never changes address provenance.
pub(crate) fn machine_storage_origins<'program>(
    program: &'program dyn ::validation::CheckingInput,
    frames: &CallFrameResolver<'program>,
    machine: &'program typed_trees::machine::Machine,
) -> MachineParameterOrigins {
    solve_parameter_origins(program, machine, Some(frames))
}

fn solve_parameter_origins<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    frames: Option<&CallFrameResolver<'program>>,
) -> MachineParameterOrigins {
    let states = program.machine_states(machine);
    if states.is_empty() {
        return MachineParameterOrigins {
            rows: Vec::new(),
            storage_available: frames.is_some(),
        };
    }
    let edges = parameter_edges(program, machine, frames);
    // Reuse the storage substitution owner's SCC law. Recursive selector
    // growth is unknown, rather than truncated by an arbitrary iteration cap.
    let bindings = edges
        .iter()
        .flat_map(|edge| &edge.storage)
        .flat_map(|(target, sources)| {
            sources
                .iter()
                .flatten()
                .filter(|source| source.root.is_valid())
                .map(move |source| (*target, source.clone()))
        })
        .collect::<Vec<_>>();
    let storage_available =
        frames.is_some_and(|frames| frames.storage_origin_substitutions_are_finite(&bindings));
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
                    storage: if index == 0 && storage_available {
                        vec![Some(root_storage(program, parameter.symbol))]
                    } else {
                        Vec::new()
                    },
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
            if storage_available {
                for (target, sources) in &edge.storage {
                    let mut incoming = Vec::new();
                    match sources {
                        None => incoming.push(None),
                        Some(sources) => {
                            for source in sources {
                                incoming.extend(transport_storage(
                                    program,
                                    machine,
                                    &origins[edge.source],
                                    source,
                                ));
                            }
                        }
                    }
                    if let Some(row) = origins[edge.target]
                        .iter_mut()
                        .find(|row| row.parameter == *target)
                    {
                        for source in incoming {
                            if !row.storage.contains(&source) {
                                row.storage.push(source);
                                changed = true;
                            }
                        }
                    }
                }
            }
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
    MachineParameterOrigins {
        rows: origins,
        storage_available,
    }
}

fn root_storage(program: &dyn ::validation::CheckingInput, root: SymbolHandle) -> LocalWriteSource {
    LocalWriteSource {
        recasts: Vec::new(),
        root,
        segments: Vec::new(),
        representation: None,
        path: program.symbols.name(root).to_owned(),
        collection_coarse: false,
    }
}

fn transport_storage(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    rows: &[ParameterOrigins],
    source: &LocalWriteSource,
) -> Vec<Option<LocalWriteSource>> {
    if !source.root.is_valid() {
        return vec![None];
    }
    if let Some(row) = rows.iter().find(|row| row.parameter == source.root) {
        return row
            .storage
            .iter()
            .map(|parent| {
                parent
                    .as_ref()
                    .and_then(|parent| parent.substitute_storage(source))
            })
            .collect();
    }
    // A proven private allocation retains its declaration identity. Reference
    // locals must already have been resolved by the caller-prefix owner.
    for state in program.machine_states(machine) {
        if let Some((_, local)) = program.state_local_declaration(state, source.root) {
            return if !reference_type(program, local.type_reference) {
                vec![Some(source.clone())]
            } else {
                vec![None]
            };
        }
        if program.state_parameters(state).iter().any(|parameter| {
            parameter.symbol == source.root && !reference_type(program, parameter.type_reference)
        }) {
            return vec![Some(source.clone())];
        }
    }
    vec![None]
}

/// Rebase any captured state-local address to invocation storage. Unknown is
/// distinct from an unreachable empty set. Private owned allocations retain
/// their exact roots; no caller origin is invented for an unresolved alias.
pub(crate) fn entry_storage_origins(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    solved: &MachineParameterOrigins,
    source: &LocalWriteSource,
) -> Option<Vec<LocalWriteSource>> {
    if !solved.storage_available {
        return None;
    }
    let index = typed_trees::state::state_position(program.machine_states(machine), state.symbol)?;
    transport_storage(program, machine, solved.rows.get(index)?, source)
        .into_iter()
        .collect()
}

/// The entry-parameter identities `state`'s stable parameters carry on every
/// incoming path, read from its machine's solved origins.
pub(crate) fn state_origins(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    solved: &MachineParameterOrigins,
) -> Vec<(SymbolHandle, SymbolHandle)> {
    let Some(state_index) =
        typed_trees::state::state_position(program.machine_states(machine), state.symbol)
    else {
        return Vec::new();
    };
    solved.rows[state_index]
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
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
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
    let origins = if entry.symbol == state.symbol {
        // Entry can also be reached by a named backedge. Immutable scalar
        // bindings retain the original invocation value only when every
        // incoming argument preserves it, just as for other named states.
        let incoming_origins = build.entry_origin_chain_at(program, machine, state);
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
        build
            .entry_origin_chain_at(program, machine, state)
            .as_ref()
            .clone()
    };
    let mut rebased = HandleSpan::empty();
    let mut parameter_origins = HandleSpan::empty();
    let sources = build
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
            reference_spans::append_flow_reference(
                &mut build.contexts.semantic_context_refs,
                &mut rebased,
                source,
            );
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
                let contract_occurrences = build.proof_fact_occurrences_at(program, contract);
                for occurrence in contract_occurrences.iter().copied() {
                    if let Some(place) =
                        build.canonical_place_at(program, entry.symbol, 0, occurrence)
                        && let Some(root) = place.root.as_symbol()
                    {
                        required_roots.push(root);
                    }
                }
            }
            if let Some(place) = fact.place.as_place() {
                let mut place = *semantic.places.get(place);
                if let Some(root) = place.root.as_symbol() {
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
                if !program.state_parameter_by_symbol(entry, root).is_some() {
                    continue;
                }
                let target = origins
                    .iter()
                    .find_map(|(entry, target)| (*entry == root).then_some(*target))
                    .unwrap_or_default();
                // Entry assumptions do not establish arrival invariants after
                // arbitrary predecessor writes. Only explicit state requires
                // are assumed here; state_values transports independently
                // captured live qualifications through actual arrivals.
                complete &= target.is_valid() && !assumptions;
                if !assumptions && let Some(contract) = contract {
                    build.control.exit_parameter_origins.append_to_span(
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
            reference_spans::append_flow_reference(
                &mut build.contexts.semantic_context_refs,
                &mut rebased,
                FlowSemanticContextRef { context },
            );
        }
    }
    (rebased, parameter_origins)
}

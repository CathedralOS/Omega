//! Normal-call transfer of reference slots. A type supplies the slot inventory;
//! the complete body supplies its possible exit contents. Identity rows mean
//! incoming contents survive that route, not that the carrier is its backing.
//! Unknown new contents, local backing escapes and recursive summaries fail
//! closed. These rows carry provenance, never access or predicate authority.

pub(super) mod retention;

use super::caller_aliases::{LocalWriteOrigin, LocalWriteSource};
use super::place_paths::{FramePathPrecision, FramePlaceOrigin, FrameSourcePlace};
use super::state_write_walk::{CollectedStatementPrefix, stored_reference_transfer};
use super::stored_origins::{self, StoredLocalOrigins, StoredWriteOrigin};
use super::{FrameInference, Machine, SymbolHandle, TableCall, TopLevelSymbols, TypedTrees};
use facts::PlaceSegment;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

/// One exact destination leaf after an ordinary call. All sources must remain
/// live while this leaf is used. Incoming means the call may leave its prior
/// contents intact; an empty new-source set is not permission to drop them.
#[derive(Clone)]
pub struct StoredReferenceEffect {
    pub destination: LocalWriteOrigin,
    pub sources: Vec<LocalWriteSource>,
    pub retains_incoming: bool,
}

#[derive(Clone)]
pub(super) struct SlotEffect {
    root: SymbolHandle,
    segments: Vec<PlaceSegment>,
    sources: Vec<FramePlaceOrigin>,
    incoming: bool,
}

pub(super) fn public_effects(effects: Vec<SlotEffect>) -> Vec<StoredReferenceEffect> {
    effects
        .into_iter()
        .map(|effect| StoredReferenceEffect {
            destination: LocalWriteOrigin {
                local_symbol: effect.root,
                local_segments: effect.segments,
                source: None,
            },
            sources: effect
                .sources
                .into_iter()
                .filter(|source| !source.source.is_static())
                .map(LocalWriteSource::from)
                .collect(),
            retains_incoming: effect.incoming,
        })
        .collect()
}

pub(super) fn call_effects(
    program: &TypedTrees,
    machine: &Machine,
    call: &TableCall,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let (callee, state) = super::machine_state_by_symbol(program, call.target_symbol)?;
    let parameters = program.state_parameters(state);
    let arguments = program.statement_table.expression_handles(call.arguments);
    if (parameters.len() != arguments.len())
        || !call.machine_arguments.is_empty()
        || !program.machine_type_parameters(callee).is_empty()
        || !callee.body_is_present
        || callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
    {
        return None;
    }
    if destination_inventory(program, state)?.is_empty() {
        return Some(Vec::new());
    }
    // The relation below captures actuals from the incoming inventory. Pure
    // argument calls preserve it; a write during argument evaluation needs a
    // post-evaluation inventory, which this query must not fabricate from the
    // pre-call snapshot. Use ordinary frame inference, not a syntactic call ban.
    if arguments
        .iter()
        .any(|argument| super::caller_aliases::expression_has_calls(program, *argument))
    {
        let (caller_state, _, _) = super::caller_aliases::caller_statement_at_site(
            program,
            machine,
            symbols,
            super::caller_aliases::CallerWriteSite::Call(call),
        )?;
        let mut diagnostics = Vec::new();
        let machine_symbols =
            crate::declarations::symbols::MachineSymbols::build(program, machine, &mut diagnostics);
        if !diagnostics.is_empty() {
            return None;
        }
        let origins = super::caller_aliases::CallOriginContext {
            parameters: program.state_parameters(caller_state),
            isolated_locals: &context.isolated_locals,
            aliases: &context.aliases,
            divergent: &context.divergent,
            stored: &context.stored,
        };
        let mut writes = Vec::new();
        for argument in arguments {
            super::demand::collect_expression_call_written_paths(
                program,
                *argument,
                machine,
                &machine_symbols,
                symbols,
                inference,
                &mut writes,
                &origins,
            )?;
        }
        if !writes.is_empty() {
            return None;
        }
    }
    let relative = state_effects(program, callee, state, symbols, inference)?;
    instantiate(
        program, machine, parameters, arguments, &relative, context, symbols, inference,
    )
}

fn destination_inventory(program: &TypedTrees, state: &State) -> Option<Vec<StoredWriteOrigin>> {
    let mut leaves = Vec::new();
    for parameter in program.state_parameters(state) {
        if !super::type_reference_is_reference(program, parameter.type_reference)
            || !super::type_may_carry_write(program, parameter.type_reference)
        {
            continue;
        }
        let reference =
            super::reference_subjects::carrier_storage_type(program, parameter.type_reference)?;
        let inventory = stored_origins::declared_origins_for_query(
            program,
            parameter.symbol,
            parameter.name.as_str(),
            reference,
            true,
        )?;
        // This query currently transports shared slots. An exclusive leaf is
        // not silently dropped from an otherwise complete output inventory.
        for leaf in &inventory.references {
            let leaf_type =
                stored_origins::projected_storage_type(program, reference, &leaf.local_segments)?;
            if !super::type_reference_is_reference(program, leaf_type)
                || super::type_may_carry_write(program, leaf_type)
            {
                return None;
            }
        }
        leaves.extend(inventory.references);
    }
    Some(leaves)
}

pub(super) fn state_effects(
    program: &TypedTrees,
    machine: &Machine,
    entry: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    if inference.active_states.contains(&entry.symbol) {
        return None;
    }
    inference.active_states.push(entry.symbol);
    let result = solve_state_effects(program, machine, entry, symbols, inference);
    inference.active_states.pop();
    result
}

enum SlotExit {
    Return(Vec<StoredLocalOrigins>),
    Edge {
        target: usize,
        arguments: Vec<ExpressionHandle>,
        prefix: CollectedStatementPrefix,
    },
}

struct SlotEquation<'program> {
    state: &'program State,
    inventory: Vec<StoredWriteOrigin>,
    exits: Vec<SlotExit>,
}

/// Least fixed point over normal exits. An unvisited return is not an identity
/// route: only a real normal exit can seed incoming contents. Root substitution
/// finiteness bounds the source set before iteration; there is no depth cutoff.
fn solve_state_effects<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    entry: &'program State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let mut states = vec![entry];
    let mut equations = Vec::new();
    let mut substitutions = super::cycle_substitutions::Substitutions::default();
    let mut index = 0;
    while index < states.len() {
        let state = states[index];
        let inventory = destination_inventory(program, state)?;
        let (last, prefixes) =
            stored_reference_transfer(program, machine, state, symbols, inference)?;
        let mut exits = Vec::new();
        let mut has_transition = false;
        for (ordinal, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            has_transition = true;
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            let prefix = prefixes.get(ordinal)?.as_ref()?;
            for edge in [transition.target, transition.continuation] {
                if !edge.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(edge) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        if super::terminal_calls::target(program, machine, edge).is_valid() {
                            return None;
                        }
                        let target = super::transition_topology::named_transition_target_state(
                            program, machine, state, edge,
                        )?;
                        let target_index = if let Some(index) = states
                            .iter()
                            .position(|candidate| candidate.symbol == target.symbol)
                        {
                            index
                        } else {
                            states.push(target);
                            states.len() - 1
                        };
                        let arguments = program.statement_table.expression_handles(*arguments);
                        let parameters = program.state_parameters(target);
                        if parameters.len() != arguments.len() {
                            return None;
                        }
                        for (parameter, argument) in parameters.iter().zip(arguments) {
                            let carries_origin = super::type_reference_is_reference(
                                program,
                                parameter.type_reference,
                            ) || stored_origins::declared_origins_for_query(
                                program,
                                parameter.symbol,
                                parameter.name.as_str(),
                                parameter.type_reference,
                                true,
                            )
                            .is_some_and(|inventory| !inventory.references.is_empty());
                            if !carries_origin {
                                continue;
                            }
                            // Finiteness follows captured backing roots, not
                            // exact descriptor identity. A dynamic window may
                            // cross this edge with conservative geometry; its
                            // destination slot is still matched exactly by
                            // instantiate. Static storage adds no dependency.
                            let origin = argument_origin(
                                program, machine, parameter, *argument, prefix, symbols, inference,
                            )?;
                            let source = &origin.source;
                            if !source.builtin_coordinates {
                                return None;
                            }
                            if source.is_static() {
                                continue;
                            }
                            if !source.root.is_valid() {
                                return None;
                            }
                            let from = substitutions.parameter(target.symbol, parameter.symbol);
                            let to = substitutions.parameter(state.symbol, source.root);
                            substitutions.insert(
                                from,
                                to,
                                !source.segments.is_empty() || !source.recasts.is_empty(),
                            );
                        }
                        exits.push(SlotExit::Edge {
                            target: target_index,
                            arguments: arguments.to_vec(),
                            prefix: prefix.clone(),
                        });
                    }
                    TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => {
                        exits.push(SlotExit::Return(prefix.stored.clone()))
                    }
                }
            }
        }
        if !has_transition {
            exits.push(SlotExit::Return(last.stored));
        }
        equations.push(SlotEquation {
            state,
            inventory,
            exits,
        });
        index += 1;
    }
    if !substitutions.is_finite() {
        return None;
    }
    let mut known = vec![None::<Vec<SlotEffect>>; equations.len()];
    loop {
        let mut changed = false;
        for (index, equation) in equations.iter().enumerate() {
            let mut candidate = Vec::new();
            let mut returns = false;
            for exit in &equation.exits {
                let stored = match exit {
                    SlotExit::Return(stored) => stored.clone(),
                    SlotExit::Edge {
                        target,
                        arguments,
                        prefix,
                    } => {
                        let Some(relative) = &known[*target] else {
                            continue;
                        };
                        let effects = instantiate(
                            program,
                            machine,
                            program.state_parameters(equations[*target].state),
                            arguments,
                            relative,
                            prefix,
                            symbols,
                            inference,
                        )?;
                        let mut stored = prefix.stored.clone();
                        apply(program, &mut stored, &effects);
                        stored
                    }
                };
                returns = true;
                collect_exit(
                    program,
                    equation.state,
                    &equation.inventory,
                    &stored,
                    &mut candidate,
                )?;
            }
            if !returns {
                continue;
            }
            if known[index].is_none() {
                known[index] = Some(Vec::new());
                changed = true;
            }
            let accumulated = known[index].as_mut()?;
            for effect in candidate {
                if let Some(prior) = accumulated
                    .iter_mut()
                    .find(|prior| prior.root == effect.root && prior.segments == effect.segments)
                {
                    if effect.incoming && !prior.incoming {
                        prior.incoming = true;
                        changed = true;
                    }
                    for source in effect.sources {
                        if !prior.sources.contains(&source) {
                            prior.sources.push(source);
                            changed = true;
                        }
                    }
                } else {
                    accumulated.push(effect);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    known.into_iter().next()?
}

fn collect_exit(
    program: &TypedTrees,
    state: &State,
    inventory: &[StoredWriteOrigin],
    stored: &[StoredLocalOrigins],
    effects: &mut Vec<SlotEffect>,
) -> Option<()> {
    for slot in inventory {
        // Runtime selector identity cannot escape its expression namespace.
        if slot
            .local_segments
            .iter()
            .any(|part| matches!(part, PlaceSegment::Index { .. }))
        {
            return None;
        }
        let record = stored
            .iter()
            .find(|record| record.local_symbol == slot.local_symbol)?;
        let mut selected = false;
        for leaf in record
            .references
            .iter()
            .filter(|leaf| leaf.local_segments == slot.local_segments)
        {
            selected = true;
            let source = leaf.origin.as_ref()?;
            if !source.source.builtin_coordinates {
                return None;
            }
            if !source.source.is_static() {
                if !source.source.root.is_valid() {
                    return None;
                }
                let parameter = program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == source.source.root)?;
                // Owned input storage dies in the callee. Only a reference
                // boundary actually crossed by this source survives invocation.
                if !super::type_reference_is_reference(program, parameter.type_reference) {
                    let frontier = stored_origins::declared_origins_for_query(
                        program,
                        parameter.symbol,
                        parameter.name.as_str(),
                        parameter.type_reference,
                        true,
                    )?;
                    if !frontier.references.iter().any(|leaf| {
                        stored_origins::source_reaches_leaf(
                            &source.source.segments,
                            &leaf.local_segments,
                        )
                    }) {
                        return None;
                    }
                }
            }
            let incoming = source.source.root == slot.local_symbol
                && source.source.segments == slot.local_segments;
            let effect = if let Some(index) = effects.iter().position(|effect| {
                effect.root == slot.local_symbol && effect.segments == slot.local_segments
            }) {
                &mut effects[index]
            } else {
                effects.push(SlotEffect {
                    root: slot.local_symbol,
                    segments: slot.local_segments.clone(),
                    sources: Vec::new(),
                    incoming: false,
                });
                effects.last_mut()?
            };
            effect.incoming |= incoming;
            if !incoming && !effect.sources.contains(source) {
                effect.sources.push(source.clone());
            }
        }
        if !selected {
            return None;
        }
    }
    Some(())
}

fn instantiate(
    program: &TypedTrees,
    machine: &Machine,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    relative: &[SlotEffect],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    if parameters.len() != arguments.len() || !context.divergent.is_empty() {
        return None;
    }
    let substitute = |root, segments: &[PlaceSegment]| {
        let index = parameters
            .iter()
            .position(|parameter| parameter.symbol == root)?;
        let actual = *arguments.get(index)?;
        if super::caller_aliases::expression_has_calls(program, actual) {
            return None;
        }
        let base = stored_origins::binding_source(program, actual, &context.aliases)?;
        if !base.root.is_valid() || !base.builtin_coordinates {
            return None;
        }
        Some(base.append_relative(&FrameSourcePlace {
            recasts: Vec::new(),
            representation: None,
            kind: Default::default(),
            root,
            segments: segments.to_vec(),
            builtin_coordinates: true,
        }))
    };
    let mut output = Vec::new();
    for effect in relative {
        let destination = substitute(effect.root, &effect.segments)?;
        let mut sources = Vec::new();
        for source in &effect.sources {
            let candidates = if source.source.is_static() {
                vec![Some(source.clone())]
            } else {
                let index = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == source.source.root)?;
                let actual = *arguments.get(index)?;
                let captured = argument_origin(
                    program,
                    machine,
                    &parameters[index],
                    actual,
                    context,
                    symbols,
                    inference,
                )?;
                let mut projected = captured.clone();
                projected.source = captured.source.append_relative(&source.source);
                if source.precision == FramePathPrecision::CollectionCoarse {
                    projected.precision = FramePathPrecision::CollectionCoarse;
                }
                projected.path = if projected.source.is_static() {
                    String::new()
                } else {
                    path(program, &projected.source)
                };
                stored_origins::captured_reference_projection_origins(
                    program,
                    &captured,
                    &projected,
                    &context.stored,
                )
            };
            for source in candidates {
                let source = source?;
                if (!source.source.root.is_valid() && !source.source.is_static())
                    || !source.source.builtin_coordinates
                {
                    return None;
                }
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }
        output.push(SlotEffect {
            root: destination.root,
            segments: destination.segments,
            sources,
            incoming: effect.incoming,
        });
    }
    Some(output)
}

fn argument_origin(
    program: &TypedTrees,
    machine: &Machine,
    parameter: &StateParameter,
    actual: ExpressionHandle,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<FramePlaceOrigin> {
    if super::caller_aliases::expression_has_calls(program, actual) {
        return None;
    }
    if super::type_reference_is_reference(program, parameter.type_reference) {
        return super::reference_subjects::backing_origin(
            program,
            machine,
            actual,
            symbols,
            inference,
            &context.aliases,
            &context.stored,
        );
    }
    let source = stored_origins::binding_source(program, actual, &context.aliases)?;
    if !source.root.is_valid() || !source.builtin_coordinates {
        return None;
    }
    Some(FramePlaceOrigin {
        path: path(program, &source),
        precision: FramePathPrecision::Exact,
        source,
    })
}

fn path(program: &TypedTrees, source: &FrameSourcePlace) -> String {
    let mut path = program.symbols.name(source.root).to_owned();
    for segment in &source.segments {
        if let PlaceSegment::Field { symbol } = segment {
            path.push('.');
            path.push_str(program.symbols.name(*symbol));
        }
    }
    path
}

pub(super) fn apply(
    program: &TypedTrees,
    stored: &mut Vec<StoredLocalOrigins>,
    effects: &[SlotEffect],
) {
    // Every source above was captured from the same pre-call inventory.
    for effect in effects {
        let index = stored
            .iter()
            .position(|record| record.local_symbol == effect.root)
            .unwrap_or_else(|| {
                stored.push(StoredLocalOrigins {
                    local_symbol: effect.root,
                    references: Vec::new(),
                    cases: Vec::new(),
                    moves: Vec::new(),
                    symbolic: false,
                });
                stored.len() - 1
            });
        let record = &mut stored[index];
        if !effect.incoming {
            record
                .references
                .retain(|leaf| leaf.local_segments != effect.segments);
        }
        for source in &effect.sources {
            if record.references.iter().any(|leaf| {
                leaf.local_segments == effect.segments && leaf.origin.as_ref() == Some(source)
            }) {
                continue;
            }
            record.references.push(StoredWriteOrigin {
                local_symbol: effect.root,
                local_path: path(
                    program,
                    &FrameSourcePlace {
                        recasts: Vec::new(),
                        representation: None,
                        kind: Default::default(),
                        root: effect.root,
                        segments: effect.segments.clone(),
                        builtin_coordinates: true,
                    },
                ),
                local_segments: effect.segments.clone(),
                origin: Some(source.clone()),
            });
        }
        record.symbolic = false;
    }
}

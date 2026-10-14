//! Normal-call transfer of reference slots. A type supplies the slot inventory;
//! the complete body supplies its possible exit contents. Identity rows mean
//! incoming contents survive that route, not that the carrier is its backing.
//! Unknown new contents, local backing escapes and recursive summaries fail
//! closed. Rows retain each slot's declared access alongside provenance;
//! checked stores still own permission and predicate authority.
//!
//! Slice inventories retain incoming defaults plus exact slot replacements.
//! Normal-exit summaries refine those defaults across all reachable exits before
//! joining their origins. A branch which never writes therefore retains its
//! incoming alternative; an untouched sibling keeps its original descriptor.
//! Type-default selectors are not executable indices, and a runtime selector
//! cannot cross a call boundary without a captured substitution relation.

mod expression_effects;
pub(super) mod retention;
pub(super) use expression_effects::statement_effects;

use super::caller_aliases::{LocalWriteOrigin, LocalWriteSource};
use super::place_paths::{FramePathPrecision, FramePlaceOrigin, FrameSourcePlace};
use super::state_write_walk::{CollectedStatementPrefix, stored_reference_transfer};
use super::stored_origins::{self, StoredLocalOrigins, StoredWriteOrigin};
use super::{FrameInference, Machine, SymbolHandle, TableCall, TopLevelSymbols};
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
    pub sources: Vec<StoredReferenceSource>,
    pub retains_incoming: bool,
    pub access: language_semantics::ReferenceAccess,
}

/// The actual descriptor home and its backing are distinct coordinates.
/// Both are required: backing alone cannot authorize an exclusive transfer.
#[derive(Clone)]
pub struct StoredReferenceSource {
    pub carrier_symbol: SymbolHandle,
    pub carrier_segments: Vec<PlaceSegment>,
    pub backing: LocalWriteSource,
}

#[derive(Clone)]
pub(super) struct SlotEffect {
    root: SymbolHandle,
    segments: Vec<PlaceSegment>,
    sources: Vec<SlotSource>,
    incoming: bool,
    access: language_semantics::ReferenceAccess,
}

#[derive(Clone, PartialEq, Eq)]
struct SlotSource {
    origin: FramePlaceOrigin,
    carrier: FrameSourcePlace,
}

#[derive(Clone)]
struct SlotInventory {
    origin: StoredWriteOrigin,
    access: language_semantics::ReferenceAccess,
    /// Each crossed pointer still denotes its incoming descriptor occurrence.
    /// A final spelling cannot recover a destination captured before rebinding.
    ancestors: Vec<Vec<PlaceSegment>>,
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
                .filter(|source| !source.origin.source.is_static())
                .map(|source| StoredReferenceSource {
                    carrier_symbol: source.carrier.root,
                    carrier_segments: source.carrier.segments,
                    backing: LocalWriteSource::from(source.origin),
                })
                .collect(),
            retains_incoming: effect.incoming,
            access: effect.access,
        })
        .collect()
}

pub(super) fn call_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    call: &TableCall,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    invocation_effects(
        program,
        machine,
        call.target_symbol,
        program.statement_table.expression_handles(call.arguments),
        call.machine_arguments.is_empty(),
        context,
        symbols,
        inference,
    )
}

fn invocation_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    machine_arguments_empty: bool,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let (callee, state) = super::machine_state_by_symbol(program, target)?;
    let parameters = program.state_parameters(state);
    if (parameters.len() != arguments.len())
        || !machine_arguments_empty
        || !program.machine_type_parameters(callee).is_empty()
        || !callee.body_is_present
        || callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
    {
        return None;
    }
    if destination_inventory(program, state)?.is_empty() {
        return Some(Vec::new());
    }
    let relative = state_effects(program, callee, state, symbols, inference)?;
    instantiate(
        program, machine, parameters, arguments, &relative, context, symbols, inference,
    )
}

fn destination_inventory(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Option<Vec<SlotInventory>> {
    let mut leaves = Vec::new();
    for parameter in program.state_parameters(state) {
        let shell = crate::reference_shell(program, parameter.type_reference);
        if shell.is_some_and(|shell| !shell.access.is_exclusive()) {
            continue;
        }
        let reference = shell.map_or(parameter.type_reference, |shell| shell.referee);
        let mut pending = vec![(
            reference,
            Vec::new(),
            shell.is_some(),
            shell.is_none_or(|shell| shell.access.is_readable()),
            Vec::new(),
            Vec::<Vec<PlaceSegment>>::new(),
        )];
        while let Some((reference, prefix, borrowed, readable, mut visited, ancestors)) =
            pending.pop()
        {
            // A recursive pointee needs demand-bounded captured destinations;
            // do not turn an incomplete reachable inventory into an empty one.
            if visited.contains(&reference) {
                return None;
            }
            visited.push(reference);
            let inventory = stored_origins::declared_origins_for_query(
                program,
                parameter.symbol,
                parameter.name.as_str(),
                reference,
                true,
            )?;
            for mut leaf in inventory.references {
                let leaf_type = stored_origins::projected_storage_type(
                    program,
                    reference,
                    &leaf.local_segments,
                )?;
                let Some(shell) = crate::reference_shell(program, leaf_type) else {
                    let leaf_type = program
                        .type_reference_table
                        .unconstrained_type_reference(leaf_type);
                    let (definition, _) = super::type_instantiation::DeclarationScopes::default()
                        .definition(program, leaf_type, 0)?;
                    if definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
                    {
                        return None;
                    }
                    continue;
                };
                let mut segments = prefix.clone();
                segments.extend_from_slice(&leaf.local_segments);
                leaf.local_segments = segments.clone();
                leaf.local_path = super::place_paths::coordinate_storage_path(
                    program,
                    parameter.name.as_str(),
                    &segments,
                )
                .0;
                if borrowed {
                    leaves.push(SlotInventory {
                        origin: leaf,
                        access: shell.access,
                        ancestors: ancestors.clone(),
                    });
                }
                // A pointer under write-only storage cannot be loaded to
                // reach another destination. An owned/readable carrier can
                // lend the storage behind its exclusive pointer fields.
                if readable && shell.access.is_exclusive() {
                    let mut child_ancestors = ancestors.clone();
                    child_ancestors.push(segments.clone());
                    pending.push((
                        shell.referee,
                        segments,
                        true,
                        shell.access.is_readable(),
                        visited.clone(),
                        child_ancestors,
                    ));
                }
            }
        }
    }
    Some(leaves)
}

/// Seed the incoming pointee slots before the ordinary transfer walk. Missing
/// rows after that walk are unknown, never evidence that a slot was untouched.
/// Unsupported reachable inventories add no knowledge; any demanded summary
/// still fails in `destination_inventory`/`collect_exit`.
pub(super) fn seed_incoming_slots(
    program: &dyn crate::CheckingInput,
    state: &State,
    stored: &mut [StoredLocalOrigins],
) {
    let Some(inventory) = destination_inventory(program, state) else {
        return;
    };
    for declaration in inventory {
        if declaration.ancestors.is_empty() {
            continue;
        }
        let mut slot = declaration.origin;
        let Some(record) = stored
            .iter_mut()
            .find(|record| record.local_symbol == slot.local_symbol)
        else {
            continue;
        };
        if record
            .references
            .iter()
            .any(|leaf| leaf.local_segments == slot.local_segments)
        {
            continue;
        }
        slot.origin = Some(FramePlaceOrigin {
            path: slot.local_path.clone(),
            precision: FramePathPrecision::Exact,
            source: FrameSourcePlace {
                root: slot.local_symbol,
                segments: slot.local_segments.clone(),
                builtin_coordinates: true,
                ..Default::default()
            },
        });
        record.references.push(slot);
    }
}

pub(super) fn state_effects(
    program: &dyn crate::CheckingInput,
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
    inventory: Vec<SlotInventory>,
    exits: Vec<SlotExit>,
}

/// Least fixed point over normal exits. An unvisited return is not an identity
/// route: only a real normal exit can seed incoming contents. Root substitution
/// finiteness bounds the source set before iteration; there is no depth cutoff.
fn solve_state_effects<'program>(
    program: &'program dyn crate::CheckingInput,
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
            .indexed_statements(state.statement_nodes)
        {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            has_transition = true;
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            let prefix = prefixes.get(ordinal)?.as_ref()?;
            for edge in transition.targets() {
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
            let mut returned = Vec::new();
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
                returned.push(stored);
            }
            if returned.is_empty() {
                continue;
            }
            // A slice's type inventory is a default for every element. Exact
            // writes refine that inventory, and every exit must answer every
            // refined slot. Otherwise an unchanged branch would disappear
            // when another branch replaces one element.
            let mut inventory = equation.inventory.clone();
            for declaration in &equation.inventory {
                for stored in &returned {
                    let record = stored
                        .iter()
                        .find(|record| record.local_symbol == declaration.origin.local_symbol)?;
                    for leaf in &record.references {
                        if leaf.local_segments.len() != declaration.origin.local_segments.len()
                            || !stored_origins::path_covers(
                                &declaration.origin.local_segments,
                                &leaf.local_segments,
                            )
                            || inventory.iter().any(|existing| {
                                existing.origin.local_symbol == leaf.local_symbol
                                    && existing.origin.local_segments == leaf.local_segments
                            })
                        {
                            continue;
                        }
                        let mut refined = declaration.clone();
                        refined.origin.local_segments = leaf.local_segments.clone();
                        for ancestor in &mut refined.ancestors {
                            for (segment, selected) in ancestor.iter_mut().zip(&leaf.local_segments)
                            {
                                if matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid())
                                {
                                    *segment = *selected;
                                }
                            }
                        }
                        inventory.push(refined);
                    }
                }
            }
            for stored in &returned {
                collect_exit(program, equation.state, &inventory, stored, &mut candidate)?;
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
                    if prior.access != effect.access {
                        return None;
                    }
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
    program: &dyn crate::CheckingInput,
    state: &State,
    inventory: &[SlotInventory],
    stored: &[StoredLocalOrigins],
    effects: &mut Vec<SlotEffect>,
) -> Option<()> {
    for declaration in inventory {
        let slot = &declaration.origin;
        // Runtime selector identity cannot escape its expression namespace.
        // A type-derived wildcard is different: it records incoming contents
        // for untouched elements, beside independently checked exact overlays.
        if slot
            .local_segments
            .iter()
            .any(|part| matches!(part, PlaceSegment::Index { expression } if expression.is_valid()))
        {
            return None;
        }
        let record = stored
            .iter()
            .find(|record| record.local_symbol == slot.local_symbol)?;
        for ancestor in &declaration.ancestors {
            let matching = stored_origins::slot_origins(record, ancestor)?;
            let origin = matching.first()?;
            if matching.len() != 1
                || origin.source.root != slot.local_symbol
                || &origin.source.segments != ancestor
                || !origin.source.builtin_coordinates
            {
                return None;
            }
        }
        let sources = stored_origins::slot_origins(record, &slot.local_segments)?;
        for source in sources {
            if !source.source.builtin_coordinates {
                return None;
            }
            if !source.source.is_static() {
                if !source.source.root.is_valid() {
                    return None;
                }
                let parameter = program.state_parameter_by_symbol(state, source.source.root)?;
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
            if !incoming
                && slot
                    .local_segments
                    .iter()
                    .any(|part| matches!(part, PlaceSegment::Index { .. }))
            {
                return None;
            }
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
                    access: declaration.access,
                });
                effects.last_mut()?
            };
            if effect.access != declaration.access {
                return None;
            }
            effect.incoming |= incoming;
            let source = SlotSource {
                origin: source.clone(),
                carrier: source.source.clone(),
            };
            if !incoming && !effect.sources.contains(&source) {
                effect.sources.push(source);
            }
        }
    }
    Some(())
}

fn instantiate(
    program: &dyn crate::CheckingInput,
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
    // The relation below captures actuals from the incoming inventory. Pure
    // argument calls preserve it; a write during argument evaluation needs a
    // post-evaluation inventory, which this query must not fabricate from the
    // pre-call snapshot. Use ordinary frame inference, not a syntactic call ban.
    if arguments
        .iter()
        .any(|argument| super::caller_aliases::expression_has_calls(program, *argument))
    {
        let actual = *arguments.first()?;
        let (caller_state, _, _) = super::caller_aliases::caller_statement_owner(
            program,
            machine,
            super::caller_aliases::CallerWriteSite::Expression(actual),
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
    let substitute = |root, segments: &[PlaceSegment], inference: &mut FrameInference| {
        let index = typed_trees::signature::state_parameter_position(parameters, root)?;
        destination_source(
            program,
            machine,
            &parameters[index],
            *arguments.get(index)?,
            segments,
            context,
            symbols,
            inference,
        )
    };
    let mut output = Vec::new();
    for effect in relative {
        // An identity row proves preservation; it has no store to instantiate.
        // In particular, a wildcard default is not an executable selector.
        if effect.incoming && effect.sources.is_empty() {
            continue;
        }
        let destination = substitute(effect.root, &effect.segments, inference)?;
        let mut sources = Vec::new();
        for supplied in &effect.sources {
            let source = &supplied.origin;
            if !source.source.is_static() {
                let index = typed_trees::signature::state_parameter_position(
                    parameters,
                    source.source.root,
                )?;
                if !super::type_reference_is_reference(program, parameters[index].type_reference) {
                    for source in aggregate_argument_sources(
                        program,
                        machine,
                        &parameters[index],
                        *arguments.get(index)?,
                        supplied,
                        context,
                        symbols,
                        inference,
                    )? {
                        if !sources.contains(&source) {
                            sources.push(source);
                        }
                    }
                    continue;
                }
            }
            // Keep the actual descriptor home beside its backing. Resolving
            // only the backing would turn an exclusive transfer into a fresh
            // competing loan of the same referent.
            let carrier = if supplied.carrier.is_static() {
                supplied.carrier.clone()
            } else {
                let index = typed_trees::signature::state_parameter_position(
                    parameters,
                    supplied.carrier.root,
                )?;
                let actual = FrameSourcePlace::from_expression(program, *arguments.get(index)?);
                if !actual.root.is_valid() || !actual.builtin_coordinates {
                    return None;
                }
                actual.append_relative(&supplied.carrier)
            };
            let candidates = if source.source.is_static() {
                vec![Some(source.clone())]
            } else {
                let index = typed_trees::signature::state_parameter_position(
                    parameters,
                    source.source.root,
                )?;
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
                let source = SlotSource {
                    origin: source,
                    carrier: carrier.clone(),
                };
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
            access: effect.access,
        });
    }
    Some(output)
}

/// Resolve each pointer boundary before appending the destination slot.
/// Only the selected descriptor is interpreted; the final slot's old contents
/// never become its own storage address.
#[allow(clippy::too_many_arguments)]
fn destination_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    actual: ExpressionHandle,
    mut segments: &[PlaceSegment],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<FrameSourcePlace> {
    if segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. }
                | PlaceSegment::Case { .. }
                | PlaceSegment::FixedIndex { .. }
        )
    }) {
        return None;
    }
    let mut reference = parameter.type_reference;
    let mut base = None;
    if let Some(shell) = crate::reference_shell(program, reference) {
        if !shell.access.is_exclusive() {
            return None;
        }
        base = Some(argument_origin(
            program, machine, parameter, actual, context, symbols, inference,
        )?);
        reference = shell.referee;
    }
    loop {
        let inventory = stored_origins::declared_origins_for_query(
            program,
            parameter.symbol,
            parameter.name.as_str(),
            reference,
            true,
        )?;
        let mut boundaries = inventory.references.iter().filter(|leaf| {
            leaf.local_segments.len() < segments.len() && segments.starts_with(&leaf.local_segments)
        });
        let Some(boundary) = boundaries.next() else {
            let leaf = stored_origins::projected_storage_type(program, reference, segments)?;
            crate::reference_shell(program, leaf)?;
            let base = base?;
            if !base.source.root.is_valid()
                || base.source.is_static()
                || !base.source.builtin_coordinates
            {
                return None;
            }
            return Some(base.source.append_segments(segments));
        };
        if boundaries.next().is_some() {
            return None;
        }
        let shell = crate::reference_shell(
            program,
            stored_origins::projected_storage_type(program, reference, &boundary.local_segments)?,
        )?;
        if !shell.access.is_exclusive() {
            return None;
        }
        let candidates = if let Some(captured) = &base {
            let mut projected = captured.clone();
            projected.source = captured.source.append_segments(&boundary.local_segments);
            projected.path = path(program, &projected.source);
            stored_origins::captured_reference_projection_origins(
                program,
                captured,
                &projected,
                &context.stored,
            )
            .into_iter()
            .collect::<Option<Vec<_>>>()?
        } else {
            let relative = FrameSourcePlace {
                root: parameter.symbol,
                segments: boundary.local_segments.clone(),
                builtin_coordinates: true,
                ..Default::default()
            };
            aggregate_argument_sources(
                program,
                machine,
                parameter,
                actual,
                &SlotSource {
                    origin: FramePlaceOrigin {
                        path: path(program, &relative),
                        precision: FramePathPrecision::Exact,
                        source: relative.clone(),
                    },
                    carrier: relative,
                },
                context,
                symbols,
                inference,
            )?
            .into_iter()
            .map(|source| source.origin)
            .collect()
        };
        let mut candidates = candidates.into_iter();
        base = Some(candidates.next()?);
        if candidates.next().is_some() {
            return None;
        }
        reference = shell.referee;
        segments = &segments[boundary.local_segments.len()..];
    }
}

/// Instantiate value operands through their returned leaf relation before
/// resolving caller backing. The symbolic leaf is the supplying carrier;
/// resolving it first would lose the exclusive permission being transferred.
/// This is shared by names, projections, constructors and computed results.
#[allow(clippy::too_many_arguments)]
fn aggregate_argument_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    actual: ExpressionHandle,
    supplied: &SlotSource,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotSource>> {
    // A relative normal-exit row must still identify the same input leaf for
    // carrier transport and backing substitution. Do not pair independent
    // alternatives merely because their referents happen to overlap.
    if supplied.carrier != supplied.origin.source {
        return None;
    }
    let carriers = super::result_origins::instantiate_source(
        program,
        machine,
        parameter,
        actual,
        &supplied.origin,
        symbols,
        inference,
        true,
        &|expression, _, _, inference| {
            let (state, _, index) = super::caller_aliases::caller_statement_owner(
                program,
                machine,
                super::caller_aliases::CallerWriteSite::Expression(expression),
            )?;
            super::result_origins::reference_result_relation(
                program, machine, state, index, expression, symbols, inference,
            )?
            .references
            .into_iter()
            .map(|leaf| leaf.origin)
            .collect()
        },
        &|expression, reference, _| {
            let (state, before, _) = super::caller_aliases::caller_statement_owner(
                program,
                machine,
                super::caller_aliases::CallerWriteSite::Expression(expression),
            )?;
            stored_origins::reference_leaves_before_statement_for_query(
                program, state, before, expression, reference, None, None, true,
            )
        },
    )?;
    let mut sources = Vec::new();
    for carrier in carriers {
        if (!carrier.source.root.is_valid() && !carrier.source.is_static())
            || !carrier.source.builtin_coordinates
        {
            return None;
        }
        for origin in stored_origins::canonical_reference_origins(
            program,
            &Some(carrier.clone()),
            &context.aliases,
            &context.stored,
        ) {
            let origin = origin?;
            if (!origin.source.root.is_valid() && !origin.source.is_static())
                || !origin.source.builtin_coordinates
            {
                return None;
            }
            let source = SlotSource {
                origin,
                carrier: carrier.source.clone(),
            };
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
    }
    (!sources.is_empty()).then_some(sources)
}

fn argument_origin(
    program: &dyn crate::CheckingInput,
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
    let (path, precision) = super::place_paths::coordinate_storage_path(
        program,
        program.symbols.name(source.root),
        &source.segments,
    );
    Some(FramePlaceOrigin {
        path,
        precision,
        source,
    })
}

fn path(program: &dyn crate::CheckingInput, source: &FrameSourcePlace) -> String {
    super::place_paths::coordinate_storage_path(
        program,
        program.symbols.name(source.root),
        &source.segments,
    )
    .0
}

pub(super) fn apply(
    program: &dyn crate::CheckingInput,
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
        // Materialize the old concrete slot before installing an overlay on
        // a conditional write. Keeping only its wildcard default would let
        // the new exact row shadow the no-write branch's surviving origin.
        let incoming = if effect.incoming {
            stored_origins::slot_origins(record, &effect.segments)
        } else {
            Some(Vec::new())
        };
        let incoming_unknown = incoming.is_none();
        // Installing a pointer changes the storage reached beneath it. Old
        // pointee-slot snapshots cannot outlive that replacement, even when
        // another normal route preserves the pointer's incoming contents.
        record.references.retain(|leaf| {
            !((!effect.incoming || !effect.sources.is_empty())
                && leaf.local_segments.len() > effect.segments.len()
                && leaf.local_segments.starts_with(&effect.segments))
                && (effect.incoming || leaf.local_segments != effect.segments)
        });
        let origins = incoming
            .into_iter()
            .flatten()
            .map(Some)
            .chain(incoming_unknown.then_some(None))
            .chain(
                effect
                    .sources
                    .iter()
                    .map(|source| Some(source.origin.clone())),
            );
        for origin in origins {
            if record
                .references
                .iter()
                .any(|leaf| leaf.local_segments == effect.segments && leaf.origin == origin)
            {
                continue;
            }
            record.references.push(StoredWriteOrigin {
                local_symbol: effect.root,
                local_path: path(
                    program,
                    &FrameSourcePlace {
                        root: effect.root,
                        segments: effect.segments.clone(),
                        builtin_coordinates: true,
                        ..Default::default()
                    },
                ),
                local_segments: effect.segments.clone(),
                origin,
            });
        }
        record.symbolic = false;
    }
}

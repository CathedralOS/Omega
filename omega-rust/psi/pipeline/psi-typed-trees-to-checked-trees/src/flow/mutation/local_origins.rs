//! Project the shared frame engine's local storage origins into flow places.
//!
//! Alias transfer belongs to validation. This adapter resolves its canonical
//! names to existing typed symbols and retains structured selectors only when
//! the shared origin is exact.

use super::*;

/// Facts can be expressed through any live reference to the written storage.
/// Transport the shared prefix origins into this representation; do not infer
/// bindings here or change the access routes used by borrow authorization.
pub(in crate::flow) fn close_storage_places_over_aliases(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    mut places: Vec<CanonicalPlace>,
) -> Option<Vec<CanonicalPlace>> {
    if places.is_empty() {
        return Some(places);
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = find_state(program, state_symbol)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let resolver = psi_validation::CallFrameResolver::new(program)?;
    let origins = resolver.local_write_origins_before_statement(machine, statement)?;
    let storage = places.clone();
    for origin in origins {
        let source = place_from_origin_path(program, state, statement_index, &origin.source_path)?;
        for place in &storage {
            if crate::flow::normalized_event_place_root(program, source.root)
                != crate::flow::normalized_event_place_root(program, place.root)
            {
                continue;
            }
            let mut alias = canonical_place_from_symbol(origin.local_symbol)?;
            alias.segments.extend_from_slice(&origin.local_segments);
            if !canonical_place_segments_may_overlap(program, &place.segments, &source.segments) {
                continue;
            }
            if place.segments.len() >= source.segments.len() && !origin.collection_coarse {
                alias
                    .segments
                    .extend_from_slice(&place.segments[source.segments.len()..]);
            }
            if !places.contains(&alias) {
                places.push(alias);
            }
        }
    }
    Some(places)
}

pub(super) fn assignment_storage_places(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<Vec<CanonicalPlace>> {
    let StatementNode::Assignment(assignment) = statement else {
        return None;
    };
    let mut direct = canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        assignment.target,
    );
    if let Some(place) = &mut direct {
        normalize_write_only_range_place(program, state_symbol, place);
        if !place_requires_local_write_origin(program, state_symbol, statement_index, place) {
            return direct.map(|place| vec![place]);
        }
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let resolver = psi_validation::CallFrameResolver::new(program)?;
    match resolver.assignment_write_target(machine, statement)? {
        psi_validation::AssignmentWriteTarget::LocalBindingReplacement { .. } => {
            direct.map(|place| vec![place])
        }
        psi_validation::AssignmentWriteTarget::Storage { paths } => {
            if let Some(place) = direct {
                rebase_local_write_places(program, state_symbol, statement_index, place)
            } else {
                paths
                    .iter()
                    .map(|path| {
                        place_from_origin_path(
                            program,
                            find_state(program, state_symbol)?,
                            statement_index,
                            path,
                        )
                    })
                    .collect()
            }
        }
    }
}

fn place_requires_local_write_origin(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
) -> bool {
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    let Some(state) = find_state(program, state_symbol) else {
        return false;
    };
    let Some(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == root => Some(local),
            _ => None,
        })
    else {
        return false;
    };
    psi_validation::CallFrameResolver::new(program)
        .is_none_or(|resolver| resolver.local_requires_write_origin(local.type_reference))
}

pub(super) fn rebase_local_write_places(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    if !place_requires_local_write_origin(program, state_symbol, statement_index, &place) {
        return Some(vec![place]);
    }
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let state = find_state(program, state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state_symbol)
    })?;
    let resolver = psi_validation::CallFrameResolver::new(program)?;
    let origins =
        resolver.local_write_origins_before_statement(machine, statements.get(statement_index)?)?;
    let mut projected = Vec::new();
    let mut retains_private_storage = true;
    for origin in origins.iter().filter(|origin| origin.local_symbol == root) {
        if !canonical_place_segments_may_overlap(program, &place.segments, &origin.local_segments) {
            continue;
        }
        let mut canonical =
            place_from_origin_path(program, state, statement_index, &origin.source_path)?;
        if place.segments.len() >= origin.local_segments.len() {
            retains_private_storage = false;
            if !origin.collection_coarse {
                canonical
                    .segments
                    .extend_from_slice(&place.segments[origin.local_segments.len()..]);
            }
        }
        if !projected.contains(&canonical) {
            projected.push(canonical);
        }
    }
    // A complete prefix accounts for every write-capable reference leaf.
    // A disjoint owned field, or an ancestor also containing owned storage,
    // still has local facts to invalidate. Caller summaries filter that root.
    if retains_private_storage && !projected.contains(&place) {
        projected.push(place);
    }
    Some(projected)
}

pub(super) fn place_from_origin_path(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    path: &str,
) -> Option<CanonicalPlace> {
    let mut members = path.split('.');
    let root_name = members.next()?;
    let root = program
        .state_parameters(state)
        .iter()
        .find_map(|parameter| {
            ((parameter.is_self && root_name == "self") || parameter.name.as_str() == root_name)
                .then_some(parameter.symbol)
        })
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
                .find_map(|statement| {
                    let StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.name.as_str() == root_name).then_some(local.symbol)
                })
        })?;
    let mut place = canonical_place_from_symbol(root)?;
    let mut symbol = root;
    for member in members {
        let type_symbol = symbol_type_symbol(program, symbol)?;
        symbol = resolve_member_symbol_from_type_symbol(program, type_symbol, member)?;
        push_field_place_segments(program, &mut place.segments, symbol);
    }
    Some(place)
}

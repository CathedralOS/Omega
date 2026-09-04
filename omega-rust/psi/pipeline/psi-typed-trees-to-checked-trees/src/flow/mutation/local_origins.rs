//! Project the shared frame engine's local storage origins into flow places.
//!
//! Alias transfer belongs to validation. This adapter resolves its canonical
//! names to existing typed symbols and retains structured selectors only when
//! the shared origin is exact.

use super::*;

pub(super) fn rebase_local_write_place(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
        return Some(place);
    };
    let state = find_state(program, state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let Some(local) = statements
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == root).then_some(local)
        })
    else {
        return Some(place);
    };
    let mut local_type = local.type_reference;
    loop {
        match program.type_reference_table.type_reference(local_type) {
            psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                local_type = *base_type;
            }
            psi_typed_trees::types::TypeReferenceNode::Reference { .. } => break,
            _ => return Some(place),
        }
    }
    let machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state_symbol)
    })?;
    let resolver = psi_validation::CallFrameResolver::new(program)?;
    let origins =
        resolver.local_write_origins_before_statement(machine, statements.get(statement_index)?)?;
    let origin = origins.iter().find(|origin| origin.local_symbol == root)?;
    let mut canonical =
        place_from_origin_path(program, state, statement_index, &origin.source_path)?;
    if !origin.collection_coarse {
        canonical.segments.extend(place.segments);
    }
    Some(canonical)
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

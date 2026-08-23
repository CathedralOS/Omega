use super::resolution::{effective_member_symbol, resolve_member_symbol_from_type_symbol};
use super::*;
use crate::lookup::first_valid_name_path_symbol;

pub(super) fn contextual_canonical_place_from_expression(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => contextual_canonical_place_from_expression(
            program,
            state_symbol,
            statement_index,
            inner.target,
        ),
        ExpressionNode::Name(path) => {
            let root_symbol = resolve_contextual_name_path_root(
                program,
                state_symbol,
                statement_index,
                expression,
                path,
            )?;
            let mut place = CanonicalPlace {
                root: psi_facts::PlaceRoot::Symbol(root_symbol),
                segments: Vec::new(),
            };
            let members = program.expression_table.name_path_members(path.members);
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let start_index = usize::from(
                members
                    .first()
                    .is_some_and(|member| member.as_str() == "self"),
            );
            for (offset, member_name) in members.iter().skip(start_index + 1).enumerate() {
                let symbol = member_symbols
                    .get(offset + start_index + 1)
                    .copied()
                    .filter(|symbol| symbol.is_valid())
                    .or_else(|| {
                        resolve_member_symbol_from_place(program, &place, member_name.as_str())
                    })
                    .unwrap_or_else(SymbolHandle::invalid);
                push_field_place_segments(program, &mut place.segments, symbol);
            }
            Some(place)
        }
        ExpressionNode::Member(member) => {
            let mut place = contextual_canonical_place_from_expression(
                program,
                state_symbol,
                statement_index,
                member.receiver,
            )?;
            let symbol = {
                let symbol = effective_member_symbol(program, member.receiver, member);
                if symbol.is_valid() {
                    symbol
                } else {
                    resolve_member_symbol_from_place(program, &place, member.member.as_str())
                        .unwrap_or_else(SymbolHandle::invalid)
                }
            };
            push_field_place_segments(program, &mut place.segments, symbol);
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place = contextual_canonical_place_from_expression(
                program,
                state_symbol,
                statement_index,
                indexed.collection,
            )?;
            place
                .segments
                .push(index_place_segment(program, indexed.index));
            Some(place)
        }
        _ => None,
    }
}

fn resolve_contextual_name_path_root(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
) -> Option<SymbolHandle> {
    let name = program.expression_table.display_name(expression);
    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    if name == "self" {
        return program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol);
    }

    if let Some(symbol) = first_valid_name_path_symbol(path, &program.expression_table) {
        return Some(symbol);
    }

    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| !parameter.is_self && parameter.name.as_str() == name)
    {
        return Some(parameter.symbol);
    }

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local_data) if local_data.name.as_str() == name => {
                Some(local_data.symbol)
            }
            _ => None,
        })
}

fn resolve_member_symbol_from_place(
    program: &psi_typed_trees::TypedTrees,
    place: &CanonicalPlace,
    member_name: &str,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        psi_facts::PlaceRoot::Symbol(symbol) => resolution::symbol_type_symbol(program, symbol)?,
        psi_facts::PlaceRoot::Expression(expression) => {
            resolution::expression_type_symbol(program, expression)?
        }
        psi_facts::PlaceRoot::Unknown | psi_facts::PlaceRoot::TypeReference(_) => return None,
    };

    for segment in &place.segments {
        match segment {
            psi_facts::PlaceSegment::Case { .. } => {}
            psi_facts::PlaceSegment::Field { symbol } => {
                current = resolution::symbol_type_symbol(program, *symbol)?;
            }
            psi_facts::PlaceSegment::FixedIndex { .. }
            | psi_facts::PlaceSegment::FixedRange { .. }
            | psi_facts::PlaceSegment::Index { .. } => {
                return None;
            }
        }
    }

    resolve_member_symbol_from_type_symbol(program, current, member_name)
}

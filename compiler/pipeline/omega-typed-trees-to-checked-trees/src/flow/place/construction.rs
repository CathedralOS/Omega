use super::*;
use crate::lookup::first_valid_name_path_symbol;
use resolution::{effective_member_symbol, resolve_member_symbol_from_type_symbol};

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

pub(crate) fn canonical_place_from_expression_in_state(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    canonical_place_from_expression(program, expression).or_else(|| {
        contextual_canonical_place_from_expression(
            program,
            state_symbol,
            statement_index,
            expression,
        )
    })
}

pub(crate) fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
    symbol.is_valid().then_some(CanonicalPlace {
        root: omega_facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    })
}

pub(crate) fn canonical_place_from_semantic_place(
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

fn contextual_canonical_place_from_expression(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => contextual_canonical_place_from_expression(
            program,
            state_symbol,
            statement_index,
            *inner,
        ),
        ExpressionNode::Name(path) => {
            let root_symbol =
                resolve_contextual_name_path_root(program, state_symbol, statement_index, expression, path)?;
            let mut place = CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(root_symbol),
                segments: Vec::new(),
            };
            let members = program.expression_table.name_path_members(path.members);
            let member_symbols = program.expression_table.name_path_member_symbols(path.member_symbols);
            let start_index = usize::from(members.first().is_some_and(|member| member.as_str() == "self"));
            for (offset, member_name) in members.iter().skip(start_index + 1).enumerate() {
                let symbol = member_symbols
                    .get(offset + start_index + 1)
                    .copied()
                    .filter(|symbol| symbol.is_valid())
                    .or_else(|| resolve_member_symbol_from_place(program, &place, member_name.as_str()))
                    .unwrap_or_else(SymbolHandle::invalid);
                place.segments.push(omega_facts::PlaceSegment::Field { symbol });
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
            place.segments.push(omega_facts::PlaceSegment::Field { symbol });
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place = contextual_canonical_place_from_expression(
                program,
                state_symbol,
                statement_index,
                indexed.collection,
            )?;
            place.segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(place)
        }
        _ => None,
    }
}

fn resolve_contextual_name_path_root(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    path: &omega_typed_trees::expression::TableNamePath,
) -> Option<SymbolHandle> {
    if let Some(symbol) = first_valid_name_path_symbol(path, &program.expression_table) {
        return Some(symbol);
    }

    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    let name = program.expression_table.display_name(expression);
    if name == "self" {
        return program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol);
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
    program: &omega_typed_trees::TypedTrees,
    place: &CanonicalPlace,
    member_name: &str,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        omega_facts::PlaceRoot::Symbol(symbol) => resolution::symbol_type_symbol(program, symbol)?,
        omega_facts::PlaceRoot::Expression(expression) => {
            resolution::expression_type_symbol(program, expression)?
        }
        omega_facts::PlaceRoot::Unknown | omega_facts::PlaceRoot::TypeReference(_) => return None,
    };

    for segment in &place.segments {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                current = resolution::symbol_type_symbol(program, *symbol)?;
            }
            omega_facts::PlaceSegment::Index { .. } => return None,
        }
    }

    resolve_member_symbol_from_type_symbol(program, current, member_name)
}

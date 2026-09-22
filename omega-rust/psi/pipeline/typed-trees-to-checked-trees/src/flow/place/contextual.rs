use super::resolution;
use super::resolution::{effective_member_symbol, resolve_member_symbol_from_type_symbol};
use crate::flow::CanonicalPlace;
use crate::flow::index_place_segment;
use crate::flow::push_field_place_segments;
use crate::lookup::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use language_core::is_self_receiver;
use symbols::SymbolHandle;

pub(crate) fn contextual_canonical_place_from_expression(
    program: &typed_trees::TypedTrees,
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
                root: facts::PlaceRoot::Symbol(root_symbol),
                segments: Vec::new(),
            };
            let members = program.expression_table.name_path_members(path.members);
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let start_index = usize::from(
                members
                    .first()
                    .is_some_and(|member| member.is_self_receiver()),
            );
            for (offset, member_name) in members.iter().skip(start_index + 1).enumerate() {
                let symbol = member_symbols
                    .get(offset + start_index + 1)
                    .copied()
                    .filter(|symbol| symbol.is_valid())
                    .or_else(|| {
                        resolve_member_symbol_from_place(
                            program,
                            &place,
                            member_name.as_str(),
                            None,
                        )
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
                    resolve_member_symbol_from_place(
                        program,
                        &place,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|variant| variant.as_str()),
                    )
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
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
) -> Option<SymbolHandle> {
    let name = program.expression_table.display_name(expression);
    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    if is_self_receiver(&name) {
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

/// Resolve the demanded member on the place walk's leaf. `case_variant`
/// carries the member expression's destructure qualification when it has
/// one: payload spellings repeat across cases, so a qualified demand is
/// answered only by the field that exact variant declares — an unqualified
/// name search would mint a sibling case's same-named field for it.
fn resolve_member_symbol_from_place(
    program: &typed_trees::TypedTrees,
    place: &CanonicalPlace,
    member_name: &str,
    case_variant: Option<&str>,
) -> Option<SymbolHandle> {
    let position = match place.root {
        facts::PlaceRoot::Symbol(symbol) => resolution::symbol_type_position(program, symbol)?,
        facts::PlaceRoot::Expression(expression) => {
            resolution::expression_type_position(program, expression)?
        }
        // A type-reference root names the place's own stored type: the walk
        // resumes at that reference with its reaching application intact.
        facts::PlaceRoot::TypeReference(reference) => {
            resolution::MemberPosition::Reference(reference)
        }
        facts::PlaceRoot::Unknown => return None,
    };

    // The segment hops are the shared fold `member_position_after_segments`
    // owns: reference positions replay the reaching generic application's
    // own substitution, declaration positions resume at the member's own
    // declared type, and a window keeps its element while refusing member
    // demands — the same contract `expression_place_type_reference` replays
    // for reference-world consumers.
    let position = resolution::member_position_after_segments(program, position, &place.segments)?;

    let current = resolution::position_leaf_symbol(program, position);
    match case_variant {
        Some(variant) => resolution::resolve_case_member_symbol_from_type_symbol(
            program,
            current,
            variant,
            member_name,
        ),
        None => resolve_member_symbol_from_type_symbol(program, current, member_name),
    }
}

//! Resolve storage names and members at an exact statement prefix.

use super::canonical_projection::CanonicalPlace;
use super::canonical_projection::index_place_segment;
use super::canonical_projection::push_field_place_segments;
use super::member_resolution;
use super::member_resolution::resolve_member_symbol_from_type_symbol;
use crate::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

pub fn contextual_canonical_place_from_expression(
    program: &dyn crate::CheckingInput,
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
        // A borrow recast denotes the operand's storage, not a fresh rvalue.
        ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            contextual_canonical_place_from_expression(
                program,
                state_symbol,
                statement_index,
                cast.value,
            )
        }
        ExpressionNode::Name(path) => {
            if super::canonical_projection::name_materializes_value(program, expression) {
                return super::canonical_projection::canonical_place_from_expression(
                    program, expression,
                );
            }
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
                let retained = member_symbols
                    .get(offset + start_index + 1)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                let symbol = resolve_member_symbol_from_place(
                    program,
                    &place,
                    member_name.as_str(),
                    None,
                    retained,
                )
                .unwrap_or(retained);
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
                let symbol = member_resolution::effective_member_symbol_from_position(
                    program,
                    member.receiver,
                    member_resolution::expression_type_position(program, member.receiver),
                    member,
                );
                if symbol.is_valid() {
                    symbol
                } else {
                    resolve_member_symbol_from_place(
                        program,
                        &place,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|variant| variant.as_str()),
                        member.member_symbol,
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
            // A whole builtin window preserves every element's coordinate.
            // Keeping a range selector here would instead put facts below a
            // phantom extra collection dimension, losing domains and cleanup
            // predicates even though the view names the same storage.
            if let Some((machine, state)) = crate::find_state(program, state_symbol)
                && super::builtin_coordinates::whole_collection_view_source(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                .is_some()
            {
                return Some(place);
            }
            place
                .segments
                .push(index_place_segment(program, indexed.index));
            Some(place)
        }
        // Computed backing participates in the same contextual projection
        // walk as named storage. A member or index cannot erase its origin.
        _ => super::canonical_projection::canonical_place_from_expression(program, expression),
    }
}

fn resolve_contextual_name_path_root(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
) -> Option<SymbolHandle> {
    let name = program.expression_table.display_name(expression);
    let state = program.state_by_symbol(state_symbol)?;
    if let Some(symbol) = first_valid_name_path_symbol(path, &program.expression_table) {
        return Some(symbol);
    }

    crate::state_scoped_name_symbol(program, state, &name, statement_index)
}

/// Resolve the demanded member on the place walk's leaf. `case_variant`
/// carries the member expression's destructure qualification when it has
/// one: payload spellings repeat across cases, so a qualified demand is
/// answered only by the field that exact variant declares — an unqualified
/// name search would mint a sibling case's same-named field for it.
pub(super) fn resolve_member_symbol_from_place(
    program: &dyn crate::CheckingInput,
    place: &CanonicalPlace,
    member_name: &str,
    case_variant: Option<&str>,
    retained: SymbolHandle,
) -> Option<SymbolHandle> {
    let position = match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            member_resolution::symbol_type_position(program, symbol)?
        }
        facts::PlaceRoot::Expression(expression) => {
            member_resolution::expression_type_position(program, expression)?
        }
        // A type-reference root names the place's own stored type: the walk
        // resumes at that reference with its reaching application intact.
        facts::PlaceRoot::TypeReference(reference) => {
            member_resolution::MemberPosition::Reference(reference)
        }
        facts::PlaceRoot::Unknown => return None,
    };

    // The segment hops are the shared fold `member_position_after_segments`
    // owns: reference positions replay the reaching generic application's
    // own substitution, declaration positions resume at the member's own
    // declared type, and a window keeps its element while refusing member
    // demands — the same contract `expression_place_type_reference` replays
    // for reference-world consumers.
    let position =
        member_resolution::member_position_after_segments(program, position, &place.segments)?;

    if case_variant.is_none() {
        if let Some(symbol) = member_resolution::closed_generic_member_symbol(
            program,
            Some(position),
            member_name,
            retained,
        ) {
            return Some(symbol);
        }
        if retained.is_valid() {
            return Some(retained);
        }
    }
    let current = member_resolution::position_leaf_symbol(program, position);
    match case_variant {
        Some(variant) => member_resolution::resolve_case_member_symbol_from_type_symbol(
            program,
            current,
            variant,
            member_name,
        ),
        None => resolve_member_symbol_from_type_symbol(program, current, member_name),
    }
}

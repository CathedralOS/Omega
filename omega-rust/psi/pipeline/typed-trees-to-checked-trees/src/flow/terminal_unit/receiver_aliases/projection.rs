//! Join source coordinates to checked capture and normalized receiver paths.

use super::*;

pub(super) fn formation_place(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(crate::flow::CanonicalPlace, checked_trees::CapturedPlace)> {
    if !validation::place_has_builtin_coordinates(program, machine, Some(state), expression) {
        return None;
    }
    // Validate the authored coordinate spine; the ordinary place owner below
    // remains responsible for resolving fields and literal index geometry.
    let mut current = expression;
    let mut visited = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(current) || visited.contains(&current) {
            return None;
        }
        visited.push(current);
        match program.expression_table.expression(current) {
            ExpressionNode::Name(name) => {
                let symbols = program
                    .expression_table
                    .name_path_member_symbols(name.member_symbols);
                let members = program.expression_table.name_path_members(name.members);
                if !name.symbol.is_valid()
                    || !name.head_symbol.is_valid()
                    || members.is_empty()
                    || (members.len() == 1
                        && (name.symbol != name.head_symbol
                            || symbols.first().is_some_and(|symbol| *symbol != name.symbol)))
                    || (members.len() > 1
                        && (symbols.len() != members.len()
                            || symbols.iter().any(|symbol| !symbol.is_valid())
                            || symbols.first() != Some(&name.head_symbol)
                            || symbols.last() != Some(&name.symbol)))
                {
                    return None;
                }
                break;
            }
            ExpressionNode::Member(member) => {
                if validation::exact_self_field(program, machine, current).is_some() {
                    current = member.receiver;
                    continue;
                }
                let receiver_type = crate::flow::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    member.receiver,
                )?;
                let receiver_type = validation::unwrapped_type_reference(program, receiver_type)?;
                let resolved = crate::flow::resolve_member_symbol_from_type_symbol(
                    program,
                    program.type_reference_table.type_symbol(receiver_type),
                    member.member.as_str(),
                )?;
                if member.member_symbol.is_valid() && member.member_symbol != resolved {
                    return None;
                }
                current = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                if !program.expression_table.expression_is_valid(indexed.index)
                    || !matches!(
                        crate::flow::index_place_segment(program, indexed.index),
                        facts::PlaceSegment::FixedIndex { .. }
                    )
                {
                    return None;
                }
                current = indexed.collection;
            }
            _ => return None,
        }
    }
    let capture = crate::borrow::accesses::borrow_access_place(
        program,
        state.symbol,
        statement_index,
        expression,
        machine.symbol,
    )?;
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )?;
    crate::flow::normalize_attached_place_root(program, machine.symbol, state.symbol, &mut place);
    if !place
        .segments
        .iter()
        .chain(&capture.segments)
        .all(|segment| {
            matches!(segment,
        facts::PlaceSegment::Field { symbol } if symbol.is_valid())
                || matches!(segment, facts::PlaceSegment::FixedIndex { .. })
        })
    {
        return None;
    }
    Some((place, capture))
}

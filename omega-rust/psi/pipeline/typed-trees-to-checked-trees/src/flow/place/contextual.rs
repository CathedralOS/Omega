use super::resolution;
use super::resolution::{effective_member_symbol, resolve_member_symbol_from_type_symbol};
use crate::flow::CanonicalPlace;
use crate::flow::index_place_segment;
use crate::flow::push_field_place_segments;
use crate::lookup::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
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
                    .is_some_and(|member| member.as_str() == "self"),
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
    let mut position = match place.root {
        facts::PlaceRoot::Symbol(symbol) => resolution::symbol_type_position(program, symbol)?,
        facts::PlaceRoot::Expression(expression) => {
            resolution::expression_type_position(program, expression)?
        }
        facts::PlaceRoot::Unknown | facts::PlaceRoot::TypeReference(_) => return None,
    };

    for segment in &place.segments {
        match segment {
            facts::PlaceSegment::Case { .. } => {}
            facts::PlaceSegment::Field { symbol } => {
                // A reference position replays the reaching generic
                // application's own substitution (see
                // `project_type_reference_from_segments`), so a field whose
                // declared type is a bound parameter resumes at the supplied
                // argument — `Box<Context>::item` continues at `Context`.
                // When the hop does not replay (a declaration position, an
                // opaque leaf, or a field outside the replayed declaration)
                // the field's own declared position applies, as before.
                position = match position {
                    resolution::MemberPosition::Reference(reference) => {
                        match crate::flow::project_type_reference_from_segments(
                            program,
                            reference,
                            std::slice::from_ref(segment),
                        ) {
                            Some(projected) => resolution::MemberPosition::Reference(projected),
                            None => resolution::symbol_type_position(program, *symbol)?,
                        }
                    }
                    resolution::MemberPosition::Declaration(_) => {
                        resolution::symbol_type_position(program, *symbol)?
                    }
                };
            }
            facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                // The index hop lands on an element, not on the collection
                // itself: replay the reaching reference through the same
                // element projection `expression_type_position` applies to an
                // `Indexed` node, so `values[i]` resumes at the element
                // position with the collection's generic arguments still
                // bound. A position that does not project to an element keeps
                // no position rather than minting the collection's own for
                // the element; a declaration position names a record, which
                // has no element to resume at.
                position = match position {
                    resolution::MemberPosition::Reference(reference) => {
                        resolution::MemberPosition::Reference(
                            crate::flow::project_type_reference_from_segments(
                                program,
                                reference,
                                std::slice::from_ref(segment),
                            )?,
                        )
                    }
                    resolution::MemberPosition::Declaration(_) => return None,
                };
            }
            facts::PlaceSegment::FixedRange { .. } => {
                // A range hop produces a slice of the collection, not one
                // element; the projection has no range shape to replay, so
                // the demanded member stays unresolved here.
                return None;
            }
        }
    }

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

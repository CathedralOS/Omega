use super::contextual::{contextual_effective_member_symbol, contextual_name_root_symbol};
use crate::context::*;

pub(crate) use checked_trees::CapturedPlace as BorrowAccessPlace;

pub(crate) fn borrow_access_place(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<BorrowAccessPlace> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(_) => None,
        ExpressionNode::Indexed(indexed) => {
            let mut place = borrow_access_place(
                program,
                state_symbol,
                statement_index,
                indexed.collection,
                machine_symbol,
            )?;
            place
                .segments
                .push(crate::flow::index_place_segment(program, indexed.index));
            Some(place)
        }
        ExpressionNode::Range(_) => None,
        ExpressionNode::Borrow(inner) => borrow_access_place(
            program,
            state_symbol,
            statement_index,
            inner.target,
            machine_symbol,
        ),
        ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && contextual_name_root_symbol(
                            program,
                            state_symbol,
                            statement_index,
                            member.receiver,
                            path,
                        )
                        .is_some_and(|symbol| symbol == machine_symbol) =>
                {
                    let member_symbol = contextual_effective_member_symbol(
                        program,
                        state_symbol,
                        statement_index,
                        member.receiver,
                        member,
                        machine_symbol,
                    );
                    Some(BorrowAccessPlace {
                        root_symbol: member_symbol,
                        segments: Vec::new(),
                    })
                }
                _ => {
                    let mut place = borrow_access_place(
                        program,
                        state_symbol,
                        statement_index,
                        member.receiver,
                        machine_symbol,
                    )?;
                    let symbol = contextual_effective_member_symbol(
                        program,
                        state_symbol,
                        statement_index,
                        member.receiver,
                        member,
                        machine_symbol,
                    );
                    crate::flow::push_field_place_segments(program, &mut place.segments, symbol);
                    Some(place)
                }
            }
        }
        ExpressionNode::Name(path) => {
            let root_symbol = contextual_name_root_symbol(
                program,
                state_symbol,
                statement_index,
                expression,
                path,
            )?;
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let skip = member_symbols
                .iter()
                .position(|member_symbol| *member_symbol == root_symbol)
                .map(|index| index + 1)
                .unwrap_or(1);
            let mut segments = Vec::new();
            for symbol in member_symbols.iter().skip(skip).copied() {
                crate::flow::push_field_place_segments(program, &mut segments, symbol);
            }
            Some(BorrowAccessPlace {
                root_symbol,
                segments,
            })
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

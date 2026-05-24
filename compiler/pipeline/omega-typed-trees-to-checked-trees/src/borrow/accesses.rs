use super::*;
use crate::flow::effective_member_symbol;
use crate::lookup::first_valid_name_path_symbol;

pub(crate) fn collect_call_argument_accesses(
    program: &omega_typed_trees::TypedTrees,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    machine_symbol: SymbolHandle,
) -> omega_core::arena::HandleSpan<BorrowArgumentAccessFact> {
    let mut accesses = omega_core::arena::HandleSpan::empty();

    for argument in arguments {
        collect_argument_accesses(
            *argument,
            program,
            access_segments,
            argument_accesses,
            &mut accesses,
            machine_symbol,
        );
    }

    accesses
}

fn collect_argument_accesses(
    expression: ExpressionHandle,
    program: &omega_typed_trees::TypedTrees,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            if let Some(access_place) =
                borrow_access_place(program, *inner_expression, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol: access_place.root_symbol,
                        segments: access_segments.insert_many(access_place.segments),
                        kind: BorrowAccessKind::Mutable,
                    },
                );
            }
        }
        _ => collect_read_accesses(
            expression,
            program,
            access_segments,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
    }
}

fn collect_read_accesses(
    expression: ExpressionHandle,
    program: &omega_typed_trees::TypedTrees,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_read_accesses(
                    *value,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses(
                binary.left,
                program,
                access_segments,
                argument_accesses,
                accesses,
                machine_symbol,
            );
            collect_read_accesses(
                binary.right,
                program,
                access_segments,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_read_accesses(
                    call.receiver,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }

            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_read_accesses(
                    *argument,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_read_accesses(
            cast.value,
            program,
            access_segments,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::Indexed(indexed) => {
            if let Some(access_place) =
                borrow_access_place(program, expression, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol: access_place.root_symbol,
                        segments: access_segments.insert_many(access_place.segments),
                        kind: BorrowAccessKind::Read,
                    },
                );
            }

            collect_read_accesses(
                indexed.index,
                program,
                access_segments,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Member(member) => {
            if let Some(access_place) =
                borrow_access_place(program, expression, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol: access_place.root_symbol,
                        segments: access_segments.insert_many(access_place.segments),
                        kind: BorrowAccessKind::Read,
                    },
                );
            } else {
                collect_read_accesses(
                    member.receiver,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Name(_) => {
            if let Some(access_place) = borrow_access_place(program, expression, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol: access_place.root_symbol,
                        segments: access_segments.insert_many(access_place.segments),
                        kind: BorrowAccessKind::Read,
                    },
                );
            }
        }
        ExpressionNode::Mutable(inner_expression) => collect_read_accesses(
            *inner_expression,
            program,
            access_segments,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                collect_read_accesses(
                    field.value,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BorrowAccessPlace {
    root_symbol: SymbolHandle,
    segments: Vec<omega_facts::PlaceSegment>,
}

fn borrow_access_place(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<BorrowAccessPlace> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            let mut place = borrow_access_place(program, indexed.collection, machine_symbol)?;
            place.segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(place)
        }
        ExpressionNode::Mutable(inner) => {
            borrow_access_place(program, *inner, machine_symbol)
        }
        ExpressionNode::Member(member) => match program.expression_table.expression(member.receiver) {
            ExpressionNode::Name(path)
                if path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol =>
            {
                let member_symbol = effective_member_symbol(program, member.receiver, member);
                Some(BorrowAccessPlace {
                    root_symbol: member_symbol,
                    segments: Vec::new(),
                })
            }
            _ => {
                let mut place = borrow_access_place(program, member.receiver, machine_symbol)?;
                place.segments.push(omega_facts::PlaceSegment::Field {
                    symbol: effective_member_symbol(program, member.receiver, member),
                });
                Some(place)
            }
        },
        ExpressionNode::Name(path) => {
            let root_symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let skip = member_symbols
                .iter()
                .position(|member_symbol| *member_symbol == root_symbol)
                .map(|index| index + 1)
                .unwrap_or_default();
            Some(BorrowAccessPlace {
                root_symbol,
                segments: member_symbols
                    .iter()
                    .skip(skip)
                    .copied()
                    .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                    .collect(),
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
        | ExpressionNode::StructLiteral(_) => None,
    }
}

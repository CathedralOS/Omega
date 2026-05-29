mod contextual;
mod place;
mod read;

use super::*;

pub(crate) use place::{BorrowAccessPlace, borrow_access_place};
use read::collect_read_accesses;

pub(crate) fn collect_call_argument_accesses(
    program: &omega_typed_trees::TypedTrees,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
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
            state_symbol,
            statement_index,
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
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            if let Some(access_place) = borrow_access_place(
                program,
                state_symbol,
                statement_index,
                *inner_expression,
                machine_symbol,
            ) {
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
            state_symbol,
            statement_index,
            machine_symbol,
        ),
    }
}

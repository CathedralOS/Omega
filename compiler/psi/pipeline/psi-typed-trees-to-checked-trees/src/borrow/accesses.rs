mod collection;
mod contextual;
mod place;
mod read;
mod records;

use super::*;

use collection::BorrowAccessCollection;
pub(crate) use place::{BorrowAccessPlace, borrow_access_place};
use read::collect_read_accesses;

pub(crate) fn collect_call_argument_accesses(
    program: &psi_typed_trees::TypedTrees,
    access_segments: &mut psi_arena::Arena<psi_facts::PlaceSegment>,
    argument_accesses: &mut psi_arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
) -> psi_arena::HandleSpan<BorrowArgumentAccessFact> {
    let mut accesses = psi_arena::HandleSpan::empty();

    {
        let mut collection = BorrowAccessCollection::new(
            program,
            access_segments,
            argument_accesses,
            &mut accesses,
            state_symbol,
            statement_index,
            machine_symbol,
        );

        for argument in arguments {
            collect_argument_accesses(&mut collection, *argument);
        }
    }

    accesses
}

fn collect_argument_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) {
    match collection.program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            if let Some(access_place) = collection.borrow_access_place(*inner_expression) {
                collection.append_argument_access(access_place, BorrowAccessKind::Mutable);
            }
        }
        _ => collect_read_accesses(collection, expression),
    }
}

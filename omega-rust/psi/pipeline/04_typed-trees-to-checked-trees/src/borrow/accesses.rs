//! The places a call's arguments access. `collect_call_argument_accesses`
//! appends one `BorrowArgumentAccessFact` per accessed place and returns the
//! call's span of them: a `&`, `&mut` or `&write` argument records a read,
//! mutable or write-only access to its borrowed place, and any other argument
//! records a read access for each place it reads (`read`). `borrow::calls`
//! calls it while building borrow facts, and `checks::borrows::calls::receiver`
//! calls it into scratch arenas for the operand that follows an exclusive
//! operation's receiver.
//!
//! `borrow_access_place` (`place`) resolves an expression to the place it
//! accesses, with `contextual` resolving names and members in the state and
//! machine; the borrow checks use it too. `collection` holds the arenas one
//! call appends to, and `records` writes one access row.

use crate::checked_trees::expression::{ExpressionHandle, ExpressionNode};
use crate::checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};
use symbols::SymbolHandle;
mod collection;
mod contextual;
mod place;
mod read;
mod records;

use collection::BorrowAccessCollection;
pub(crate) use place::{BorrowAccessPlace, borrow_access_place};
use read::collect_read_accesses;

pub(crate) fn collect_call_argument_accesses(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    access_segments: &mut arena::Arena<crate::fact_plan::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
) -> arena::HandleSpan<BorrowArgumentAccessFact> {
    let mut accesses = arena::HandleSpan::empty();

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
        ExpressionNode::Borrow(inner_expression) => {
            if let Some(access_place) = collection.borrow_access_place(inner_expression.target) {
                let kind = match inner_expression.access {
                    language_semantics::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
                    language_semantics::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
                    language_semantics::ReferenceAccess::Shared => BorrowAccessKind::Read,
                };
                collection.append_argument_access(access_place, kind);
            }
        }
        _ => collect_read_accesses(collection, expression),
    }
}

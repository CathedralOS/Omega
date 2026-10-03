//! The places a call's arguments access. `collect_call_argument_accesses`
//! appends one `BorrowArgumentAccessFact` per accessed place and returns the
//! call's span of them: a `&`, `&mut` or `&write` argument records a read,
//! mutable or write-only access to its borrowed place, and any other argument
//! records a read access for each place it reads (`read`).
//! Forwarded reference bindings retain the selected formal's loan access;
//! a reference name is not a copied read of its referent.
//! Retained rows also distinguish copied observations and nested-call loans
//! that finish during operand evaluation from loans held through invocation.
//! `borrow::calls` calls it while building borrow facts, and `checks::borrows::calls::receiver`
//! calls it into scratch arenas for the operand that follows an exclusive
//! operation's receiver. A transition arm's target roster comes from
//! `collect_transition_target_argument_accesses`, which also ends the reads of
//! payloads its dispatch already copied (`dispatch_extraction`).
//!
//! `borrow_access_place` (`place`) resolves an expression to the place it
//! accesses, with `contextual` resolving names and members in the state and
//! machine; the borrow checks use it too. `collection` holds the arenas one
//! call appends to, and `records` writes one access row.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};
use symbols::SymbolHandle;
mod collection;
mod contextual;
mod dispatch_extraction;
mod place;
mod read;
mod records;
mod replay;
pub(crate) use replay::rejoin_call_accesses;

use collection::BorrowAccessCollection;
pub(crate) use place::{BorrowAccessPlace, borrow_access_place};
use read::collect_read_accesses;

pub(crate) fn collect_call_argument_accesses(
    program: &typed_trees::TypedTrees,
    access_segments: &mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
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

        collect_target_argument_accesses(&mut collection, arguments, target_symbol);
    }

    accesses
}

/// A transition arm's own target roster. Its plain-copy payload bindings were
/// copied out of the dispatch's saved subject before the target evaluates any
/// argument; nested calls' rosters keep `collect_call_argument_accesses`.
pub(crate) fn collect_transition_target_argument_accesses(
    program: &typed_trees::TypedTrees,
    access_segments: &mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
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
        )
        .after_dispatch_extraction();

        collect_target_argument_accesses(&mut collection, arguments, target_symbol);
    }

    accesses
}

/// Nested and outer invocations retain the same formal-directed loan access.
fn collect_target_argument_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    arguments: &[ExpressionHandle],
    target_symbol: SymbolHandle,
) {
    let program = collection.program;
    let mut parameters = crate::semantic::calls::call_target_parameters(program, target_symbol)
        .unwrap_or_default()
        .iter()
        .filter(|parameter| !parameter.is_self);
    for argument in arguments {
        let parameter = parameters.next();
        // Forwarding a reference binding retains a loan through the
        // invocation just like an authored borrow. The selected formal
        // can attenuate a mutable reference to shared observation.
        let forwarded_access = parameter.and_then(|parameter| {
            if matches!(
                program.expression_table.expression(*argument),
                ExpressionNode::Borrow(_)
            ) {
                return None;
            }
            let reference = collection.expression_type_reference(*argument)?;
            validation::reference_shell(program, reference)?;
            let formal = validation::reference_shell(program, parameter.type_reference)?;
            Some(match formal.access {
                language_semantics::ReferenceAccess::Shared => BorrowAccessKind::Read,
                language_semantics::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
                language_semantics::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
            })
        });
        if let Some(kind) = forwarded_access
            && let Some(place) = collection.borrow_access_place(*argument)
        {
            read::collect_place_evaluation_reads(collection, *argument);
            collection.append_argument_access(place, kind);
            continue;
        }
        collect_argument_accesses(collection, *argument);
    }
}

fn collect_argument_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) {
    match collection.program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner_expression) => {
            read::collect_place_evaluation_reads(collection, inner_expression.target);
            if let Some(access_place) = collection.borrow_access_place(expression) {
                collection.append_argument_access(
                    access_place,
                    borrow_access_kind(inner_expression.access),
                );
            }
        }
        _ => collect_read_accesses(collection, expression),
    }
}

fn borrow_access_kind(access: language_semantics::ReferenceAccess) -> BorrowAccessKind {
    match access {
        language_semantics::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
        language_semantics::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
        language_semantics::ReferenceAccess::Shared => BorrowAccessKind::Read,
    }
}

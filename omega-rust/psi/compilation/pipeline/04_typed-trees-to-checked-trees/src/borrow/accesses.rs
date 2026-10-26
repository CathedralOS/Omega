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
//! payloads its dispatch already copied (`validation::DispatchExtraction`).
//! Each row retains the operand that produced it. The statement read judgment
//! leaves a call's staged operands to that call's own judgment without a
//! separate expression-recording mode or a parallel provenance walk.
//!
//! `borrow_access_place` (`place`) resolves an expression to the place it
//! accesses, with `contextual` resolving names and members in the state and
//! machine; the borrow checks use it too. `collection` holds the arenas one
//! call appends to, and `records` writes one access row.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};
use symbols::SymbolHandle;
mod collection;
mod completed_dispatch;
mod place;
mod read;
mod records;
mod replay;
pub(crate) use completed_dispatch::retain_completed_dispatch_extractions;
pub(crate) use replay::rejoin_expression_call_accesses;

use collection::BorrowAccessCollection;
pub(crate) use place::{
    BorrowAccessGeometry, BorrowAccessPlace, borrow_access_geometry, borrow_access_place,
    is_borrowed_parameter_root, is_image_storage_root,
};
use read::collect_read_accesses;

pub(crate) fn collect_call_argument_accesses(
    program: &dyn ::validation::CheckingInput,
    access_segments: &mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
) -> arena::HandleSpan<BorrowArgumentAccessFact> {
    collect_argument_access_span(
        program,
        access_segments,
        argument_accesses,
        arguments,
        state_symbol,
        statement_index,
        machine_symbol,
        target_symbol,
        false,
        None,
    )
}

/// A transition arm's own target roster. Its plain-copy payload bindings were
/// copied out of the dispatch's saved subject before the target evaluates any
/// argument; nested calls' rosters keep `collect_call_argument_accesses`.
pub(crate) fn collect_transition_target_argument_accesses(
    program: &dyn ::validation::CheckingInput,
    access_segments: &mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
) -> arena::HandleSpan<BorrowArgumentAccessFact> {
    collect_argument_access_span(
        program,
        access_segments,
        argument_accesses,
        arguments,
        state_symbol,
        statement_index,
        machine_symbol,
        target_symbol,
        true,
        None,
    )
}

fn collect_completed_transition_target_argument_accesses(
    program: &dyn ::validation::CheckingInput,
    access_segments: &mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    extraction: validation::DispatchExtraction,
) -> arena::HandleSpan<BorrowArgumentAccessFact> {
    collect_argument_access_span(
        program,
        access_segments,
        argument_accesses,
        arguments,
        state_symbol,
        statement_index,
        machine_symbol,
        target_symbol,
        true,
        Some(extraction),
    )
}

/// The ordinary typed access schedule distinguishes value observations from
/// source-place loans. This query reuses that schedule when a selected arm
/// must complete its payload copies before the first target operand.
pub(crate) fn dispatch_target_value_reads(
    program: &dyn ::validation::CheckingInput,
    arguments: &[ExpressionHandle],
    state: SymbolHandle,
    statement: usize,
    machine: SymbolHandle,
    target: SymbolHandle,
    extraction: validation::DispatchExtraction,
) -> Vec<ExpressionHandle> {
    let mut segments = arena::Arena::new();
    let mut rows = arena::Arena::new();
    let accesses = collect_completed_transition_target_argument_accesses(
        program,
        &mut segments,
        &mut rows,
        arguments,
        state,
        statement,
        machine,
        target,
        extraction,
    );
    rows.span_or_empty(accesses)
        .iter()
        .filter(|access| {
            if access.kind != BorrowAccessKind::Read
                || matches!(
                    program.expression_table.expression(access.expression),
                    ExpressionNode::Borrow(_)
                )
            {
                return false;
            }
            // Ordinary copied reads name their own expression as the completed
            // read. A nested call instead names that invocation when it releases
            // its argument loans; that endpoint is not a copied argument value.
            let completed_value_read =
                access.end_before_access.is_some() && access.released_at == access.expression;
            // Forwarding a shared reference retains a loan through invocation, but
            // its stored descriptor is a value. Copy permission is checked by the
            // copy producer; this grants no read of its referee and no extra loan.
            let descriptor_read = validation::expression_type_reference_in_state(
                program,
                state,
                statement,
                access.expression,
            )
            .is_some_and(|reference| validation::reference_shell(program, reference).is_some());
            completed_value_read || descriptor_read
        })
        .map(|access| access.expression)
        .collect()
}

fn collect_argument_access_span(
    program: &dyn ::validation::CheckingInput,
    access_segments: &mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    after_dispatch: bool,
    completed_extraction: Option<validation::DispatchExtraction>,
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
        if after_dispatch {
            collection = collection.after_dispatch_extraction();
        }
        if let Some(extraction) = completed_extraction {
            collection = collection.with_completed_dispatch_extraction(extraction);
        }

        collect_target_argument_accesses(
            &mut collection,
            arguments,
            state_symbol,
            statement_index,
            target_symbol,
        );
    }

    accesses
}

/// Nested and outer invocations retain the same formal-directed loan access.
fn collect_target_argument_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    arguments: &[ExpressionHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    target_symbol: SymbolHandle,
) {
    let mut parameters = validation::call_target_parameters(collection.program, target_symbol)
        .unwrap_or_default()
        .iter();
    for argument in arguments {
        let parameter = parameters.next();
        collect_formal_argument_accesses(
            collection,
            *argument,
            parameter.map_or(Default::default(), |parameter| parameter.type_reference),
            state_symbol,
            statement_index,
        );
    }
}

/// A value join preserves its destination contract on each yielded value.
/// Evaluating an arm is not enough: a reference result still has to retain
/// the invocation's formal-directed access, including a computed call result.
/// Subjects and patterns remain ordinary reads, not reference arguments.
fn collect_formal_argument_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    argument: ExpressionHandle,
    formal: typed_trees::types::TypeReferenceHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
) {
    let program = collection.program;
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(argument) {
        collect_read_accesses(collection, dispatch.subject);
        for arm in program.expression_table.match_arms(dispatch.arms) {
            for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                collect_read_accesses(collection, pattern);
            }
            collect_formal_argument_accesses(
                collection,
                arm.value,
                formal,
                state_symbol,
                statement_index,
            );
        }
        return;
    }
    // Forwarding a reference binding retains a loan through the
    // invocation just like an authored borrow. The selected formal
    // can attenuate a mutable reference to shared observation.
    let forwarded_access = (|| {
        if !formal.is_valid()
            || matches!(
                program.expression_table.expression(argument),
                ExpressionNode::Borrow(_)
            )
        {
            return None;
        }
        let formal_reference = formal;
        let formal = validation::reference_shell(program, formal_reference)?;
        let reference = collection.expression_type_reference(argument)?;
        // A reference binding forwards its loan; a place lent as a whole
        // slice view does too. Its storage backs the formal's descriptor
        // through the invocation, so its read is the live borrow, not a
        // copied observation that ends during operand evaluation.
        if validation::reference_shell(program, reference).is_none()
            && !(validation::slice_view_shell(program, formal_reference, &[]).is_some()
                && crate::semantic::calls::view_argument_names_existing_place(program, argument))
        {
            return None;
        }
        Some(BorrowAccessKind::from(formal.access))
    })();
    if let Some(kind) = forwarded_access
        && let Some(place) = collection.borrow_access_place(argument)
    {
        let first_access = collection.access_count();
        read::collect_place_evaluation_reads(collection, argument);
        collection.append_argument_access(place, kind, argument);
        collection.bind_completed_argument(first_access);
        return;
    }
    // An unborrowed actual to a shared formal lends the storage it names:
    // the implicit view keeps that place's loan through invocation rather
    // than recording a copied observation that ends inside operand
    // evaluation (`shared_argument_referent`, `evaluated_argument_root`).
    let implicit_loan = (|| {
        if !formal.is_valid()
            || matches!(
                program.expression_table.expression(argument),
                ExpressionNode::Borrow(_)
            )
        {
            return None;
        }
        let (referent, _) =
            crate::semantic::calls::shared_argument_referent(program, argument, formal)?;
        validation::canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            referent,
        )?
        .root
        .as_symbol()?;
        Some(BorrowAccessKind::Read)
    })();
    if let Some(kind) = implicit_loan
        && let Some(place) = collection.borrow_access_place(argument)
    {
        read::collect_place_evaluation_reads(collection, argument);
        collection.append_argument_access(place, kind, argument);
        return;
    }
    collect_argument_accesses(collection, argument);
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
                    BorrowAccessKind::from(inner_expression.access),
                    expression,
                );
            }
        }
        _ => collect_read_accesses(collection, expression),
    }
}

/// A completed value read can copy descriptor bytes without reading their
/// referee. This is independent of whether the value owns inline storage.
pub(crate) fn is_copyable_value_type(
    program: &dyn validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    validation::reference_result_custody::is_copyable_stored_value(program, reference)
}

/// A detached owned copy releases its source home, independently of its
/// contained loan frontier. The staged-value checker retains those leaves.
/// A reference descriptor value is copyable but does not own its referee.
pub(crate) fn is_owned_copy_type(
    program: &dyn validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    is_copyable_value_type(program, reference)
        && validation::reference_shell(program, reference).is_none()
        && program.type_multiplicity(reference) == language_semantics::Multiplicity::Unrestricted
}

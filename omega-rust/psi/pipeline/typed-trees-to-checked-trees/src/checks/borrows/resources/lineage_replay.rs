//! Replay of direct reborrow lineages and their expected parents.

use checked_trees::{BorrowFacts, BorrowLoanFact, BorrowLoanLineage};
use diagnostics::Diagnostic;

pub(crate) fn replay_checked_direct_reborrow_lineage(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
) -> Result<(), Vec<Diagnostic>> {
    for (_, state) in borrow.states.iter() {
        let Some(typed_state) = crate::semantic::calls::find_state_in_machine(
            program,
            state.machine_symbol,
            state.state_symbol,
        ) else {
            return Err(vec![Diagnostic::error(
                "checked borrow loan lineage has no exact typed state owner",
            )]);
        };
        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            let expected =
                expected_loan_lineage(program, typed_state, borrow, state, loan_handle, loan);
            if loan.lineage != expected {
                return Err(vec![Diagnostic::error(
                    "checked borrow loan lineage drifted from independent direct-reborrow replay",
                )]);
            }
        }
    }
    Ok(())
}

fn expected_loan_lineage(
    program: &typed_trees::TypedTrees,
    typed_state: &typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &checked_trees::StateBorrowFact,
    loan_handle: arena::Handle<BorrowLoanFact>,
    loan: &BorrowLoanFact,
) -> BorrowLoanLineage {
    let Some(statement) = program
        .statement_table
        .statements(typed_state.statement_nodes)
        .get(loan.statement_index)
    else {
        return if loan.source_owner_symbol.is_valid() {
            BorrowLoanLineage::UnretainedDerived
        } else {
            BorrowLoanLineage::DirectRoot
        };
    };
    let checked_trees::statement::StatementNode::LocalData(local) = statement else {
        if let checked_trees::statement::StatementNode::Assignment(assignment) = statement {
            match program.expression_table.expression(assignment.value) {
                checked_trees::expression::ExpressionNode::Call(call) => {
                    let target_is_reference = crate::flow::expression_type_reference_in_state(
                        program,
                        typed_state.symbol,
                        loan.statement_index,
                        assignment.target,
                    )
                    .is_some_and(|target| {
                        crate::borrow::view_link::is_reference_type(program, target)
                    });
                    return expected_call_result_lineage(
                        program,
                        typed_state,
                        state.machine_symbol,
                        target_is_reference,
                        call,
                        loan,
                    );
                }
                checked_trees::expression::ExpressionNode::Cast(_)
                | checked_trees::expression::ExpressionNode::ArrayLiteral(_)
                | checked_trees::expression::ExpressionNode::StructLiteral(_) => {
                    return BorrowLoanLineage::UnretainedDerived;
                }
                _ => {}
            }
        }
        return if loan.source_owner_symbol.is_valid() {
            BorrowLoanLineage::UnretainedDerived
        } else {
            BorrowLoanLineage::DirectRoot
        };
    };
    if local.symbol != loan.owner_symbol {
        return BorrowLoanLineage::UnretainedDerived;
    }

    match program.expression_table.expression(local.initial_value) {
        checked_trees::expression::ExpressionNode::Borrow(reborrow) => {
            expected_explicit_reborrow_parent(
                program,
                typed_state,
                borrow,
                state,
                loan_handle,
                loan,
                reborrow.target,
            )
            .map(|parent_loan| BorrowLoanLineage::Reborrow { parent_loan })
            .unwrap_or_else(|| {
                if loan.source_owner_symbol.is_valid() {
                    BorrowLoanLineage::UnretainedDerived
                } else {
                    BorrowLoanLineage::DirectRoot
                }
            })
        }
        checked_trees::expression::ExpressionNode::Call(call) => expected_call_result_lineage(
            program,
            typed_state,
            state.machine_symbol,
            crate::borrow::view_link::is_reference_type(program, local.type_reference),
            call,
            loan,
        ),
        checked_trees::expression::ExpressionNode::Cast(_)
        | checked_trees::expression::ExpressionNode::ArrayLiteral(_)
        | checked_trees::expression::ExpressionNode::StructLiteral(_) => {
            BorrowLoanLineage::UnretainedDerived
        }
        _ if loan.source_owner_symbol.is_valid() => BorrowLoanLineage::UnretainedDerived,
        _ => BorrowLoanLineage::DirectRoot,
    }
}

/// Expected lineage for a call-valued initializer or assignment value.
///
/// Formation promotes a call result's loan to `DirectRoot` only when the
/// call target's own declaration names one exact borrow source -- the self
/// receiver or a single direct reference parameter -- and that source place
/// resolves to storage carrying no live local loan. The name-matched
/// slice/view builtins, carrier-field sources (`Fields`), ambiguous or
/// view-free signatures, and any source rebased through a live local loan
/// all stay deliberately `UnretainedDerived`.
fn expected_call_result_lineage(
    program: &typed_trees::TypedTrees,
    typed_state: &typed_trees::state::State,
    machine_symbol: symbols::SymbolHandle,
    target_is_reference: bool,
    call: &checked_trees::expression::TableCallExpression,
    loan: &BorrowLoanFact,
) -> BorrowLoanLineage {
    if target_is_reference
        && !loan.source_owner_symbol.is_valid()
        && crate::borrow::call_declares_direct_view_source(program, call.target_symbol)
        && crate::borrow::helper_call_borrow_loan_place(
            program,
            typed_state.symbol,
            loan.statement_index,
            machine_symbol,
            call,
        )
        .is_some()
    {
        BorrowLoanLineage::DirectRoot
    } else {
        BorrowLoanLineage::UnretainedDerived
    }
}

#[allow(clippy::too_many_arguments)]
fn expected_explicit_reborrow_parent(
    program: &typed_trees::TypedTrees,
    typed_state: &typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &checked_trees::StateBorrowFact,
    child_handle: arena::Handle<BorrowLoanFact>,
    child: &BorrowLoanFact,
    source_expression: checked_trees::expression::ExpressionHandle,
) -> Option<arena::Handle<BorrowLoanFact>> {
    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        typed_state.symbol,
        child.statement_index,
        source_expression,
    )?;
    let facts::PlaceRoot::Symbol(source_root) = source.root else {
        return None;
    };
    let mut candidates = borrow.loans.iter().filter(|(parent_handle, parent)| {
        *parent_handle != child_handle
            && borrow.state_owns_loan(state, *parent_handle)
            && parent.statement_index < child.statement_index
            && parent.lineage != BorrowLoanLineage::UnretainedDerived
            // A call-result root is retained for receiver resolution but is
            // not borrow ancestry: formation never gives its children
            // `Reborrow` lineage, so replay must not either.
            && !loan_is_call_result_root(program, typed_state, parent)
            && parent.owner_symbol == source_root
            && owner_path_matches_source(program, borrow.loan_owner_path(parent), &source.segments)
            && child.source_owner_symbol == parent.owner_symbol
    });
    let (parent_handle, parent) = candidates.next()?;
    if candidates.next().is_some()
        || !child_place_replays_from_parent(borrow, parent, &source.segments, child)
    {
        return None;
    }
    Some(parent_handle)
}

/// True when the loan's retained root was captured from a call result rather
/// than a borrow expression. Mirrors the `call_result` marker formation
/// records on `StatementBorrowLoan`: a `LocalData` initializer or an
/// `Assignment` value that is a call. Non-reference call carriers stay
/// `UnretainedDerived` during formation, so any retained loan reaching this
/// check through a call-valued statement is exactly a promoted call root.
fn loan_is_call_result_root(
    program: &typed_trees::TypedTrees,
    typed_state: &typed_trees::state::State,
    loan: &BorrowLoanFact,
) -> bool {
    let Some(statement) = program
        .statement_table
        .statements(typed_state.statement_nodes)
        .get(loan.statement_index)
    else {
        return false;
    };
    match statement {
        checked_trees::statement::StatementNode::LocalData(local) => {
            local.symbol == loan.owner_symbol
                && matches!(
                    program.expression_table.expression(local.initial_value),
                    checked_trees::expression::ExpressionNode::Call(_)
                )
        }
        checked_trees::statement::StatementNode::Assignment(assignment) => matches!(
            program.expression_table.expression(assignment.value),
            checked_trees::expression::ExpressionNode::Call(_)
        ),
        _ => false,
    }
}

fn child_place_replays_from_parent(
    borrow: &BorrowFacts,
    parent: &BorrowLoanFact,
    source_segments: &[facts::PlaceSegment],
    child: &BorrowLoanFact,
) -> bool {
    let parent_owner_path = borrow.loan_owner_path(parent);
    let Some(remainder) = source_segments.get(parent_owner_path.len()..) else {
        return false;
    };
    child.root_symbol == parent.root_symbol
        && borrow.loan_segments(child).len() == borrow.loan_segments(parent).len() + remainder.len()
        && borrow
            .loan_segments(child)
            .iter()
            .eq(borrow.loan_segments(parent).iter().chain(remainder))
}

fn owner_path_matches_source(
    program: &typed_trees::TypedTrees,
    owner_path: &[checked_trees::BorrowLoanOwnerSegment],
    source_segments: &[facts::PlaceSegment],
) -> bool {
    owner_path.len() <= source_segments.len()
        && owner_path
            .iter()
            .zip(source_segments)
            .all(|(owner, source)| match (owner, source) {
                (
                    checked_trees::BorrowLoanOwnerSegment::Field(owner_symbol),
                    facts::PlaceSegment::Field {
                        symbol: source_symbol,
                    },
                ) => !source_symbol.is_valid() || owner_symbol == source_symbol,
                (
                    checked_trees::BorrowLoanOwnerSegment::Case(owner_variant),
                    facts::PlaceSegment::Case {
                        variant: source_variant,
                    },
                ) => owner_variant == source_variant,
                (
                    checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::FixedIndex {
                        index: source_index,
                    },
                ) => owner_index == source_index,
                (
                    checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|source_index| *owner_index == source_index),
                (
                    checked_trees::BorrowLoanOwnerSegment::DynamicIndex,
                    facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::FixedRange { .. }
                    | facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}

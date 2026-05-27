use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::{BorrowAccessKind, BorrowCallFact, CheckFacts, FlowStateFact};
use omega_core::diagnostics::Diagnostic;

use crate::flow::statement_mutated_place;
use crate::labels::{
    borrow_access_label, call_target_label, canonical_place_label_from_parts, symbol_name,
};
use crate::semantic_calls::{
    call_site_argument_expressions, find_call_site, find_state_in_machine,
};

pub(crate) fn check_flow_call_borrows(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.states.iter() {
        let Some(borrow_state) = facts.borrow.states.iter().find_map(|(_, state)| {
            (state.machine_symbol == state_flow.machine_symbol
                && state.state_symbol == state_flow.state_symbol)
                .then_some(state)
        }) else {
            continue;
        };

        for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
            check_call_borrows(program, facts, state_flow, borrow_call, &mut diagnostics);
        }

        check_statement_borrows(program, facts, state_flow, &mut diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_statement_borrows(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(state) =
        find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return;
    };
    let Some(borrow_state) = facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    }) else {
        return;
    };

    for statement in facts.flow.statements.span_or_empty(state_flow.statements) {
        let Some(statement_node) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement.statement_index)
        else {
            continue;
        };

        for loan in facts
            .borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .iter()
            .filter(|loan| loan.statement_index == statement.statement_index)
        {
            for active_loan_handle in facts
                .flow
                .borrow_loan_constraints(statement.entry_constraints)
            {
                let active_loan = facts.borrow.loans.get(active_loan_handle);
                if loan.kind == BorrowAccessKind::Read && active_loan.kind == BorrowAccessKind::Read
                {
                    continue;
                }

                if !borrow_loan_overlaps_loan(program, facts, loan, active_loan) {
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "statement {} creates local borrow `{}` while local borrow `{}` is still active ({})",
                    statement.statement_index,
                    symbol_name(program, loan.owner_symbol),
                    symbol_name(program, active_loan.owner_symbol),
                    active_loan_detail(
                        state_flow,
                        facts,
                        active_loan_handle,
                        statement.statement_index,
                    )
                    .unwrap_or_else(|| format!("borrowed at statement {}", active_loan.statement_index)),
                )));
            }
        }

        let Some(mutated_place) = statement_mutated_place(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            statement.statement_index,
            statement_node,
        ) else {
            continue;
        };

        for loan_handle in facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
        {
            let loan = facts.borrow.loans.get(loan_handle);
            if canonical_place_overlaps_loan(program, &mutated_place, loan, &facts.borrow) {
                diagnostics.push(Diagnostic::error(format!(
                    "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                    statement.statement_index,
                    canonical_place_label(program, &mutated_place),
                    symbol_name(program, loan.owner_symbol),
                    active_loan_detail(state_flow, facts, loan_handle, statement.statement_index)
                        .unwrap_or_else(|| format!(
                            "borrowed at statement {}",
                            loan.statement_index
                        )),
                )));
            }
        }
    }
}

fn check_call_borrows(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target_name = call_target_label(program, borrow_call.target_symbol);
    let entry_constraints = call_borrow_constraints(borrow_call, state_flow, facts);
    let writable_roots: Vec<_> = facts
        .flow
        .borrow_writable_root_constraints(entry_constraints)
        .map(|root| symbol_name(program, facts.borrow.writable_roots.get(root).symbol))
        .collect();
    let accesses: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .collect();
    let active_loans: Vec<_> = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .map(|loan| (loan, facts.borrow.loans.get(loan)))
        .collect();

    for (index, access) in accesses.iter().enumerate() {
        if access.kind != BorrowAccessKind::Mutable {
            continue;
        }

        for other_access in accesses.iter().skip(index + 1) {
            if !borrow_accesses_overlap(program, facts, access, other_access) {
                continue;
            }

            match other_access.kind {
                BorrowAccessKind::Mutable => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as mutable more than once",
                    borrow_access_label(program, &facts.borrow, access),
                ))),
                BorrowAccessKind::Read => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as both mutable and read-only",
                    borrow_access_label(program, &facts.borrow, access),
                ))),
            }
        }

        for (loan_handle, loan) in &active_loans {
            if borrow_access_overlaps_loan(program, facts, access, loan) {
                let detail = active_loan_detail(
                    state_flow,
                    facts,
                    *loan_handle,
                    borrow_call.statement_index,
                );
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` while local borrow `{}` is still active{}",
                    borrow_access_label(program, &facts.borrow, access),
                    symbol_name(program, loan.owner_symbol),
                    detail
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default(),
                )));
            }
        }
    }

    let Some(call_site) = find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };

    for argument in call_site_argument_expressions(program, &call_site) {
        let omega_checked_trees::expression::ExpressionNode::Mutable(inner_expression) =
            program.expression_table.expression(*argument)
        else {
            continue;
        };

        let Some(root_name) = mutable_argument_root_name(program, *inner_expression) else {
            diagnostics.push(Diagnostic::error(format!(
                "mutable argument for state `{target_name}` must be a named place"
            )));
            continue;
        };

        if !writable_roots.contains(&root_name) {
            diagnostics.push(Diagnostic::error(format!(
                "mutable argument `{root_name}` for state `{target_name}` is not writable in this state"
            )));
        }
    }
}

fn call_borrow_constraints<'a>(
    borrow_call: &BorrowCallFact,
    state_flow: &'a FlowStateFact,
    facts: &'a CheckFacts,
) -> omega_core::arena::HandleSpan<omega_checked_trees::FlowConstraintRef> {
    facts.flow.state_call_entry_constraints(
        state_flow,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
        borrow_call.target_symbol,
        borrow_call.receiver_symbol,
    )
}

fn mutable_argument_root_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        omega_checked_trees::expression::ExpressionNode::Indexed(indexed) => {
            mutable_argument_root_name(program, indexed.collection)
        }
        omega_checked_trees::expression::ExpressionNode::Range(_) => None,
        omega_checked_trees::expression::ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                omega_checked_trees::expression::ExpressionNode::Name(path) => {
                    let members = program.expression_table.name_path_members(path.members);
                    if members
                        .first()
                        .is_some_and(|member_name| member_name.as_str() == "self")
                    {
                        return Some(member.member.as_str().to_owned());
                    }
                }
                _ => {}
            }
            mutable_argument_root_name(program, member.receiver)
        }
        omega_checked_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            mutable_argument_root_name(program, *inner_expression)
        }
        omega_checked_trees::expression::ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .map(|member| member.as_str().to_owned()),
        omega_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
        | omega_checked_trees::expression::ExpressionNode::Binary(_)
        | omega_checked_trees::expression::ExpressionNode::Boolean(_)
        | omega_checked_trees::expression::ExpressionNode::Call(_)
        | omega_checked_trees::expression::ExpressionNode::Cast(_)
        | omega_checked_trees::expression::ExpressionNode::Float(_)
        | omega_checked_trees::expression::ExpressionNode::Integer(_)
        | omega_checked_trees::expression::ExpressionNode::String(_)
        | omega_checked_trees::expression::ExpressionNode::StructLiteral(_) => None,
    }
}

fn active_loan_detail(
    state_flow: &FlowStateFact,
    facts: &CheckFacts,
    loan: omega_core::arena::Handle<omega_checked_trees::BorrowLoanFact>,
    statement_index: usize,
) -> Option<String> {
    facts.flow
        .borrow_weakenings
        .span_or_empty(state_flow.borrow_weakenings)
        .iter()
        .find(|weakening| weakening.loan == loan)
        .and_then(|weakening| {
            let loan = facts.borrow.loans.get(loan);
            match (weakening.reason, weakening.source) {
                (
                    omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    omega_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: weakening_statement,
                    },
                ) if weakening_statement > statement_index => Some(format!(
                    "borrowed at statement {}; its last use is at statement {}",
                    loan.statement_index, loan.last_use_statement_index
                )),
                (
                    omega_checked_trees::FlowBorrowWeakeningReason::StateExit,
                    omega_checked_trees::FlowInvalidationSource::Statement { .. },
                ) if loan.last_use_statement_index > statement_index => Some(format!(
                    "borrowed at statement {}; its last use is at statement {} and it is released at state exit",
                    loan.statement_index, loan.last_use_statement_index
                )),
                (
                    omega_checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    omega_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: weakening_statement,
                    },
                ) if weakening_statement > statement_index => Some(format!(
                    "borrowed at statement {}; it is reassigned at statement {}",
                    loan.statement_index, weakening_statement
                )),
                (
                    omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    omega_checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    omega_checked_trees::FlowBorrowWeakeningReason::StateExit,
                    omega_checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    omega_checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    omega_checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    omega_checked_trees::FlowInvalidationSource::Call { .. },
                )
                | (
                    omega_checked_trees::FlowBorrowWeakeningReason::StateExit,
                    omega_checked_trees::FlowInvalidationSource::Call { .. },
                )
                | (
                    omega_checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    omega_checked_trees::FlowInvalidationSource::Call { .. },
                ) => None,
            }
        })
}

fn canonical_place_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &omega_checked_trees::BorrowLoanFact,
    borrow: &omega_checked_trees::BorrowFacts,
) -> bool {
    match place.root {
        omega_facts::PlaceRoot::Symbol(symbol) => {
            if symbol == loan.root_symbol {
                return place_segments_may_overlap(
                    program,
                    &place.segments,
                    borrow.loan_segments(loan),
                );
            }

            match place.segments.split_first() {
                Some((
                    omega_facts::PlaceSegment::Field {
                        symbol: field_symbol,
                    },
                    remaining,
                )) if *field_symbol == loan.root_symbol => {
                    place_segments_may_overlap(program, remaining, borrow.loan_segments(loan))
                }
                _ => false,
            }
        }
        omega_facts::PlaceRoot::Unknown
        | omega_facts::PlaceRoot::Expression(_)
        | omega_facts::PlaceRoot::TypeReference(_) => false,
    }
}

fn borrow_accesses_overlap(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    left: &omega_checked_trees::BorrowArgumentAccessFact,
    right: &omega_checked_trees::BorrowArgumentAccessFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(left),
            facts.borrow.access_segments(right),
        )
}

fn borrow_access_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    access: &omega_checked_trees::BorrowArgumentAccessFact,
    loan: &omega_checked_trees::BorrowLoanFact,
) -> bool {
    access.root_symbol == loan.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(access),
            facts.borrow.loan_segments(loan),
        )
}

fn borrow_loan_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    left: &omega_checked_trees::BorrowLoanFact,
    right: &omega_checked_trees::BorrowLoanFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.loan_segments(left),
            facts.borrow.loan_segments(right),
        )
}

fn place_segments_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| {
            place_segment_pair_may_overlap(program, *left_segment, *right_segment)
        })
}

fn place_segment_pair_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: omega_facts::PlaceSegment,
    right: omega_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            omega_facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            omega_facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            omega_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            omega_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => index_expressions_may_overlap(program, left_expression, right_expression),
        _ => false,
    }
}

fn index_expressions_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (
            omega_checked_trees::expression::ExpressionNode::Integer(left_value),
            omega_checked_trees::expression::ExpressionNode::Integer(right_value),
        ) => left_value == right_value,
        (
            omega_checked_trees::expression::ExpressionNode::Range(left_range),
            omega_checked_trees::expression::ExpressionNode::Integer(right_value),
        ) => range_may_contain_integer(program, left_range, *right_value),
        (
            omega_checked_trees::expression::ExpressionNode::Integer(left_value),
            omega_checked_trees::expression::ExpressionNode::Range(right_range),
        ) => range_may_contain_integer(program, right_range, *left_value),
        (
            omega_checked_trees::expression::ExpressionNode::Range(left_range),
            omega_checked_trees::expression::ExpressionNode::Range(right_range),
        ) => ranges_may_overlap(program, left_range, right_range),
        _ => true,
    }
}

fn range_may_contain_integer(
    program: &omega_typed_trees::TypedTrees,
    range: &omega_checked_trees::expression::TableRangeExpression,
    value: i64,
) -> bool {
    let (start, end) = range_integer_bounds(program, range);
    if start.is_some_and(|start| value < start) {
        return false;
    }
    if end.is_some_and(|end| value >= end) {
        return false;
    }
    true
}

fn ranges_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: &omega_checked_trees::expression::TableRangeExpression,
    right: &omega_checked_trees::expression::TableRangeExpression,
) -> bool {
    let (left_start, left_end) = range_integer_bounds(program, left);
    let (right_start, right_end) = range_integer_bounds(program, right);

    if let (Some(left_end), Some(right_start)) = (left_end, right_start)
        && left_end <= right_start
    {
        return false;
    }
    if let (Some(right_end), Some(left_start)) = (right_end, left_start)
        && right_end <= left_start
    {
        return false;
    }
    true
}

fn range_integer_bounds(
    program: &omega_typed_trees::TypedTrees,
    range: &omega_checked_trees::expression::TableRangeExpression,
) -> (Option<i64>, Option<i64>) {
    (
        integer_expression_value(program, range.start),
        integer_expression_value(program, range.end),
    )
}

fn integer_expression_value(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<i64> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        omega_checked_trees::expression::ExpressionNode::Integer(value) => Some(*value),
        _ => None,
    }
}

fn canonical_place_label(
    program: &omega_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
) -> String {
    canonical_place_label_from_parts(program, place.root, &place.segments)
}

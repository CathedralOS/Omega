use psi_checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, CheckFacts, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation, FlowFacts,
    FlowInvalidationSource,
};
use psi_diagnostics::Diagnostic;

/// Populate the checked-only direct-root resource closure before ordinary
/// checked-fact replay. Reborrow parent identity is replayed here too, but this
/// resource carrier remains deliberately absent for reborrows and
/// borrow-carrying transfers until their complete lifetime/restoration closure
/// is represented.
pub(super) fn initialize_checked_direct_borrow_resources(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let resources = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    facts.borrow.direct_loan_resources.reset_retain_capacity();
    facts.borrow.direct_loan_resources.insert_many(resources);
    Ok(())
}

/// Independently replay every retained direct-root resource from the
/// authoritative loan and flow-lifetime ledgers, then rebuild it
/// deterministically. The row itself never participates in borrow admission.
pub(super) fn replay_checked_direct_borrow_resources(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let expected = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    let retained = facts
        .borrow
        .direct_loan_resources
        .iter()
        .map(|(_, resource)| resource.clone())
        .collect::<Vec<_>>();
    if retained != expected {
        return Err(vec![Diagnostic::error(
            "checked direct-root borrow resource closure drifted from independent replay",
        )]);
    }

    facts.borrow.direct_loan_resources.reset_retain_capacity();
    facts.borrow.direct_loan_resources.insert_many(expected);
    Ok(())
}

fn reconstruct_direct_borrow_resources(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedDirectBorrowLoanResource>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-root borrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            // Parent identity is now retained for the narrow direct-reborrow
            // case, but its complete lifetime/restoration resource is not.
            // Every derived occurrence therefore remains outside this first
            // direct-root closure.
            if loan.lineage != BorrowLoanLineage::DirectRoot {
                continue;
            }

            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .filter(|activation| activation.loan == loan_handle)
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .filter(|weakening| weakening.loan == loan_handle)
                .collect::<Vec<_>>();
            let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice())
            else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow resource requires exactly one activation and one weakening",
                ));
                continue;
            };
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow activation drifted from loan formation",
                ));
                continue;
            }

            let parent_lifetime = CheckedDirectBorrowParentLifetime {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                root_symbol: loan.root_symbol,
            };
            let restoration = CheckedDirectBorrowRestorationObligation {
                parent: parent_lifetime.clone(),
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
            };
            resources.push(CheckedDirectBorrowLoanResource {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: psi_checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
                    segments: borrow.loan_segments(loan).to_vec(),
                },
                access: loan.kind.clone(),
                activation_source: activation.source,
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
                parent_lifetime,
                restoration,
            });
        }
    }

    if diagnostics.is_empty() {
        Ok(resources)
    } else {
        Err(diagnostics)
    }
}

fn replay_checked_direct_reborrow_lineage(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
) -> Result<(), Vec<Diagnostic>> {
    for (_, state) in borrow.states.iter() {
        let Some(typed_state) = crate::semantic_calls::find_state_in_machine(
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
    program: &psi_typed_trees::TypedTrees,
    typed_state: &psi_typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &psi_checked_trees::StateBorrowFact,
    loan_handle: psi_arena::Handle<BorrowLoanFact>,
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
    let psi_checked_trees::statement::StatementNode::LocalData(local) = statement else {
        if let psi_checked_trees::statement::StatementNode::Assignment(assignment) = statement
            && matches!(
                program.expression_table.expression(assignment.value),
                psi_checked_trees::expression::ExpressionNode::Call(_)
                    | psi_checked_trees::expression::ExpressionNode::Cast(_)
                    | psi_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
                    | psi_checked_trees::expression::ExpressionNode::StructLiteral(_)
            )
        {
            return BorrowLoanLineage::UnretainedDerived;
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
        psi_checked_trees::expression::ExpressionNode::Borrow(reborrow) => {
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
        psi_checked_trees::expression::ExpressionNode::Call(_)
        | psi_checked_trees::expression::ExpressionNode::Cast(_)
        | psi_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
        | psi_checked_trees::expression::ExpressionNode::StructLiteral(_) => {
            BorrowLoanLineage::UnretainedDerived
        }
        _ if loan.source_owner_symbol.is_valid() => BorrowLoanLineage::UnretainedDerived,
        _ => BorrowLoanLineage::DirectRoot,
    }
}

#[allow(clippy::too_many_arguments)]
fn expected_explicit_reborrow_parent(
    program: &psi_typed_trees::TypedTrees,
    typed_state: &psi_typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &psi_checked_trees::StateBorrowFact,
    child_handle: psi_arena::Handle<BorrowLoanFact>,
    child: &BorrowLoanFact,
    source_expression: psi_checked_trees::expression::ExpressionHandle,
) -> Option<psi_arena::Handle<BorrowLoanFact>> {
    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        typed_state.symbol,
        child.statement_index,
        source_expression,
    )?;
    let psi_facts::PlaceRoot::Symbol(source_root) = source.root else {
        return None;
    };
    let mut candidates = borrow
        .loans
        .iter()
        .filter(|(parent_handle, parent)| {
            *parent_handle != child_handle
                && borrow.state_owns_loan(state, *parent_handle)
                && parent.statement_index < child.statement_index
                && parent.lineage != BorrowLoanLineage::UnretainedDerived
                && parent.owner_symbol == source_root
                && owner_path_matches_source(
                    program,
                    borrow.loan_owner_path(parent),
                    &source.segments,
                )
                && child.source_owner_symbol == parent.owner_symbol
        })
        .map(|(handle, parent)| (handle, parent));
    let (parent_handle, parent) = candidates.next()?;
    if candidates.next().is_some()
        || !child_place_replays_from_parent(borrow, parent, &source.segments, child)
    {
        return None;
    }
    Some(parent_handle)
}

fn child_place_replays_from_parent(
    borrow: &BorrowFacts,
    parent: &BorrowLoanFact,
    source_segments: &[psi_facts::PlaceSegment],
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
    program: &psi_typed_trees::TypedTrees,
    owner_path: &[psi_checked_trees::BorrowLoanOwnerSegment],
    source_segments: &[psi_facts::PlaceSegment],
) -> bool {
    owner_path.len() <= source_segments.len()
        && owner_path
            .iter()
            .zip(source_segments)
            .all(|(owner, source)| match (owner, source) {
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Field(owner_symbol),
                    psi_facts::PlaceSegment::Field {
                        symbol: source_symbol,
                    },
                ) => !source_symbol.is_valid() || owner_symbol == source_symbol,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Case(owner_variant),
                    psi_facts::PlaceSegment::Case {
                        variant: source_variant,
                    },
                ) => owner_variant == source_variant,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::FixedIndex {
                        index: source_index,
                    },
                ) => owner_index == source_index,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|source_index| *owner_index == source_index),
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::DynamicIndex,
                    psi_facts::PlaceSegment::FixedIndex { .. }
                    | psi_facts::PlaceSegment::FixedRange { .. }
                    | psi_facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}

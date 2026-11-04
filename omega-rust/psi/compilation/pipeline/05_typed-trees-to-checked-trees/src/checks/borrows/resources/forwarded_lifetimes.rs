//! Replacing a reference slot must not end access held by an earlier copy.
//! Keep the original borrow held until all surviving copies finish. Lexical
//! slot retirement remains separate from the final holder's resource closure.

use arena::Handle;
use checked_trees::{
    BorrowLoanFact, CheckFacts, CheckedBorrowResourceRetention, FlowBorrowWeakeningFact,
};
use diagnostics::Diagnostic;

use super::lifecycle::{activation_boundary, exact_loan_lifecycle_rows, weakening_event_boundary};

#[derive(Default)]
struct TransferHolders {
    loan: Handle<BorrowLoanFact>,
    holders: Vec<Handle<BorrowLoanFact>>,
}

#[derive(Default)]
pub(super) struct LoanRetention {
    pub(super) loan: Handle<BorrowLoanFact>,
    pub(super) retention: CheckedBorrowResourceRetention,
}

/// Resolve each complete holder closure before installing resource lifetimes.
/// A holder must capture its supplier before that descriptor's lexical death;
/// a later view cannot retrospectively establish its own parent's permission.
pub(super) fn plan(facts: &mut CheckFacts) -> Result<Vec<LoanRetention>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    let mut holders_arena = arena::Arena::new();
    // Build direct forwarding edges once. Dense slots are lookup coordinates;
    // the complete generational handle still authenticates every row.
    let mut transfers: Vec<TransferHolders> = Vec::new();
    for (_, state) in facts.borrow.states.iter() {
        for (holder, _) in facts.borrow.state_loans(state) {
            let row = facts.borrow.loans.get(holder);
            let suppliers =
                crate::checks::borrows::transport_suppliers(&facts.borrow, state, holder).or_else(
                    || {
                        // A debt copy holds the same declared input window. It
                        // supplies no pointer or borrowing parent of its own.
                        let sources = facts.borrow.loan_suppliers.span(row.supplying_loans)?;
                        (row.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                            && !sources.is_empty()
                            && sources.iter().all(|source| {
                                facts.borrow.state_owns_loan(state, *source)
                                    && facts.borrow.loans.get(*source).lineage
                                        == checked_trees::BorrowLoanLineage::LifetimeDependency
                                    && facts.borrow.loans.get(*source).owner_symbol
                                        == row.source_owner_symbol
                                    && facts.borrow.loans.get(*source).kind == row.kind
                            }))
                        .then_some(sources)
                    },
                );
            let Some(suppliers) = suppliers else {
                continue;
            };
            for supplier in suppliers {
                let position = supplier.arena_index() as usize;
                if transfers.len() <= position {
                    transfers.resize_with(position + 1, TransferHolders::default);
                }
                let entry = &mut transfers[position];
                entry.loan = *supplier;
                crate::shared::collections::push_unique(&mut entry.holders, holder);
            }
        }
    }
    // A normal helper's saved descriptor holds the original evaluated input,
    // including its complete parent restriction. These are lifetime edges;
    // they do not make an input or a debt the descriptor's spatial parent.
    for (_, held) in facts.borrow.stored_call_inputs.iter() {
        for input in facts
            .borrow
            .stored_call_input_resources
            .span_or_empty(held.resources)
        {
            let resource = facts.borrow.call_loan_resources.get(*input);
            let parents = match resource.source {
                checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => facts
                    .borrow
                    .call_loan_parents
                    .span_or_empty(parents)
                    .to_vec(),
            };
            for source in parents {
                let position = source.arena_index() as usize;
                if transfers.len() <= position {
                    transfers.resize_with(position + 1, TransferHolders::default);
                }
                let entry = &mut transfers[position];
                entry.loan = source;
                crate::shared::collections::push_unique(&mut entry.holders, held.loan);
            }
        }
    }
    let descendants = close_output_descendants(facts, &transfers)?;
    // A completed output keeps each captured input held for its complete debt
    // cohort. These edges extend lifetimes only, never spatial parentage.
    for (_, output) in facts.borrow.supplied_output_resources.iter() {
        for input in facts
            .borrow
            .supplied_output_inputs
            .span_or_empty(output.inputs)
        {
            let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                input.permission
            else {
                continue; // Entry namespaces have no caller-owned restoration interval.
            };
            let input = facts.borrow.call_loan_resources.get(resource);
            let mut sources = Vec::new();
            match input.source {
                checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                    sources.push(loan)
                }
                checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                    sources
                        .extend_from_slice(facts.borrow.call_loan_parents.span_or_empty(parents));
                }
            }
            for source in sources {
                let position = source.arena_index() as usize;
                if transfers.len() <= position {
                    transfers.resize_with(position + 1, TransferHolders::default);
                }
                let entry = &mut transfers[position];
                entry.loan = source;
                for debt in facts.borrow.loan_suppliers.span_or_empty(output.debts) {
                    crate::shared::collections::push_unique(&mut entry.holders, *debt);
                }
            }
        }
    }
    for (_, state) in facts.borrow.states.iter() {
        let Some(flow_state) = facts
            .flow
            .control
            .state(state.machine_symbol, state.state_symbol)
        else {
            return Err(vec![Diagnostic::error(
                "retained borrow has no flow-state owner",
            )]);
        };
        for (loan, row) in facts.borrow.state_loans(state) {
            if !matches!(
                row.lineage,
                checked_trees::BorrowLoanLineage::DirectRoot
                    | checked_trees::BorrowLoanLineage::Reborrow { .. }
                    | checked_trees::BorrowLoanLineage::SuppliedOutput
            ) {
                continue;
            }
            let lexical = lexical_end(facts, flow_state, loan)?;
            if lexical.is_none() && row.lineage != checked_trees::BorrowLoanLineage::SuppliedOutput
            {
                // Unevaluated formations have no permission to retain. Resource
                // construction reports any required missing lifecycle rows.
                continue;
            }
            if row.lineage == checked_trees::BorrowLoanLineage::SuppliedOutput
                && super::supplied_permissions::normal_call(&facts.borrow, &facts.flow, loan)
                    .is_none()
            {
                continue;
            }
            let mut holders = vec![loan];
            // Each holder's own end, used when it supplies a later copy.
            let mut holder_ends = vec![lexical];
            let mut cursor = 0;
            let mut final_weakening = lexical;
            while cursor < holders.len() {
                let supplier = holders[cursor];
                cursor += 1;
                let supplier_end = lexical_end(facts, flow_state, supplier)?
                    .map(|handle| facts.flow.borrow_lifetimes.weakenings.get(handle));
                let Some(transferred) = transfers
                    .get(supplier.arena_index() as usize)
                    .filter(|row| row.loan == supplier)
                else {
                    continue;
                };
                for candidate in transferred.holders.iter().copied() {
                    if holders.contains(&candidate) {
                        continue;
                    }
                    let start = holder_start(facts, flow_state, candidate)?;
                    let weakening =
                        holding_end(facts, flow_state, candidate, &transfers, &descendants)?;
                    if supplier_end.is_some_and(|end| {
                        weakening_event_boundary(end.source, end.reason)
                            .is_ok_and(|boundary| start >= boundary)
                    }) {
                        return Err(vec![Diagnostic::error(
                            "saved descriptor was not captured before its supplier retired",
                        )]);
                    }
                    if let Some(weakening) = weakening {
                        let end = facts.flow.borrow_lifetimes.weakenings.get(weakening);
                        let later = if let Some(previous) = final_weakening {
                            let previous = facts.flow.borrow_lifetimes.weakenings.get(previous);
                            weakening_event_boundary(end.source, end.reason)?
                                > weakening_event_boundary(previous.source, previous.reason)?
                        } else {
                            true
                        };
                        if later {
                            final_weakening = Some(weakening);
                        }
                    }
                    holder_ends.push(weakening);
                    holders.push(candidate);
                }
            }
            let position = loan.arena_index() as usize;
            if rows.len() <= position {
                rows.resize_with(position + 1, LoanRetention::default);
            }
            rows[position] = LoanRetention {
                loan,
                retention: CheckedBorrowResourceRetention {
                    lexical_weakening: lexical.unwrap_or_default(),
                    holders: holders_arena.insert_many(holders),
                    final_weakening: final_weakening.unwrap_or_default(),
                },
            };
        }
    }
    facts.borrow.resource_holders = holders_arena;
    Ok(rows)
}

/// The input hold is established by the completed call before any saved view
/// can be formed. Its end must cover every descendant, even when that child's
/// bookkeeping debt has no descriptor-forwarding edge of its own.
fn close_output_descendants(
    facts: &mut CheckFacts,
    transfers: &[TransferHolders],
) -> Result<Vec<TransferHolders>, Vec<Diagnostic>> {
    let mut children: Vec<TransferHolders> = Vec::new();
    for (child, row) in facts.borrow.loans.iter() {
        let checked_trees::BorrowLoanLineage::Reborrow { parent_loan } = row.lineage else {
            continue;
        };
        let position = parent_loan.arena_index() as usize;
        if children.len() <= position {
            children.resize_with(position + 1, TransferHolders::default);
        }
        let entry = &mut children[position];
        entry.loan = parent_loan;
        entry.holders.push(child);
    }
    // A later output can retain an earlier reference as an input lifetime.
    // This edge belongs only to this lifetime-closure graph, never to the
    // spatial parent graph or the descriptor-forwarding graph.
    for (_, output) in facts.borrow.supplied_output_resources.iter() {
        for input in facts
            .borrow
            .supplied_output_inputs
            .span_or_empty(output.inputs)
        {
            let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                input.permission
            else {
                continue;
            };
            let input = facts.borrow.call_loan_resources.get(resource);
            let sources = match input.source {
                checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => facts
                    .borrow
                    .call_loan_parents
                    .span_or_empty(parents)
                    .to_vec(),
            };
            for source in sources {
                let position = source.arena_index() as usize;
                if children.len() <= position {
                    children.resize_with(position + 1, TransferHolders::default);
                }
                let entry = &mut children[position];
                entry.loan = source;
                crate::shared::collections::push_unique(&mut entry.holders, output.loan);
            }
        }
    }
    let mut ends = Vec::new();
    for (handle, output) in facts.borrow.supplied_output_resources.iter() {
        let state = facts
            .flow
            .control
            .unique_state(output.machine_symbol, output.state_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "completed output closure has no unique flow state",
                )]
            })?;
        let (holders, holder_end) = closure_end(facts, state, output.loan, &[transfers])?;
        let (_, end) = closure_end(facts, state, output.loan, &[transfers, children.as_slice()])?;
        ends.push((
            handle,
            holders,
            holder_end.unwrap_or_default(),
            end.unwrap_or_default(),
        ));
    }
    for (handle, holders, holder_end, end) in ends {
        let holders = facts.borrow.loan_suppliers.insert_many(holders);
        let output = facts.borrow.supplied_output_resources.get_mut(handle);
        output.holders = holders;
        output.holder_weakening = holder_end;
        output.final_weakening = end;
    }
    Ok(children)
}

fn closure_end(
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    root: Handle<BorrowLoanFact>,
    graphs: &[&[TransferHolders]],
) -> Result<
    (
        Vec<Handle<BorrowLoanFact>>,
        Option<Handle<FlowBorrowWeakeningFact>>,
    ),
    Vec<Diagnostic>,
> {
    let (_, owner) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "completed output closure has no borrow owner",
            )]
        })?;
    let mut pending = vec![(root, false)];
    let mut completed = Vec::new();
    let mut active = Vec::new();
    let mut end = None;
    while let Some((loan, closing)) = pending.pop() {
        if completed.contains(&loan) {
            continue;
        }
        if closing {
            if active.pop() != Some(loan) {
                return Err(vec![Diagnostic::error(
                    "completed output closure order is unavailable",
                )]);
            }
            completed.push(loan);
            continue;
        }
        if active.contains(&loan) || !facts.borrow.state_owns_loan(owner, loan) {
            return Err(vec![Diagnostic::error(
                "completed output has cyclic or foreign lifetime dependents",
            )]);
        }
        active.push(loan);
        pending.push((loan, true));
        if let Some(candidate) = lexical_end(facts, state, loan)? {
            end = later_end(facts, end, candidate)?;
        }
        for graph in graphs {
            if let Some(edges) = graph
                .get(loan.arena_index() as usize)
                .filter(|entry| entry.loan == loan)
            {
                pending.extend(edges.holders.iter().rev().map(|child| (*child, false)));
            }
        }
    }
    Ok((completed, end))
}

fn holding_end(
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    loan: Handle<BorrowLoanFact>,
    transfers: &[TransferHolders],
    descendants: &[TransferHolders],
) -> Result<Option<Handle<FlowBorrowWeakeningFact>>, Vec<Diagnostic>> {
    let mut end = lexical_end(facts, state, loan)?;
    if facts.borrow.retained_call_inputs(loan).is_some() {
        let (_, complete_end) = closure_end(facts, state, loan, &[transfers, descendants])?;
        if let Some(candidate) = complete_end {
            end = later_end(facts, end, candidate)?;
        }
    }
    for (_, dependency) in facts
        .borrow
        .output_lifetime_dependencies
        .iter()
        .filter(|(_, row)| row.loan == loan)
    {
        let checked_trees::CheckedOutputLifetimeDependencySource::Completion { output } =
            dependency.source
        else {
            continue;
        };
        let output = facts.borrow.supplied_output_resources.get(output);
        if facts
            .flow
            .borrow_lifetimes
            .weakenings
            .is_valid(output.final_weakening)
        {
            end = later_end(facts, end, output.final_weakening)?;
        }
    }
    Ok(end)
}

fn later_end(
    facts: &CheckFacts,
    previous: Option<Handle<FlowBorrowWeakeningFact>>,
    candidate: Handle<FlowBorrowWeakeningFact>,
) -> Result<Option<Handle<FlowBorrowWeakeningFact>>, Vec<Diagnostic>> {
    let Some(previous) = previous else {
        return Ok(Some(candidate));
    };
    let left = facts.flow.borrow_lifetimes.weakenings.get(previous);
    let right = facts.flow.borrow_lifetimes.weakenings.get(candidate);
    Ok(Some(
        if weakening_event_boundary(right.source, right.reason)?
            > weakening_event_boundary(left.source, left.reason)?
        {
            candidate
        } else {
            previous
        },
    ))
}

fn lexical_end(
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    loan: Handle<BorrowLoanFact>,
) -> Result<Option<Handle<FlowBorrowWeakeningFact>>, Vec<Diagnostic>> {
    let rows = state
        .borrow_weakenings
        .handles()
        .filter(|handle| facts.flow.borrow_lifetimes.weakenings.get(*handle).loan == loan)
        .collect::<Vec<_>>();
    match rows.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(*one)),
        _ => Err(vec![Diagnostic::error(
            "retained descriptor has ambiguous lexical retirement",
        )]),
    }
}

fn holder_start(
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    loan: Handle<BorrowLoanFact>,
) -> Result<super::lifecycle::LifecycleBoundaryKey, Vec<Diagnostic>> {
    let row = facts.borrow.loans.get(loan);
    if row.established_by_call.is_valid() {
        let origin = facts.borrow.calls.get(row.established_by_call);
        let normal = state
            .calls
            .handles()
            .filter(|handle| {
                let actual = facts.flow.control.calls.get(*handle);
                actual.statement_index == origin.statement_index
                    && actual.call_ordinal == origin.call_ordinal
                    && actual.target_symbol == origin.target_symbol
                    && actual.authored_expression == origin.authored_expression
                    && facts
                        .flow
                        .borrow_loan_constraints(actual.exit_constraints)
                        .any(|active| active == loan)
            })
            .collect::<Vec<_>>();
        if normal.len() != 1 {
            return Err(vec![Diagnostic::error(
                "retained output holder has no exact normal completion",
            )]);
        }
        return Ok(super::lifecycle::normal_completion_boundary(
            origin.statement_index,
        ));
    }
    let (activation, _) = exact_loan_lifecycle_rows(&facts.borrow, &facts.flow, state, loan)
        .map_err(|error| vec![error])?;
    activation_boundary(
        facts
            .flow
            .borrow_lifetimes
            .activations
            .get(activation)
            .source,
    )
}

pub(super) fn get(
    rows: &[LoanRetention],
    loan: Handle<BorrowLoanFact>,
) -> Result<&CheckedBorrowResourceRetention, Vec<Diagnostic>> {
    rows.get(loan.arena_index() as usize)
        .filter(|row| row.loan == loan)
        .map(|row| &row.retention)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "borrow resource has no complete descriptor-holder closure",
            )]
        })
}

pub(super) fn end<'flow>(
    facts: &'flow checked_trees::FlowFacts,
    retention: &CheckedBorrowResourceRetention,
) -> &'flow FlowBorrowWeakeningFact {
    facts
        .borrow_lifetimes
        .weakenings
        .get(retention.final_weakening)
}

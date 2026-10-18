//! Preserve the invocation's current loan graph and its backing dependencies.
//!
//! Flow has already applied evaluation-order replacement and weakening. Read
//! this call's entry constraints, never reconstruct an active roster from the
//! historical loans' last-use indices. Parent and supplier occurrences stay
//! distinct, including dependencies whose descriptor is no longer current.
//! Captured operand referents retain backing needed by the incomplete callee,
//! including constructed children with no named caller descriptor. Both routes
//! use the same owned-storage coordinate join before closing backing liveness.
//! This graph names logical demands; it establishes no StackLease or permission
//! to park. Incoming and unresolved actuals still require a per-value join.

use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowCallFact, FlowFacts, StateBorrowFact,
    SuspensionCrossingBorrowFrontier, SuspensionCrossingLoanBacking, SuspensionCrossingLoanFact,
};
use diagnostics::Diagnostic;

use super::StateCarryValue;

/// Resource construction has already decided access, capture and lifetime.
/// Build the occurrence join once for all crossings, without replaying that
/// judgment or equating loans merely because they share backing storage.
pub(super) fn resource_bindings(borrow: &BorrowFacts) -> Vec<LoanResourceBinding> {
    let mut resources = Vec::new();
    for (resource, row) in borrow.direct_loan_resources.iter() {
        bind_resource(
            &mut resources,
            row.loan,
            checked_trees::CheckedParentBorrowResource::DirectRoot { resource },
        );
    }
    for (resource, row) in borrow.reborrow_loan_resources.iter() {
        bind_resource(
            &mut resources,
            row.loan,
            checked_trees::CheckedParentBorrowResource::Reborrow { resource },
        );
    }
    resources
}

fn bind_resource(
    resources: &mut Vec<LoanResourceBinding>,
    loan: Handle<BorrowLoanFact>,
    resource: checked_trees::CheckedParentBorrowResource,
) {
    let position = loan.arena_index() as usize;
    if resources.len() <= position {
        resources.resize_with(position + 1, Default::default);
    }
    resources[position] = LoanResourceBinding { loan, resource };
}

/// Construction scratch for the current checked loan generation. Slot index
/// is a lookup coordinate, never identity: a reused slot cannot inherit the
/// previous occurrence's resource even if its index is unchanged.
#[derive(Default)]
pub(super) struct LoanResourceBinding {
    loan: Handle<BorrowLoanFact>,
    resource: checked_trees::CheckedParentBorrowResource,
}

/// Bind a live value to its complete stored-leaf frontier, preserving the
/// invocation's occurrence identities. A partial group is not evidence of
/// complete coverage. Captured actuals remain their own evaluation namespace:
/// a later operand may have replaced the named descriptor which supplied one.
/// Lifetime, access and physical preservation still need independent joins.
pub(super) fn value_frontier(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    frontier: &Arena<SuspensionCrossingLoanFact>,
    frontier_span: HandleSpan<SuspensionCrossingLoanFact>,
    operands: arena::HandleSpan<checked_trees::FlowOperandFact>,
    value: &checked_trees::SuspensionCrossingLiveValueFact,
    value_loans: &mut Arena<Handle<SuspensionCrossingLoanFact>>,
) -> checked_trees::SuspensionValueBorrowFrontier {
    use checked_trees::{SuspensionCrossingValueOrigin, SuspensionValueBorrowFrontier};

    // Formal type parameters describe the callee's obligations, not the
    // captured actual's stored leaves. Keep those floors on the live-value
    // fact, but inspect the producer's original actual type for coverage.
    // Neither a missing capture nor an absent type is the empty product.
    let (type_reference, captured) = match value.origin {
        SuspensionCrossingValueOrigin::CallArgument { position } => {
            let Some(operand) = operands.handles().nth(position) else {
                return SuspensionValueBorrowFrontier::Unresolved;
            };
            if !flow.control.operands.is_valid(operand) {
                return SuspensionValueBorrowFrontier::Unresolved;
            }
            (flow.control.operands.get(operand).type_reference, operand)
        }
        _ => (value.type_reference, Handle::invalid()),
    };
    if !type_reference.is_valid() {
        return SuspensionValueBorrowFrontier::Unresolved;
    }
    let Some(paths) =
        crate::borrow::view_link::borrow_carrying_owner_paths(program, type_reference)
    else {
        return SuspensionValueBorrowFrontier::Unresolved;
    };
    if paths.is_empty() {
        return SuspensionValueBorrowFrontier::ViewFree;
    }
    let symbol = match value.origin {
        SuspensionCrossingValueOrigin::Parameter { symbol, .. }
        | SuspensionCrossingValueOrigin::Local { symbol, .. } => symbol,
        SuspensionCrossingValueOrigin::CallArgument { .. } => Default::default(),
    };
    let matching = frontier_span
        .handles()
        .filter(|handle| {
            let entry = frontier.get(*handle);
            if !entry.active_at_call || !entry.retained_for_crossing {
                return false;
            }
            match entry.source {
                checked_trees::SuspensionCrossingLoanSource::Stored { loan, .. } => {
                    !captured.is_valid() && borrow.loans.get(loan).owner_symbol == symbol
                }
                checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } => {
                    captured.is_valid()
                        && borrow.call_loan_resources.get(resource).owner_operand == captured
                }
            }
        })
        .collect::<Vec<_>>();
    if !paths.iter().all(|path| {
        matching.iter().any(|handle| {
            let entry = frontier.get(*handle);
            owner_path(borrow, entry) == path.as_slice()
        })
    }) || matching.iter().any(|handle| {
        let entry = frontier.get(*handle);
        !paths
            .iter()
            .any(|path| owner_path(borrow, entry) == path.as_slice())
    }) {
        return SuspensionValueBorrowFrontier::Unresolved;
    }
    SuspensionValueBorrowFrontier::RetainedLoans {
        loans: value_loans.insert_many(matching),
    }
}

fn owner_path<'borrow>(
    borrow: &'borrow BorrowFacts,
    entry: &SuspensionCrossingLoanFact,
) -> &'borrow [checked_trees::BorrowLoanOwnerSegment] {
    match entry.source {
        checked_trees::SuspensionCrossingLoanSource::Stored { loan, .. } => {
            borrow.loan_owner_path(borrow.loans.get(loan))
        }
        checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } => borrow
            .owner_segments
            .span_or_empty(borrow.call_loan_resources.get(resource).owner_path),
    }
}

pub(super) fn retain_call_frontier(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    state: &StateBorrowFact,
    call: &FlowCallFact,
    source_values: &[StateCarryValue<'_>],
    resources: &[LoanResourceBinding],
    retained: &mut Arena<SuspensionCrossingLoanFact>,
) -> Result<SuspensionCrossingBorrowFrontier, Diagnostic> {
    let mut current = Vec::new();
    for loan in flow.borrow_loan_constraints(call.entry_constraints) {
        append_dependency(borrow, state, loan, &mut current)?;
    }
    let mut closure = current.clone();
    let captured = borrow
        .call_loan_resources
        .iter()
        .filter(|(_, resource)| {
            resource.machine_symbol == state.machine_symbol
                && resource.state_symbol == state.state_symbol
                && flow.control.calls.is_valid(resource.call)
                && flow.control.calls.get(resource.call) == call
        })
        .collect::<Vec<_>>();
    for (_, resource) in &captured {
        match resource.source {
            checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                append_dependency(borrow, state, loan, &mut closure)?;
            }
            checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                for loan in borrow.call_loan_parents.span_or_empty(parents) {
                    append_dependency(borrow, state, *loan, &mut closure)?;
                }
            }
        }
    }
    let mut loan_cursor = 0;
    while let Some(handle) = closure.get(loan_cursor).copied() {
        loan_cursor += 1;
        let loan = borrow.loans.get(handle);
        if let BorrowLoanLineage::Reborrow { parent_loan } = loan.lineage {
            append_dependency(borrow, state, parent_loan, &mut closure)?;
        }
        for supplier in borrow.loan_suppliers.span_or_empty(loan.supplying_loans) {
            append_dependency(borrow, state, *supplier, &mut closure)?;
        }
    }

    let stored = closure.into_iter().map(|handle| {
        let loan = borrow.loans.get(handle);
        let backing = if loan.incoming_reference.is_valid() {
            SuspensionCrossingLoanBacking::IncomingReferent {
                identity: loan.incoming_reference,
            }
        } else {
            owned_backing_origin(program, source_values, loan.root, call.statement_index)
                .map_or(SuspensionCrossingLoanBacking::Unresolved, |origin| {
                    SuspensionCrossingLoanBacking::StateValue { origin }
                })
        };
        SuspensionCrossingLoanFact {
            source: checked_trees::SuspensionCrossingLoanSource::Stored {
                loan: handle,
                resource: resources
                    .get(handle.arena_index() as usize)
                    .filter(|binding| binding.loan == handle)
                    .map(|binding| binding.resource.clone())
                    .unwrap_or_default(),
            },
            active_at_call: current.contains(&handle),
            retained_for_crossing: false,
            backing,
        }
    });
    let actuals = captured
        .into_iter()
        .map(|(resource, row)| SuspensionCrossingLoanFact {
            source: checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource },
            active_at_call: true,
            retained_for_crossing: true,
            backing: owned_backing_origin(
                program,
                source_values,
                row.parent_lifetime.root,
                call.statement_index,
            )
            .map_or(SuspensionCrossingLoanBacking::Unresolved, |origin| {
                SuspensionCrossingLoanBacking::StateValue { origin }
            }),
        });
    let loans = retained.insert_many(stored.chain(actuals));
    Ok(SuspensionCrossingBorrowFrontier {
        loans,
        operands: call.argument_operands(),
    })
}

/// A root view denotes caller-supplied storage, never its descriptor slot.
/// The entry tracker models contained descriptors, not the root borrowed
/// parameter permission; absent that captured identity, backing stays unknown.
fn owned_backing_origin(
    program: &dyn ::validation::CheckingInput,
    source_values: &[StateCarryValue<'_>],
    root: facts::PlaceRoot,
    statement_index: usize,
) -> Option<checked_trees::SuspensionCrossingValueOrigin> {
    let root = root.as_symbol()?;
    source_values
        .iter()
        .find(|value| value.symbol == root && value.available_before(statement_index))
        .filter(|value| {
            let reference = validation::peel_declared_constraints(program, value.type_reference);
            reference.is_valid()
                && validation::reference_shell(program, reference).is_none()
                && !matches!(
                    program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Slice { .. }
                )
        })
        .map(|value| value.origin)
}

fn append_dependency(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    loan: Handle<BorrowLoanFact>,
    closure: &mut Vec<Handle<BorrowLoanFact>>,
) -> Result<(), Diagnostic> {
    if !borrow.state_owns_loan(state, loan) {
        return Err(Diagnostic::error(
            "suspension loan dependency has no retained state-owned occurrence".to_owned(),
        ));
    }
    if !closure.contains(&loan) {
        closure.push(loan);
    }
    Ok(())
}

/// Join the value's complete loan closure to the already constructed resource
/// lifetimes and retained owner storage. The caller cannot leave its invocation
/// while this ordinary call is incomplete. Keeping the original root in that
/// invocation therefore extends no source lifetime and returns no parent access.
/// Incoming referents still need their independent enclosing-activation join.
///
/// CPU/thread demands remain conservative and address stability remains a
/// nonmoving-backing obligation. This is not a runtime park/resume receipt.
pub(super) fn retained_lifetime_demand(
    borrow: &BorrowFacts,
    frontier_loans: &Arena<SuspensionCrossingLoanFact>,
    value_loans: &Arena<Handle<SuspensionCrossingLoanFact>>,
    state: &StateBorrowFact,
    frontier: SuspensionCrossingBorrowFrontier,
    value: &checked_trees::SuspensionCrossingLiveValueFact,
    live_values: &[checked_trees::SuspensionCrossingLiveValueFact],
) -> Result<language_semantics::CarryPolicy, &'static str> {
    use checked_trees::{CheckedParentBorrowResource, SuspensionValueBorrowFrontier};
    use language_semantics::{CarryPolicy, CarrySuspension};
    let links = match value.borrow_frontier {
        SuspensionValueBorrowFrontier::ViewFree => return Ok(CarryPolicy::PERMISSIVE),
        SuspensionValueBorrowFrontier::RetainedLoans { loans } => value_loans
            .span(loans)
            .filter(|links| !links.is_empty())
            .ok_or("the stored loan-leaf frontier is missing")?,
        SuspensionValueBorrowFrontier::Unresolved => {
            return Err("the stored loan-leaf frontier is incomplete");
        }
    };
    let rows = frontier_loans
        .span(frontier.loans)
        .ok_or("the crossing's loan closure is missing")?;
    let mut pending = links.to_vec();
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        if !frontier.loans.handles().any(|member| member == handle) {
            return Err("the value names another crossing's loan");
        }
        let row = frontier_loans.get(handle);
        if !row.retained_for_crossing {
            return Err("the loan has no retained invocation owner");
        }
        let origin = match row.backing {
            SuspensionCrossingLoanBacking::StateValue { origin } => origin,
            SuspensionCrossingLoanBacking::IncomingReferent { .. }
            | SuspensionCrossingLoanBacking::Unresolved => {
                return Err("the referent has no retained enclosing-activation lifetime");
            }
        };
        if !live_values.iter().any(|live| live.origin == origin) {
            return Err("the original backing is absent from retained storage");
        }
        let (loan_handle, parent_resource) = match row.source {
            checked_trees::SuspensionCrossingLoanSource::Stored { loan, ref resource } => {
                if !borrow.state_owns_loan(state, loan) {
                    return Err("the loan has no retained invocation owner");
                }
                (loan, resource)
            }
            checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } => {
                if !borrow.call_loan_resources.is_valid(resource) {
                    return Err("the captured operand has no checked loan resource");
                }
                let resource = borrow.call_loan_resources.get(resource);
                let root = match origin {
                    checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                    | checked_trees::SuspensionCrossingValueOrigin::Local { symbol, .. } => symbol,
                    checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => {
                        return Err("the captured backing has no enclosing owner");
                    }
                };
                if resource.machine_symbol != state.machine_symbol
                    || resource.state_symbol != state.state_symbol
                    || resource.parent_lifetime.machine_symbol != state.machine_symbol
                    || resource.parent_lifetime.state_symbol != state.state_symbol
                    || resource.parent_lifetime.root != facts::PlaceRoot::Symbol(root)
                    || !frontier
                        .operands
                        .handles()
                        .any(|operand| operand == resource.owner_operand)
                {
                    return Err("the captured loan has no exact enclosing-invocation lifetime");
                }
                let parents = match resource.source {
                    checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                    checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                        borrow
                            .call_loan_parents
                            .span(parents)
                            .ok_or("the captured loan's parent roster is missing")?
                            .to_vec()
                    }
                };
                for loan in parents {
                    let parent = frontier
                        .loans
                        .handles()
                        .zip(rows)
                        .find(|(_, row)| row.stored_loan() == loan)
                        .map(|(handle, _)| handle)
                        .ok_or("the captured loan's ancestor resource is not retained")?;
                    pending.push(parent);
                }
                continue;
            }
        };
        let loan = borrow.loans.get(loan_handle);
        match *parent_resource {
            CheckedParentBorrowResource::DirectRoot { resource }
                if borrow.direct_loan_resources.is_valid(resource) =>
            {
                let resource = borrow.direct_loan_resources.get(resource);
                if resource.loan != loan_handle
                    || resource.parent_lifetime.machine_symbol != state.machine_symbol
                    || resource.parent_lifetime.state_symbol != state.state_symbol
                    || resource.parent_lifetime.root != loan.root
                {
                    return Err("the root loan has no exact enclosing-invocation lifetime");
                }
            }
            CheckedParentBorrowResource::Reborrow { resource }
                if borrow.reborrow_loan_resources.is_valid(resource) =>
            {
                let resource = borrow.reborrow_loan_resources.get(resource);
                if resource.loan != loan_handle
                    || resource.machine_symbol != state.machine_symbol
                    || resource.state_symbol != state.state_symbol
                {
                    return Err("the reborrow has no exact enclosing-invocation resource");
                }
                let parent = frontier
                    .loans
                    .handles()
                    .zip(rows)
                    .find(|(_, row)| row.stored_loan() == resource.parent_loan)
                    .map(|(handle, _)| handle)
                    .ok_or("the immediate parent resource is not retained")?;
                pending.push(parent);
            }
            _ => return Err("the loan lifetime resource is unavailable"),
        }
        for supplying in borrow.loan_suppliers.span_or_empty(loan.supplying_loans) {
            let supplier = frontier
                .loans
                .handles()
                .zip(rows)
                .find(|(_, row)| row.stored_loan() == *supplying)
                .map(|(handle, _)| handle)
                .ok_or("the transported loan's supplier is not retained")?;
            pending.push(supplier);
        }
    }
    Ok(CarryPolicy {
        suspension: CarrySuspension::Allowed,
        ..CarryPolicy::STRICT
    })
}

/// A live descriptor keeps its actual backing live even when later source
/// expressions mention only the descriptor. Follow exact retained dependency
/// edges, not all historical loans sharing that descriptor's symbol. Incoming
/// referents have no caller-frame coordinate and never retain the descriptor
/// slot as a substitute for their backing.
pub(super) fn retain_backing_values(
    borrow: &BorrowFacts,
    frontier: &mut [SuspensionCrossingLoanFact],
    live_symbols: &mut Vec<symbols::SymbolHandle>,
) {
    let mut retained_loans = Vec::new();
    for entry in frontier.iter().filter(|entry| entry.retained_for_crossing) {
        if let checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } =
            entry.source
        {
            let resource = borrow.call_loan_resources.get(resource);
            match resource.source {
                checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                    if !retained_loans.contains(&loan) {
                        retained_loans.push(loan);
                    }
                }
                checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                    for loan in borrow.call_loan_parents.span_or_empty(parents) {
                        if !retained_loans.contains(loan) {
                            retained_loans.push(*loan);
                        }
                    }
                }
            }
            if let SuspensionCrossingLoanBacking::StateValue { origin } = entry.backing {
                let root = match origin {
                    checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                    | checked_trees::SuspensionCrossingValueOrigin::Local { symbol, .. } => symbol,
                    checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => continue,
                };
                if !live_symbols.contains(&root) {
                    live_symbols.push(root);
                }
            }
        }
    }
    let mut value_cursor = 0;
    let mut loan_cursor = 0;
    // Backing can itself contain view leaves. Newly retained storage enters
    // the same value queue so indirect carrier chains reach a fixed point.
    while value_cursor < live_symbols.len() || loan_cursor < retained_loans.len() {
        if let Some(symbol) = live_symbols.get(value_cursor).copied() {
            value_cursor += 1;
            for entry in frontier.iter().filter(|entry| {
                entry.active_at_call
                    && entry.stored_loan().is_valid()
                    && borrow.loans.get(entry.stored_loan()).owner_symbol == symbol
            }) {
                if !retained_loans.contains(&entry.stored_loan()) {
                    retained_loans.push(entry.stored_loan());
                }
            }
            continue;
        }
        let handle = retained_loans[loan_cursor];
        loan_cursor += 1;
        let loan = borrow.loans.get(handle);
        for dependency in std::iter::once(match loan.lineage {
            BorrowLoanLineage::Reborrow { parent_loan } => parent_loan,
            BorrowLoanLineage::DirectRoot
            | BorrowLoanLineage::UnretainedDerived
            | BorrowLoanLineage::LifetimeDependency => Handle::invalid(),
        })
        .chain(
            borrow
                .loan_suppliers
                .span_or_empty(loan.supplying_loans)
                .iter()
                .copied(),
        ) {
            if dependency.is_valid() && !retained_loans.contains(&dependency) {
                retained_loans.push(dependency);
            }
        }
        let Some(entry) = frontier
            .iter_mut()
            .find(|entry| entry.stored_loan() == handle)
        else {
            continue;
        };
        entry.retained_for_crossing = true;
        if let SuspensionCrossingLoanBacking::StateValue { origin } = entry.backing {
            let symbol = match origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                | checked_trees::SuspensionCrossingValueOrigin::Local { symbol, .. } => symbol,
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => continue,
            };
            if !live_symbols.contains(&symbol) {
                live_symbols.push(symbol);
            }
        }
    }
}

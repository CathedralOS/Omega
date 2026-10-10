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
    if captured.is_valid() {
        return SuspensionValueBorrowFrontier::CapturedOperand { operand: captured };
    }
    let symbol = match value.origin {
        SuspensionCrossingValueOrigin::Parameter { symbol, .. }
        | SuspensionCrossingValueOrigin::Local { symbol, .. } => symbol,
        SuspensionCrossingValueOrigin::CallArgument { .. } => {
            return SuspensionValueBorrowFrontier::Unresolved;
        }
    };
    let matching = frontier_span
        .handles()
        .filter(|handle| {
            let entry = frontier.get(*handle);
            entry.active_at_call
                && entry.retained_for_crossing
                && borrow.loans.get(entry.loan).owner_symbol == symbol
        })
        .collect::<Vec<_>>();
    if !paths.iter().all(|path| {
        matching.iter().any(|handle| {
            let entry = frontier.get(*handle);
            borrow.loan_owner_path(borrow.loans.get(entry.loan)) == path.as_slice()
        })
    }) || matching.iter().any(|handle| {
        let entry = frontier.get(*handle);
        !paths
            .iter()
            .any(|path| borrow.loan_owner_path(borrow.loans.get(entry.loan)) == path.as_slice())
    }) {
        return SuspensionValueBorrowFrontier::Unresolved;
    }
    SuspensionValueBorrowFrontier::RetainedLoans {
        loans: value_loans.insert_many(matching),
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
    let mut loan_cursor = 0;
    while let Some(handle) = closure.get(loan_cursor).copied() {
        loan_cursor += 1;
        let loan = borrow.loans.get(handle);
        if let BorrowLoanLineage::Reborrow { parent_loan } = loan.lineage {
            append_dependency(borrow, state, parent_loan, &mut closure)?;
        }
        if loan.supplying_loan.is_valid() {
            append_dependency(borrow, state, loan.supplying_loan, &mut closure)?;
        }
    }

    let loans = retained.insert_many(closure.into_iter().map(|handle| {
        let loan = borrow.loans.get(handle);
        let backing = if loan.incoming_reference.is_valid() {
            SuspensionCrossingLoanBacking::IncomingReferent {
                identity: loan.incoming_reference,
            }
        } else {
            owned_backing_origin(
                program,
                source_values,
                loan.root_symbol,
                call.statement_index,
            )
            .map_or(SuspensionCrossingLoanBacking::Unresolved, |origin| {
                SuspensionCrossingLoanBacking::StateValue { origin }
            })
        };
        SuspensionCrossingLoanFact {
            loan: handle,
            resource: resources
                .get(handle.arena_index() as usize)
                .filter(|binding| binding.loan == handle)
                .map(|binding| binding.resource.clone())
                .unwrap_or_default(),
            active_at_call: current.contains(&handle),
            retained_for_crossing: false,
            backing,
        }
    }));
    Ok(SuspensionCrossingBorrowFrontier {
        loans,
        operands: call.operands,
    })
}

/// A root view denotes caller-supplied storage, never its descriptor slot.
/// The entry tracker models contained descriptors, not the root borrowed
/// parameter permission; absent that captured identity, backing stays unknown.
fn owned_backing_origin(
    program: &dyn ::validation::CheckingInput,
    source_values: &[StateCarryValue<'_>],
    root: symbols::SymbolHandle,
    statement_index: usize,
) -> Option<checked_trees::SuspensionCrossingValueOrigin> {
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

/// The callee may still be using its completed actual while the caller has no
/// later use. Traverse that retained value tree, not argument syntax or the
/// current descriptor roster: a later operand may already have replaced the
/// descriptor which supplied an earlier actual. Empty referents remain unknown
/// or detached; this operation establishes no borrow-free or parking permission.
pub(super) fn retain_operand_backing(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    frontier: SuspensionCrossingBorrowFrontier,
    source_values: &[StateCarryValue<'_>],
    statement_index: usize,
    live_symbols: &mut Vec<symbols::SymbolHandle>,
) -> Result<(), Diagnostic> {
    let missing_capture =
        || Diagnostic::error("suspension argument has no retained operand capture".to_owned());
    let mut pending = flow
        .control
        .operands
        .span(frontier.operands)
        .ok_or_else(missing_capture)?
        .iter()
        .collect::<Vec<_>>();
    // Flow appends each constructed child's capture after evaluating its own
    // children. These producer-owned edges form a tree, not a graph needing a
    // repeated linear membership search for every array or record element.
    while let Some(operand) = pending.pop() {
        for referent in flow
            .control
            .operand_referents
            .span(operand.referents)
            .ok_or_else(missing_capture)?
        {
            let Some(root) = referent.root.as_symbol() else {
                continue;
            };
            if owned_backing_origin(program, source_values, root, statement_index).is_some()
                && !live_symbols.contains(&root)
            {
                live_symbols.push(root);
            }
        }
        for child in flow
            .control
            .operand_children
            .span(operand.children)
            .ok_or_else(missing_capture)?
        {
            if !flow.control.operands.is_valid(child.value) {
                return Err(missing_capture());
            }
            pending.push(flow.control.operands.get(child.value));
        }
    }
    Ok(())
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
        SuspensionValueBorrowFrontier::Unresolved
        | SuspensionValueBorrowFrontier::CapturedOperand { .. } => {
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
        if !row.retained_for_crossing || !borrow.state_owns_loan(state, row.loan) {
            return Err("the loan has no retained invocation owner");
        }
        let loan = borrow.loans.get(row.loan);
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
        match row.resource {
            CheckedParentBorrowResource::DirectRoot { resource }
                if borrow.direct_loan_resources.is_valid(resource) =>
            {
                let resource = borrow.direct_loan_resources.get(resource);
                if resource.loan != row.loan
                    || resource.parent_lifetime.machine_symbol != state.machine_symbol
                    || resource.parent_lifetime.state_symbol != state.state_symbol
                    || resource.parent_lifetime.root_symbol != loan.root_symbol
                {
                    return Err("the root loan has no exact enclosing-invocation lifetime");
                }
            }
            CheckedParentBorrowResource::Reborrow { resource }
                if borrow.reborrow_loan_resources.is_valid(resource) =>
            {
                let resource = borrow.reborrow_loan_resources.get(resource);
                if resource.loan != row.loan
                    || resource.machine_symbol != state.machine_symbol
                    || resource.state_symbol != state.state_symbol
                {
                    return Err("the reborrow has no exact enclosing-invocation resource");
                }
                let parent = frontier
                    .loans
                    .handles()
                    .zip(rows)
                    .find(|(_, row)| row.loan == resource.parent_loan)
                    .map(|(handle, _)| handle)
                    .ok_or("the immediate parent resource is not retained")?;
                pending.push(parent);
            }
            _ => return Err("the loan lifetime resource is unavailable"),
        }
        if loan.supplying_loan.is_valid() {
            let supplier = frontier
                .loans
                .handles()
                .zip(rows)
                .find(|(_, row)| row.loan == loan.supplying_loan)
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
    let mut value_cursor = 0;
    let mut loan_cursor = 0;
    // Backing can itself contain view leaves. Newly retained storage enters
    // the same value queue so indirect carrier chains reach a fixed point.
    while value_cursor < live_symbols.len() || loan_cursor < retained_loans.len() {
        if let Some(symbol) = live_symbols.get(value_cursor).copied() {
            value_cursor += 1;
            for entry in frontier.iter().filter(|entry| {
                entry.active_at_call && borrow.loans.get(entry.loan).owner_symbol == symbol
            }) {
                if !retained_loans.contains(&entry.loan) {
                    retained_loans.push(entry.loan);
                }
            }
            continue;
        }
        let handle = retained_loans[loan_cursor];
        loan_cursor += 1;
        let loan = borrow.loans.get(handle);
        for dependency in [
            match loan.lineage {
                BorrowLoanLineage::Reborrow { parent_loan } => parent_loan,
                BorrowLoanLineage::DirectRoot | BorrowLoanLineage::UnretainedDerived => {
                    Handle::invalid()
                }
            },
            loan.supplying_loan,
        ] {
            if dependency.is_valid() && !retained_loans.contains(&dependency) {
                retained_loans.push(dependency);
            }
        }
        let Some(entry) = frontier.iter_mut().find(|entry| entry.loan == handle) else {
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

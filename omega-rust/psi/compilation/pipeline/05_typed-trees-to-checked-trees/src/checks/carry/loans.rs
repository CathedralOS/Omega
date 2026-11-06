//! Preserve the invocation's current loan graph and its backing dependencies.
//!
//! Flow has already applied evaluation-order replacement and weakening. Read
//! this call's entry constraints, never reconstruct an active roster from the
//! historical loans' last-use indices. Parent and supplier occurrences stay
//! distinct, including dependencies whose descriptor is no longer current.
//! Captured operand referents retain backing needed by the incomplete callee,
//! including constructed children with no named caller descriptor. Both routes
//! join either owned storage or exact invocation-entry permissions before
//! closing backing liveness.
//! This graph names logical demands; it establishes no StackLease or permission
//! to park. Every value still requires its complete per-value lifetime join.

use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowCallFact, FlowFacts, StateBorrowFact,
    SuspensionCrossingBorrowFrontier as BorrowFrontier,
    SuspensionCrossingLoanBacking as LoanBacking, SuspensionCrossingLoanFact as LoanFact,
};
use diagnostics::Diagnostic;

use super::StateCarryValue;

mod outputs;

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
    for (resource, row) in borrow.supplied_output_resources.iter() {
        bind_resource(
            &mut resources,
            row.loan,
            checked_trees::CheckedParentBorrowResource::SuppliedOutput { resource },
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
    frontier: &Arena<LoanFact>,
    frontier_span: HandleSpan<LoanFact>,
    operands: arena::HandleSpan<checked_trees::FlowOperandFact>,
    value: &checked_trees::SuspensionCrossingLiveValueFact,
    value_loans: &mut Arena<Handle<LoanFact>>,
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
                checked_trees::SuspensionCrossingLoanSource::Incoming { identity, .. } => {
                    let incoming = borrow.incoming_references.get(identity);
                    !captured.is_valid()
                        && incoming.parameter_symbol() == symbol
                        && incoming.carrier_type
                            == validation::peel_declared_constraints(program, type_reference)
                }
                checked_trees::SuspensionCrossingLoanSource::Stored { loan, .. } => {
                    !captured.is_valid()
                        && borrow.loans.get(loan).lineage != BorrowLoanLineage::LifetimeDependency
                        && borrow.loans.get(loan).owner_symbol == symbol
                }
                checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } => {
                    captured.is_valid()
                        && borrow.call_loan_resources.get(resource).owner_operand == captured
                }
                checked_trees::SuspensionCrossingLoanSource::LifetimeDependency { .. }
                | checked_trees::SuspensionCrossingLoanSource::OutputInput { .. } => false,
            }
        })
        .collect::<Vec<_>>();
    // Prepared generic fields can have different handles for the same authored
    // member. Rejoin each loan to this value's exact applied declaration before
    // comparing coverage; field spelling or equal layout supplies no identity.
    let matching_paths = matching
        .iter()
        .map(|handle| {
            validation::corresponding_loan_owner_path(
                program,
                type_reference,
                owner_path(borrow, frontier.get(*handle)),
            )
        })
        .collect::<Option<Vec<_>>>();
    if matching_paths.as_ref().is_none_or(|matching| {
        !paths.iter().all(|path| matching.contains(path))
            || matching.iter().any(|path| !paths.contains(path))
    }) {
        if ::validation::trace_flags::struct_trace() {
            eprintln!(
                "STRUCT suspension-leaf-frontier value={:?} type={type_reference:?} required={paths:?} matching={:?}",
                value.origin,
                matching
                    .iter()
                    .map(|handle| {
                        let entry = frontier.get(*handle);
                        let handle = entry.stored_loan();
                        (
                            handle,
                            owner_path(borrow, entry),
                            handle.is_valid().then(|| &borrow.loans.get(handle).lineage),
                        )
                    })
                    .collect::<Vec<_>>()
            );
        }
        return SuspensionValueBorrowFrontier::Unresolved;
    }
    SuspensionValueBorrowFrontier::RetainedLoans {
        loans: value_loans.insert_many(matching),
    }
}

fn owner_path<'borrow>(
    borrow: &'borrow BorrowFacts,
    entry: &LoanFact,
) -> &'borrow [checked_trees::BorrowLoanOwnerSegment] {
    match entry.source {
        checked_trees::SuspensionCrossingLoanSource::LifetimeDependency { .. }
        | checked_trees::SuspensionCrossingLoanSource::OutputInput { .. } => &[],
        checked_trees::SuspensionCrossingLoanSource::Incoming { identity, .. } => borrow
            .owner_segments
            .span_or_empty(borrow.incoming_references.get(identity).owner_path),
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
    retained: &mut Arena<LoanFact>,
) -> Result<BorrowFrontier, Diagnostic> {
    let mut current = Vec::new();
    for loan in flow.borrow_loan_constraints(call.entry_constraints) {
        append_dependency(borrow, state, loan, &mut current)?;
    }
    let mut closure = current.clone();
    let mut captured = borrow
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
    let mut held_inputs = Vec::new();
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
        if let Some(output) = outputs::output(borrow, loan.incoming_reference) {
            append_dependency(
                borrow,
                state,
                borrow.supplied_output_resources.get(output).loan,
                &mut closure,
            )?;
        }
        if let Some(inputs) = borrow.retained_call_inputs(handle) {
            for resource in inputs {
                let row = borrow.call_loan_resources.get(*resource);
                if !captured.iter().any(|(candidate, _)| candidate == resource) {
                    captured.push((*resource, row));
                }
                for parent in outputs::parents(borrow, *resource).map_err(Diagnostic::error)? {
                    append_dependency(borrow, state, parent, &mut closure)?;
                }
            }
        }
        if let Some(input_span) = outputs::inputs(borrow, handle) {
            let inputs = borrow
                .supplied_output_inputs
                .span(input_span)
                .ok_or_else(|| {
                    Diagnostic::error("the output's complete captured input roster is unavailable")
                })?;
            for (input_handle, input) in input_span.handles().zip(inputs) {
                held_inputs.push((handle, input_handle));
                if let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                    input.permission
                {
                    for parent in outputs::parents(borrow, resource).map_err(Diagnostic::error)? {
                        append_dependency(borrow, state, parent, &mut closure)?;
                    }
                    let resource = borrow.call_loan_resources.get(resource);
                    if let Some(output) =
                        outputs::output(borrow, resource.backing_incoming_reference)
                    {
                        append_dependency(
                            borrow,
                            state,
                            borrow.supplied_output_resources.get(output).loan,
                            &mut closure,
                        )?;
                    }
                }
            }
        }
    }

    let current_incoming = flow
        .incoming_reference_constraints(call.entry_constraints)
        .collect::<Vec<_>>();
    let incoming = state.incoming_references.handles().filter_map(|identity| {
        let entry = borrow.incoming_references.get(identity);
        let references =
            validation::borrow_contracts::carried_reference_paths(program, entry.carrier_type)?;
        let path = borrow.owner_segments.span_or_empty(entry.owner_path);
        let (_, access) = references
            .into_iter()
            .find(|(candidate, _)| candidate == path)?;
        Some(LoanFact {
            source: checked_trees::SuspensionCrossingLoanSource::Incoming { identity, access },
            active_at_call: current_incoming.contains(&identity),
            retained_for_crossing: false,
            backing: LoanBacking::IncomingReferent { identity },
        })
    });
    let backing = |root, identity| {
        if let Some(resource) = outputs::output(borrow, identity) {
            return LoanBacking::CompletedOutput { resource };
        }
        if borrow.incoming_references.is_valid(identity) {
            return LoanBacking::IncomingReferent { identity };
        }
        if let Some(origin) =
            owned_backing_origin(program, source_values, root, call.statement_index)
        {
            return LoanBacking::StateValue { origin };
        }
        // A direct access through an incoming root reference carries the
        // invocation's borrowed-parameter permission, not owned pointer storage.
        let identity = state.incoming_references.handles().find(|identity| {
            let entry = borrow.incoming_references.get(*identity);
            entry.place_root() == root
                && entry.owner_path.is_empty()
                && matches!(
                    entry.origin,
                    checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
                )
                && current_incoming.contains(identity)
        });
        identity.map_or(LoanBacking::Unresolved, |identity| {
            LoanBacking::IncomingReferent { identity }
        })
    };
    let stored = closure.into_iter().map(|handle| {
        let loan = borrow.loans.get(handle);
        if let Some(resource) = outputs::dependency(borrow, handle) {
            return LoanFact {
                source: checked_trees::SuspensionCrossingLoanSource::LifetimeDependency {
                    loan: handle,
                    resource,
                },
                active_at_call: current.contains(&handle),
                retained_for_crossing: false,
                backing: LoanBacking::InputDependencies,
            };
        }
        let backing = backing(loan.root, loan.incoming_reference);
        LoanFact {
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
    let actuals = captured.into_iter().map(|(resource, row)| LoanFact {
        source: checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource },
        active_at_call: flow.control.calls.get(row.call) == call,
        retained_for_crossing: flow.control.calls.get(row.call) == call,
        backing: backing(row.parent_lifetime.root, row.backing_incoming_reference),
    });
    let inputs = held_inputs.into_iter().map(|(owner, input)| {
        let selected = borrow.supplied_output_inputs.get(input);
        let backing = match selected.permission {
            checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } => {
                let resource = borrow.call_loan_resources.get(resource);
                backing(
                    resource.parent_lifetime.root,
                    resource.backing_incoming_reference,
                )
            }
            checked_trees::CheckedOutputBorrowInputPermission::Incoming { identity, .. } => {
                LoanBacking::IncomingReferent { identity }
            }
        };
        LoanFact {
            source: checked_trees::SuspensionCrossingLoanSource::OutputInput { owner, input },
            active_at_call: false,
            retained_for_crossing: false,
            backing,
        }
    });
    let loans = retained.insert_many(incoming.chain(stored).chain(actuals).chain(inputs));
    Ok(BorrowFrontier {
        loans,
        operands: call.argument_operands(),
    })
}

fn retained_input_owner(
    borrow: &BorrowFacts,
    rows: &[LoanFact],
    resource: Handle<checked_trees::CheckedCallBorrowLoanResource>,
) -> Option<Handle<BorrowLoanFact>> {
    rows.iter()
        .filter(|row| row.retained_for_crossing)
        .find_map(|row| {
            let loan = row.stored_loan();
            (loan.is_valid()
                && borrow
                    .retained_call_inputs(loan)
                    .is_some_and(|inputs| inputs.contains(&resource)))
            .then_some(loan)
        })
}

/// A root view denotes caller-supplied storage, never its descriptor slot.
/// Incoming backing uses its explicit entry permission in the caller above;
/// this query must never turn the descriptor into an owned referent.
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
    crate::shared::collections::push_unique(&mut *closure, loan);
    Ok(())
}

/// Join the value's complete loan closure to the already constructed resource
/// lifetimes and retained owner storage. The caller cannot leave its invocation
/// while this ordinary call is incomplete. Keeping the original root in that
/// invocation therefore extends no source lifetime and returns no parent access.
/// Incoming permissions retain their exact enclosing-invocation argument; the
/// caller's checked call frontier independently retains the original backing.
///
/// CPU/thread demands remain conservative and address stability remains a
/// nonmoving-backing obligation. This is not a runtime park/resume receipt.
pub(super) fn retained_lifetime_demand(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    frontier_loans: &Arena<LoanFact>,
    value_loans: &Arena<Handle<LoanFact>>,
    state: &StateBorrowFact,
    frontier: BorrowFrontier,
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
        if let checked_trees::SuspensionCrossingLoanSource::OutputInput { owner, input } =
            row.source
        {
            outputs::validate_input(borrow, flow, state, owner, input)?;
        }
        let root = match row.backing {
            LoanBacking::StateValue { origin } => {
                if !live_values.iter().any(|live| live.origin == origin) {
                    return Err("the original backing is absent from retained storage");
                }
                match origin {
                    checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                    | checked_trees::SuspensionCrossingValueOrigin::Local { symbol, .. } => {
                        facts::PlaceRoot::Symbol(symbol)
                    }
                    checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => {
                        return Err("the captured backing has no enclosing owner");
                    }
                }
            }
            LoanBacking::IncomingReferent { identity } => {
                if !state
                    .incoming_references
                    .handles()
                    .any(|member| member == identity)
                {
                    return Err("the incoming loan has no exact invocation-entry permission");
                }
                let incoming = borrow.incoming_references.get(identity);
                if !matches!(
                    row.source,
                    checked_trees::SuspensionCrossingLoanSource::OutputInput { .. }
                ) && !matches!(row.source,
                    checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource }
                        if retained_input_owner(borrow, rows, resource).is_some())
                    && !rows.iter().any(|entry| {
                        entry.active_at_call
                            && matches!(entry.source,
                            checked_trees::SuspensionCrossingLoanSource::Incoming {
                                identity: current, ..
                            } if current == identity)
                    })
                {
                    return Err("the incoming descriptor has no current retained capture");
                }
                if incoming.machine_symbol != state.machine_symbol
                    || incoming.state_symbol != state.state_symbol
                    || !live_values.iter().any(|live| {
                        matches!(live.origin,
                        checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                            if symbol == incoming.parameter_symbol())
                    })
                {
                    return Err("the incoming loan's activation argument is not retained");
                }
                incoming.place_root()
            }
            LoanBacking::CompletedOutput { resource } => {
                outputs::validate_output(borrow, flow, state, resource)?;
                let output = borrow.supplied_output_resources.get(resource);
                if row.stored_loan() != output.loan {
                    pending.push(retained_loan(frontier, rows, output.loan)?);
                }
                borrow.loans.get(output.loan).root
            }
            LoanBacking::InputDependencies => {
                if !matches!(
                    row.source,
                    checked_trees::SuspensionCrossingLoanSource::LifetimeDependency { .. }
                ) {
                    return Err(
                        "a physical loan cannot use nonspatial input obligations as backing",
                    );
                }
                facts::PlaceRoot::default()
            }
            LoanBacking::Unresolved => {
                return Err("the referent has no retained enclosing-activation lifetime");
            }
        };
        let (loan_handle, parent_resource) = match row.source {
            checked_trees::SuspensionCrossingLoanSource::LifetimeDependency { loan, resource } => {
                if outputs::dependency(borrow, loan) != Some(resource) {
                    return Err("the lifetime obligation has no exact checked dependency resource");
                }
                append_retained_inputs(borrow, frontier, rows, loan, &mut pending)?;
                for supplier in borrow
                    .loan_suppliers
                    .span_or_empty(borrow.loans.get(loan).supplying_loans)
                {
                    pending.push(retained_loan(frontier, rows, *supplier)?);
                }
                continue;
            }
            checked_trees::SuspensionCrossingLoanSource::OutputInput { owner, input } => {
                pending.push(retained_loan(frontier, rows, owner)?);
                if let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                    borrow.supplied_output_inputs.get(input).permission
                {
                    for parent in outputs::parents(borrow, resource)? {
                        pending.push(retained_loan(frontier, rows, parent)?);
                    }
                }
                continue;
            }
            checked_trees::SuspensionCrossingLoanSource::Incoming { identity, .. } => {
                if row.backing != (LoanBacking::IncomingReferent { identity }) {
                    return Err("the incoming permission names another backing");
                }
                continue;
            }
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
                let resource_handle = resource;
                let resource = borrow.call_loan_resources.get(resource);
                if !flow.control.calls.is_valid(resource.call)
                    || !flow
                        .control
                        .calls
                        .get(resource.call)
                        .argument_operands()
                        .handles()
                        .any(|operand| operand == resource.owner_operand)
                    || resource.machine_symbol != state.machine_symbol
                    || resource.state_symbol != state.state_symbol
                    || resource.parent_lifetime.machine_symbol != state.machine_symbol
                    || resource.parent_lifetime.state_symbol != state.state_symbol
                    || resource.parent_lifetime.root != root
                    || (!frontier
                        .operands
                        .handles()
                        .any(|operand| operand == resource.owner_operand)
                        && retained_input_owner(borrow, rows, resource_handle).is_none())
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
            CheckedParentBorrowResource::SuppliedOutput { resource } => {
                outputs::validate_output(borrow, flow, state, resource)?;
                if borrow.supplied_output_resources.get(resource).loan != loan_handle {
                    return Err("the output permission names another original occurrence");
                }
                append_retained_inputs(borrow, frontier, rows, loan_handle, &mut pending)?;
            }
            // A descriptor transfer is in neither resource roster. Its own
            // backing was joined above; its lifetime and access come from
            // exact suppliers, which the walk below retains and checks.
            _ if borrow.retained_call_inputs(loan_handle).is_some() => {
                for resource in borrow
                    .retained_call_inputs(loan_handle)
                    .ok_or("the saved reference lost its retained inputs")?
                {
                    let (input, _) = frontier.loans.handles().zip(rows).find(|(_, row)| matches!(row.source,
                        checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource: candidate } if candidate == *resource))
                        .ok_or("the saved reference's original call input is not retained")?;
                    pending.push(input);
                }
            }
            _ if crate::checks::borrows::transport_suppliers(borrow, state, loan_handle)
                .is_some() => {}
            _ => {
                if ::validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE suspension-lifetime-resource-unavailable loan={loan_handle:?} row={loan:?} resource={parent_resource:?}"
                    );
                }
                return Err("the loan lifetime resource is unavailable");
            }
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
    frontier: &mut [LoanFact],
    live_symbols: &mut Vec<symbols::SymbolHandle>,
) {
    let mut loans = Vec::new();
    let mut inputs = Vec::new();
    for entry in frontier.iter().filter(|entry| entry.retained_for_crossing) {
        retain_backing(borrow, entry.backing, live_symbols, &mut loans);
        if let checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } =
            entry.source
            && let Ok(parents) = outputs::parents(borrow, resource)
        {
            for parent in parents {
                crate::shared::collections::push_unique(&mut loans, parent);
            }
        }
    }
    let (mut value_cursor, mut loan_cursor, mut input_cursor) = (0, 0, 0);
    while value_cursor < live_symbols.len()
        || loan_cursor < loans.len()
        || input_cursor < inputs.len()
    {
        if let Some(symbol) = live_symbols.get(value_cursor).copied() {
            value_cursor += 1;
            for entry in frontier.iter_mut() {
                if entry.active_at_call {
                    match entry.source {
                        checked_trees::SuspensionCrossingLoanSource::Incoming {
                            identity, ..
                        } if borrow.incoming_references.get(identity).parameter_symbol()
                            == symbol =>
                        {
                            entry.retained_for_crossing = true;
                        }
                        _ if entry.stored_loan().is_valid()
                            && borrow.loans.get(entry.stored_loan()).owner_symbol == symbol =>
                        {
                            crate::shared::collections::push_unique(
                                &mut loans,
                                entry.stored_loan(),
                            );
                        }
                        _ => {}
                    }
                }
            }
        } else if let Some(loan) = loans.get(loan_cursor).copied() {
            loan_cursor += 1;
            let row = borrow.loans.get(loan);
            if let BorrowLoanLineage::Reborrow { parent_loan } = row.lineage {
                crate::shared::collections::push_unique(&mut loans, parent_loan);
            }
            for supplier in borrow.loan_suppliers.span_or_empty(row.supplying_loans) {
                crate::shared::collections::push_unique(&mut loans, *supplier);
            }
            if let Some(resources) = borrow.retained_call_inputs(loan) {
                for resource in resources {
                    if let Some(entry) = frontier.iter_mut().find(|entry| matches!(entry.source,
                        checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource: candidate } if candidate == *resource)) {
                        entry.retained_for_crossing = true;
                        retain_backing(borrow, entry.backing, live_symbols, &mut loans);
                    }
                    if let Ok(parents) = outputs::parents(borrow, *resource) {
                        for parent in parents {
                            crate::shared::collections::push_unique(&mut loans, parent);
                        }
                    }
                }
            }
            if let Some(span) = outputs::inputs(borrow, loan) {
                for input in span.handles() {
                    crate::shared::collections::push_unique(&mut inputs, (loan, input));
                }
            }
            if let Some(entry) = frontier
                .iter_mut()
                .find(|entry| entry.stored_loan() == loan)
            {
                entry.retained_for_crossing = true;
                retain_backing(borrow, entry.backing, live_symbols, &mut loans);
            }
        } else {
            let (owner, input) = inputs[input_cursor];
            input_cursor += 1;
            let Some(entry) = frontier.iter_mut().find(|entry| matches!(entry.source,
                checked_trees::SuspensionCrossingLoanSource::OutputInput { owner: candidate, input: member }
                    if candidate == owner && member == input)) else { continue; };
            entry.retained_for_crossing = true;
            retain_backing(borrow, entry.backing, live_symbols, &mut loans);
            if let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                borrow.supplied_output_inputs.get(input).permission
                && let Ok(parents) = outputs::parents(borrow, resource)
            {
                for parent in parents {
                    crate::shared::collections::push_unique(&mut loans, parent);
                }
            }
        }
    }
}

fn retain_backing(
    borrow: &BorrowFacts,
    backing: LoanBacking,
    live_symbols: &mut Vec<symbols::SymbolHandle>,
    loans: &mut Vec<Handle<BorrowLoanFact>>,
) {
    let symbol = match backing {
        LoanBacking::StateValue {
            origin:
                checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                | checked_trees::SuspensionCrossingValueOrigin::Local { symbol, .. },
        } => symbol,
        LoanBacking::IncomingReferent { identity } => {
            borrow.incoming_references.get(identity).parameter_symbol()
        }
        LoanBacking::CompletedOutput { resource } => {
            crate::shared::collections::push_unique(
                loans,
                borrow.supplied_output_resources.get(resource).loan,
            );
            return;
        }
        _ => return,
    };
    if symbol.is_valid() {
        crate::shared::collections::push_unique(live_symbols, symbol);
    }
}

fn retained_loan(
    frontier: BorrowFrontier,
    rows: &[LoanFact],
    loan: Handle<BorrowLoanFact>,
) -> Result<Handle<LoanFact>, &'static str> {
    frontier
        .loans
        .handles()
        .zip(rows)
        .find(|(_, row)| row.stored_loan() == loan)
        .map(|(handle, _)| handle)
        .ok_or("the exact input or output ancestor is not retained")
}

fn append_retained_inputs(
    borrow: &BorrowFacts,
    frontier: BorrowFrontier,
    rows: &[LoanFact],
    owner: Handle<BorrowLoanFact>,
    pending: &mut Vec<Handle<LoanFact>>,
) -> Result<(), &'static str> {
    let inputs = outputs::inputs(borrow, owner).ok_or("the output has no complete input roster")?;
    borrow
        .supplied_output_inputs
        .span(inputs)
        .ok_or("the output's captured input roster is unavailable")?;
    for input in inputs.handles() {
        let row = frontier.loans.handles().zip(rows).find(|(_, row)| matches!(row.source,
            checked_trees::SuspensionCrossingLoanSource::OutputInput { owner: candidate, input: member }
                if candidate == owner && member == input))
            .map(|(handle, _)| handle).ok_or("the completed output's original input is not retained")?;
        pending.push(row);
    }
    Ok(())
}

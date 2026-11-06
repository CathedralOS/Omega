//! A call must preserve all the data its saved references still depend on.
//! Finish the checked graph once, after retained backing closure. Later
//! lowering maps these exact rows; it does not rebuild parent or input rosters.

use super::{
    BorrowFacts, BorrowFrontier, BorrowLoanFact, BorrowLoanLineage, FlowCallFact, FlowFacts,
    LoanBacking, LoanFact, StateBorrowFact, outputs,
};
use arena::{Arena, Handle};
use checked_trees::{
    CheckedBorrowSourceOccurrence as Selected, CheckedCallBorrowLoanSource as ActualSource,
    CheckedParentBorrowResource as ParentResource,
    CheckedRetainedBorrowInputPermission as InputPermission,
    SuspensionCrossingLoanBackingPlan as BackingPlan, SuspensionCrossingLoanRelations as Relations,
    SuspensionCrossingLoanResource as Resource, SuspensionCrossingLoanSource as Source,
};
use diagnostics::Diagnostic;

type Link = Handle<LoanFact>;

struct Prepared {
    row: Link,
    resource: Resource,
    access: checked_trees::BorrowAccessKind,
    descriptor_transfer: bool,
    parent: Link,
    suppliers: Vec<Link>,
    inputs: Vec<Link>,
    backing: BackingPlan,
}

pub(in crate::checks::carry) fn publish_relations(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    state: &StateBorrowFact,
    call: &FlowCallFact,
    frontier: BorrowFrontier,
    rows: &mut Arena<LoanFact>,
    links: &mut Arena<Link>,
) -> Result<(), Diagnostic> {
    let prepared = prepare(borrow, flow, state, call, frontier, rows).map_err(|reason| {
        Diagnostic::error(format!(
            "cannot finish suspension relationships in machine {:?}, state {:?}, statement {}, call {}: {reason}",
            state.machine_symbol, state.state_symbol, call.statement_index, call.call_ordinal,
        ))
    })?;
    // No row is complete until every member of this crossing has resolved.
    // An error above leaves the whole batch unpublished, not partially ready.
    for row in prepared {
        let suppliers = links.insert_many(row.suppliers);
        let inputs = links.insert_many(row.inputs);
        rows.get_mut(row.row).relations = Some(Relations {
            resource: row.resource,
            access: row.access,
            descriptor_transfer: row.descriptor_transfer,
            parent: row.parent,
            suppliers,
            inputs,
            backing: row.backing,
        });
    }
    Ok(())
}

fn prepare(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    state: &StateBorrowFact,
    call: &FlowCallFact,
    frontier: BorrowFrontier,
    arena: &Arena<LoanFact>,
) -> Result<Vec<Prepared>, &'static str> {
    let rows = arena
        .span(frontier.loans)
        .ok_or("the crossing loan roster is unavailable")?;
    let loan_row = |loan: Handle<BorrowLoanFact>| -> Result<Link, &'static str> {
        if !borrow.state_owns_loan(state, loan) {
            return Err("the crossing relation names a foreign or unavailable loan");
        }
        let mut matching = frontier
            .loans
            .handles()
            .zip(rows)
            .filter(|(_, row)| row.retained_for_crossing && row.stored_loan() == loan);
        let (handle, _) = matching
            .next()
            .ok_or("the checked relation's loan is not retained")?;
        if matching.next().is_some() {
            return Err("the checked relation's loan occurrence is ambiguous");
        }
        Ok(handle)
    };
    let input_row = |owner, input| -> Result<Link, &'static str> {
        let mut matching = frontier.loans.handles().zip(rows).filter(|(_, row)| {
            row.retained_for_crossing
                && matches!(row.source,
                Source::RetainedInput { owner: candidate, input: member }
                    if candidate == owner && member == input)
        });
        let (handle, _) = matching
            .next()
            .ok_or("the original captured input is not retained")?;
        if matching.next().is_some() {
            return Err("the original captured input occurrence is ambiguous");
        }
        Ok(handle)
    };
    let mut prepared = Vec::new();
    for (handle, row) in frontier
        .loans
        .handles()
        .zip(rows)
        .filter(|(_, row)| row.retained_for_crossing)
    {
        if row.relations.is_some() {
            return Err("crossing loan relations must be published exactly once");
        }
        let mut parent = Handle::invalid();
        let mut suppliers = Vec::new();
        let mut inputs = Vec::new();
        let mut descriptor_transfer = false;
        let (resource, access) = match &row.source {
            Source::Incoming { identity, access } => {
                if !borrow.state_owns_incoming_reference(state, *identity)
                    || row.backing
                        != (LoanBacking::IncomingReferent {
                            identity: *identity,
                        })
                {
                    return Err("the incoming permission has no exact checked backing");
                }
                (Resource::IncomingReference, access.clone())
            }
            Source::LifetimeDependency { loan, resource } => {
                if outputs::dependency(borrow, *loan) != Some(*resource) {
                    return Err("the lifetime obligation lost its checked resource");
                }
                let _ = loan_row(*loan)?;
                let loan_fact = borrow.loans.get(*loan);
                for source in borrow
                    .loan_suppliers
                    .span(loan_fact.supplying_loans)
                    .ok_or("the lifetime obligation's supplier roster is unavailable")?
                {
                    suppliers.push(loan_row(*source)?);
                }
                for input in outputs::inputs(borrow, *loan)
                    .ok_or("the lifetime obligation's input roster is unavailable")?
                    .handles()
                {
                    outputs::validate_input(borrow, flow, state, *loan, input)?;
                    inputs.push(input_row(*loan, input)?);
                }
                (Resource::LifetimeDependency, loan_fact.kind.clone())
            }
            Source::RetainedInput { owner, input } => {
                outputs::validate_input(borrow, flow, state, *owner, *input)?;
                let _ = loan_row(*owner)?;
                let input = borrow.retained_borrow_inputs.get(*input);
                let _ = borrow
                    .call_loan_paths
                    .span(input.capture_path)
                    .ok_or("the retained input has no complete checked capture path")?;
                let access = match input.permission {
                    InputPermission::Borrowed { resource } => {
                        for source in outputs::parents(borrow, resource)? {
                            suppliers.push(loan_row(source)?);
                        }
                        borrow.call_loan_resources.get(resource).access.clone()
                    }
                    InputPermission::Incoming { access, .. } => access.into(),
                };
                (Resource::OutputInput, access)
            }
            Source::Stored { loan, resource } => {
                let _ = loan_row(*loan)?;
                let loan_fact = borrow.loans.get(*loan);
                let sources = borrow
                    .loan_suppliers
                    .span(loan_fact.supplying_loans)
                    .ok_or("the stored loan's supplier roster is unavailable")?;
                descriptor_transfer = loan_fact.lineage == BorrowLoanLineage::UnretainedDerived
                    && (!sources.is_empty() || borrow.retained_call_inputs(*loan).is_some());
                for source in sources {
                    suppliers.push(loan_row(*source)?);
                }
                // Normal stored references retain their own exact input rows.
                // They do not become a new physical loan for every lifetime input.
                if let Some(span) = borrow.retained_call_input_span(*loan) {
                    let _ = borrow
                        .retained_borrow_inputs
                        .span(span)
                        .ok_or("the stored call's complete original input roster is unavailable")?;
                    for input in span.handles() {
                        outputs::validate_input(borrow, flow, state, *loan, input)?;
                        suppliers.push(input_row(*loan, input)?);
                    }
                }
                let kind = match resource {
                    ParentResource::DirectRoot { resource }
                        if borrow.direct_loan_resources.is_valid(*resource) =>
                    {
                        let original = borrow.direct_loan_resources.get(*resource);
                        if original.loan != *loan
                            || original.parent_lifetime.machine_symbol != state.machine_symbol
                            || original.parent_lifetime.state_symbol != state.state_symbol
                            || original.parent_lifetime.root != loan_fact.root
                        {
                            return Err("the direct root lost its exact checked lifetime resource");
                        }
                        Resource::DirectRoot
                    }
                    ParentResource::Reborrow { resource }
                        if borrow.reborrow_loan_resources.is_valid(*resource) =>
                    {
                        let original = borrow.reborrow_loan_resources.get(*resource);
                        if original.loan != *loan
                            || original.machine_symbol != state.machine_symbol
                            || original.state_symbol != state.state_symbol
                            || loan_fact.lineage
                                != (BorrowLoanLineage::Reborrow {
                                    parent_loan: original.parent_loan,
                                })
                        {
                            return Err("the reborrow lost its exact checked parent resource");
                        }
                        parent = loan_row(original.parent_loan)?;
                        Resource::Reborrow
                    }
                    ParentResource::SuppliedOutput { resource } => {
                        outputs::validate_output(borrow, flow, state, *resource)?;
                        let output = borrow.supplied_output_resources.get(*resource);
                        if output.loan != *loan {
                            return Err("the completed output names another loan occurrence");
                        }
                        let _ = output
                            .capture_path
                            .and_then(|path| borrow.call_loan_paths.span(path))
                            .ok_or("the completed output has no checked static capture path")?;
                        for input in output.inputs.handles() {
                            outputs::validate_input(borrow, flow, state, *loan, input)?;
                            inputs.push(input_row(*loan, input)?);
                        }
                        Resource::CompletedOutput
                    }
                    _ if borrow.retained_call_input_span(*loan).is_some()
                        || crate::checks::borrows::transport_suppliers(borrow, state, *loan)
                            .is_some() =>
                    {
                        if !descriptor_transfer {
                            return Err(
                                "the descriptor transfer has no complete checked source generation",
                            );
                        }
                        Resource::DescriptorTransfer
                    }
                    _ => return Err("the stored loan's checked resource is unavailable"),
                };
                (kind, loan_fact.kind.clone())
            }
            Source::CapturedActual { resource, selected } => {
                if !borrow.call_loan_resources.is_valid(*resource) {
                    return Err("the captured actual has no checked borrowing resource");
                }
                let actual = borrow.call_loan_resources.get(*resource);
                if actual.machine_symbol != state.machine_symbol
                    || actual.state_symbol != state.state_symbol
                    || !flow.control.calls.is_valid(actual.call)
                    || flow.control.calls.get(actual.call) != call
                    || !frontier
                        .operands
                        .handles()
                        .any(|operand| operand == actual.owner_operand)
                {
                    return Err(
                        "the current actual lost its exact original invocation and operand",
                    );
                }
                let _ = borrow
                    .call_loan_paths
                    .span(actual.capture_path)
                    .ok_or("the captured actual has no complete checked static path")?;
                let kind = match &actual.source {
                    ActualSource::StoredLoan { loan } => {
                        if *selected != (Selected::SelectedLoan { loan: *loan }) {
                            return Err("the stored actual names another selected source");
                        }
                        suppliers.push(loan_row(*loan)?);
                        Resource::DescriptorTransfer
                    }
                    ActualSource::CallAccess { parents, .. } => {
                        for source in borrow
                            .call_loan_parents
                            .span(*parents)
                            .ok_or("the actual's complete supplier roster is unavailable")?
                        {
                            suppliers.push(loan_row(*source)?);
                        }
                        match *selected {
                            Selected::DirectStorage => Resource::DirectRoot,
                            Selected::FixedIncoming { identity } => {
                                if !borrow.state_owns_incoming_reference(state, identity)
                                    || row.backing != (LoanBacking::IncomingReferent { identity })
                                {
                                    return Err(
                                        "the fixed incoming actual names another original backing",
                                    );
                                }
                                Resource::DirectRoot
                            }
                            Selected::SelectedLoan { loan } => {
                                parent = loan_row(loan)?;
                                Resource::Reborrow
                            }
                            Selected::Unknown => {
                                return Err("the actual has no complete selected physical source");
                            }
                        }
                    }
                };
                (kind, actual.access.clone())
            }
        };
        let backing = match row.backing {
            LoanBacking::StateValue { origin } => BackingPlan::StateValue { origin },
            LoanBacking::IncomingReferent { identity } => {
                if !borrow.state_owns_incoming_reference(state, identity) {
                    return Err("the backing belongs to another invocation's entry namespace");
                }
                if matches!(row.source, Source::RetainedInput { .. }) {
                    BackingPlan::CapturedInput
                } else {
                    BackingPlan::IncomingReferent { identity }
                }
            }
            LoanBacking::CompletedOutput { resource } => {
                outputs::validate_output(borrow, flow, state, resource)?;
                BackingPlan::CompletedOutput {
                    loan: loan_row(borrow.supplied_output_resources.get(resource).loan)?,
                }
            }
            LoanBacking::InputDependencies => BackingPlan::InputDependencies,
            LoanBacking::Unresolved => return Err("the checked crossing backing is unresolved"),
        };
        prepared.push(Prepared {
            row: handle,
            resource,
            access,
            descriptor_transfer,
            parent,
            suppliers,
            inputs,
            backing,
        });
    }
    Ok(prepared)
}

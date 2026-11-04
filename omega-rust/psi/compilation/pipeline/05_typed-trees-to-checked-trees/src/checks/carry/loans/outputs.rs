//! A saved helper output keeps every promised input borrowed, even when its
//! pointer came from somewhere else. Follow the already checked completion and
//! input records; the output variable's current contents cannot replace them.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowFacts, BorrowLoanFact, CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource,
    CheckedOutputBorrowInput, CheckedOutputLifetimeDependencyResource,
    CheckedOutputLifetimeDependencySource, CheckedSuppliedOutputBorrowResource, FlowFacts,
    IncomingReferenceIdentity, StateBorrowFact,
};

pub(super) fn dependency(
    borrow: &BorrowFacts,
    loan: Handle<BorrowLoanFact>,
) -> Option<Handle<CheckedOutputLifetimeDependencyResource>> {
    let mut matches = borrow
        .output_lifetime_dependencies
        .iter()
        .filter(|(_, row)| row.loan == loan);
    let (handle, _) = matches.next()?;
    matches.next().is_none().then_some(handle)
}

pub(super) fn output(
    borrow: &BorrowFacts,
    identity: Handle<IncomingReferenceIdentity>,
) -> Option<Handle<CheckedSuppliedOutputBorrowResource>> {
    if !borrow.incoming_references.is_valid(identity) {
        return None;
    }
    let mut matches = borrow
        .supplied_output_resources
        .iter()
        .filter(|(_, row)| row.identity == identity);
    let (handle, _) = matches.next()?;
    matches.next().is_none().then_some(handle)
}

pub(super) fn inputs(
    borrow: &BorrowFacts,
    loan: Handle<BorrowLoanFact>,
) -> Option<HandleSpan<CheckedOutputBorrowInput>> {
    if let Some(resource) = dependency(borrow, loan) {
        return Some(borrow.output_lifetime_dependencies.get(resource).inputs);
    }
    let resource = output(borrow, borrow.loans.get(loan).incoming_reference)?;
    let output = borrow.supplied_output_resources.get(resource);
    (output.loan == loan).then_some(output.inputs)
}

pub(super) fn parents(
    borrow: &BorrowFacts,
    resource: Handle<CheckedCallBorrowLoanResource>,
) -> Result<Vec<Handle<BorrowLoanFact>>, &'static str> {
    if !borrow.call_loan_resources.is_valid(resource) {
        return Err("the completed input has no checked borrowing resource");
    }
    match borrow.call_loan_resources.get(resource).source {
        CheckedCallBorrowLoanSource::StoredLoan { loan } => Ok(vec![loan]),
        CheckedCallBorrowLoanSource::CallAccess { parents, .. } => borrow
            .call_loan_parents
            .span(parents)
            .map(|parents| parents.to_vec())
            .ok_or("the completed input's captured parent roster is unavailable"),
    }
}

pub(super) fn validate_output(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    state: &StateBorrowFact,
    resource: Handle<CheckedSuppliedOutputBorrowResource>,
) -> Result<(), &'static str> {
    if !borrow.supplied_output_resources.is_valid(resource) {
        return Err("the completed output resource is unavailable");
    }
    let output = borrow.supplied_output_resources.get(resource);
    if output.machine_symbol != state.machine_symbol
        || output.state_symbol != state.state_symbol
        || !borrow.state_owns_loan(state, output.loan)
        || borrow.loans.get(output.loan).incoming_reference != output.identity
        || crate::borrow::supplied_outputs::normal_call(borrow, flow, output.loan)
            != Some(output.call)
        || borrow.supplied_output_inputs.span(output.inputs).is_none()
        || flow
            .control
            .operand_retained_suppliers
            .span(output.lifetime_sources)
            .is_none()
    {
        return Err("the output has no exact normally completed invocation and input roster");
    }
    Ok(())
}

/// A transferred dependency can copy its input records. Authenticate those
/// records through every original completion, without choosing one alternative.
fn completions(
    borrow: &BorrowFacts,
    resource: Handle<CheckedOutputLifetimeDependencyResource>,
    active: &mut Vec<Handle<CheckedOutputLifetimeDependencyResource>>,
    outputs: &mut Vec<Handle<CheckedSuppliedOutputBorrowResource>>,
) -> Result<(), &'static str> {
    if !borrow.output_lifetime_dependencies.is_valid(resource) || active.contains(&resource) {
        return Err("the output dependency has an unavailable or cyclic source");
    }
    active.push(resource);
    match borrow.output_lifetime_dependencies.get(resource).source {
        CheckedOutputLifetimeDependencySource::Completion { output } => {
            crate::shared::collections::push_unique(outputs, output);
        }
        CheckedOutputLifetimeDependencySource::Transfer { suppliers } => {
            let suppliers = borrow
                .output_dependency_suppliers
                .span(suppliers)
                .filter(|suppliers| !suppliers.is_empty())
                .ok_or("the output dependency has no complete supplying roster")?;
            for supplier in suppliers {
                completions(borrow, *supplier, active, outputs)?;
            }
        }
    }
    active.pop();
    Ok(())
}

pub(super) fn validate_input(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    state: &StateBorrowFact,
    owner: Handle<BorrowLoanFact>,
    input: Handle<CheckedOutputBorrowInput>,
) -> Result<(), &'static str> {
    if !borrow.state_owns_loan(state, owner)
        || !inputs(borrow, owner).is_some_and(|span| span.handles().any(|member| member == input))
        || !borrow.supplied_output_inputs.is_valid(input)
    {
        return Err("the retained input belongs to another output generation");
    }
    let mut outputs = Vec::new();
    if let Some(resource) = dependency(borrow, owner) {
        completions(borrow, resource, &mut Vec::new(), &mut outputs)?;
    } else {
        outputs.push(
            output(borrow, borrow.loans.get(owner).incoming_reference)
                .ok_or("the retained input has no completed output owner")?,
        );
    }
    let input = borrow.supplied_output_inputs.get(input);
    let mut admitted = false;
    for resource in outputs {
        validate_output(borrow, flow, state, resource)?;
        let output = borrow.supplied_output_resources.get(resource);
        if !output
            .lifetime_sources
            .handles()
            .any(|source| source == input.source)
        {
            continue;
        }
        // The constructor validates the selected member's type and access. A
        // transferred record must still be one of this completion's records.
        if !borrow
            .supplied_output_inputs
            .span_or_empty(output.inputs)
            .contains(input)
        {
            continue;
        }
        match input.permission {
            checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } => {
                if !borrow.call_loan_resources.is_valid(resource) {
                    return Err("the completed input's borrowing permission is unavailable");
                }
                let permission = borrow.call_loan_resources.get(resource);
                if permission.machine_symbol != state.machine_symbol
                    || permission.state_symbol != state.state_symbol
                    || permission.parent_lifetime.machine_symbol != state.machine_symbol
                    || permission.parent_lifetime.state_symbol != state.state_symbol
                    || permission.call != output.call
                    || !flow
                        .control
                        .calls
                        .get(output.call)
                        .argument_operands()
                        .handles()
                        .any(|operand| operand == permission.owner_operand)
                {
                    return Err("the completed input has no original captured invocation");
                }
            }
            checked_trees::CheckedOutputBorrowInputPermission::Incoming { identity, .. } => {
                if !borrow.state_owns_incoming_reference(state, identity) {
                    return Err("the completed input has no original entry namespace");
                }
            }
        }
        admitted = true;
    }
    admitted
        .then_some(())
        .ok_or("the retained input is absent from its original completion")
}

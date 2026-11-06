//! A saved pointer keeps the particular borrow that supplied its address.
//! Follow that pointer's recorded transfer alternatives, rather than choosing
//! from the larger set of loans that must remain alive. The caller retains
//! every lifetime duty separately.

use arena::Handle;
use checked_trees::{
    BorrowAccessKind, BorrowLoanFact, BorrowLoanLineage, CapturedPlace, CheckFacts,
    CheckedBorrowSourceOccurrence as Selected, CheckedRetainedBorrowInputOrigin as InputOrigin,
    CheckedRetainedBorrowInputPermission as InputPermission, StateBorrowFact,
};

/// One physical alternative stays paired with the backing selected by the
/// original permission collector. This is not a product of all lifetime
/// suppliers and all possible backings.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PhysicalSource {
    pub(crate) selected: Selected,
    pub(crate) backing: CapturedPlace,
}

/// Resolve the selected occurrence of ONE already checked physical backing.
/// Ordinary transfer edges retain the same carried backing/projection; only
/// their terminal occurrence changes. A normally completed stored generation
/// is itself a real terminal after its complete original inputs and receipt
/// are authenticated. Missing alternatives never become direct storage.
pub(crate) fn resolve(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &StateBorrowFact,
    selected: Selected,
    backing: CapturedPlace,
    access: &BorrowAccessKind,
) -> Result<Vec<PhysicalSource>, &'static str> {
    if !backing.root.is_valid() {
        return Err("the selected physical source has no captured backing");
    }
    let root = match selected {
        Selected::Unknown => return Err("the physical backing has no selected source occurrence"),
        Selected::DirectStorage => return Ok(vec![PhysicalSource { selected, backing }]),
        Selected::FixedIncoming { identity } => {
            if !facts.borrow.state_owns_incoming_reference(state, identity) {
                return Err("the selected entry source belongs to another invocation");
            }
            let incoming = facts.borrow.incoming_references.get(identity);
            // The source collector can retain an explicit parameter-referent
            // grant beside its canonical written coordinate. That spelling has
            // no incoming field, but still names this exact parameter root.
            let canonical_parameter = !backing.incoming_reference.is_valid()
                && incoming.owner_path.is_empty()
                && matches!(incoming.origin,
                    checked_trees::IncomingReferenceOrigin::ParameterReferent { symbol }
                        if backing.root == facts::PlaceRoot::Symbol(symbol));
            if incoming.place_root() != backing.root
                || (backing.incoming_reference != identity && !canonical_parameter)
            {
                return Err("the selected entry source disagrees with its exact captured backing");
            }
            return Ok(vec![PhysicalSource { selected, backing }]);
        }
        Selected::SelectedLoan { loan } => loan,
    };
    let borrow = &facts.borrow;
    let flow = facts
        .flow
        .control
        .unique_state(state.machine_symbol, state.state_symbol)
        .ok_or("the selected physical source has no unique flow owner")?;
    // Use the existing complete history validator, which understands normal
    // saved generations and fixed input terminals. The access-exemption query
    // computes common ancestors instead and is not a physical-parent test.
    let closure = crate::borrow::retained_loan_closure(
        borrow,
        state,
        root,
        &crate::borrow::RetainedLoanSourceScope {
            program,
            contexts: &facts.flow.contexts,
            control: &facts.flow.control,
            lifetimes: &facts.flow.borrow_lifetimes,
            calls: crate::borrow::lifetime_contract::SourceCallScope {
                machine: state.machine_symbol,
                state: state.state_symbol,
                calls: flow.calls,
            },
        },
    )?;
    for source in closure {
        let row = borrow.loans.get(source);
        if borrow.retained_call_input_span(source).is_some()
            || (row.lineage == BorrowLoanLineage::UnretainedDerived
                && row.formation == checked_trees::BorrowLoanFormation::ReturnedDescriptor
                && row.established_by_call.is_valid())
        {
            // Missing/duplicate original-input associations on a known normal
            // stored generation must refuse, not switch to ordinary transfer.
            authenticate_stored_generation(program, facts, state, source)?;
        }
    }
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut complete = Vec::new();
    let mut terminals = Vec::new();
    while let Some((handle, closing)) = pending.pop() {
        if closing {
            if active.pop() != Some(handle) {
                return Err("the selected physical source lost its exact traversal predecessor");
            }
            complete.push(handle);
            continue;
        }
        if complete.contains(&handle) {
            continue;
        }
        if active.contains(&handle) || !borrow.state_owns_loan(state, handle) {
            return Err("the selected physical source has a cyclic or foreign supplier");
        }
        let row = borrow.loans.get(handle);
        if row.kind.direct_reborrow_effect(access).is_none()
            || !row.root.is_valid()
            || borrow.access_segments.span(row.segments).is_none()
            || borrow
                .access_segments
                .span(row.incoming_owner_path)
                .is_none()
            || borrow.owner_segments.span(row.owner_path).is_none()
        {
            return Err("the selected physical supplier lost its access or captured path");
        }
        let suppliers = borrow
            .loan_suppliers
            .span(row.supplying_loans)
            .ok_or("the selected physical source has no complete supplier roster")?;
        // Validate handles before transport_suppliers reads their rows.
        if !suppliers
            .iter()
            .all(|source| borrow.state_owns_loan(state, *source))
        {
            return Err("the selected physical source contains an unavailable supplier");
        }
        active.push(handle);
        pending.push((handle, true));
        let terminal = match row.lineage {
            BorrowLoanLineage::DirectRoot
            | BorrowLoanLineage::Reborrow { .. }
            | BorrowLoanLineage::SuppliedOutput => {
                if let BorrowLoanLineage::Reborrow { parent_loan } = row.lineage {
                    if !borrow.state_owns_loan(state, parent_loan) {
                        return Err("the physical reborrow has no original parent occurrence");
                    }
                    let parent = borrow.loans.get(parent_loan);
                    if parent.owner_symbol != row.source_owner_symbol
                        || parent.kind.direct_reborrow_effect(&row.kind).is_none()
                    {
                        return Err(
                            "the physical reborrow disagrees with its original parent access",
                        );
                    }
                }
                if row.lineage == BorrowLoanLineage::SuppliedOutput
                    && borrow.supplied_output_call(handle).is_none()
                {
                    return Err("the physical output has no original normal completion");
                }
                // Fixed parameter referents keep their explicit entry role.
                Some(super::aliases::selected_loan_source(facts, flow, handle))
            }
            BorrowLoanLineage::UnretainedDerived => {
                if borrow.retained_call_input_span(handle).is_some()
                    || (row.formation == checked_trees::BorrowLoanFormation::ReturnedDescriptor
                        && row.established_by_call.is_valid())
                {
                    Some(Selected::SelectedLoan { loan: handle })
                } else if crate::borrow::retained_input_terminal(borrow, state, row) {
                    // Only this positive fixed-entry producer can terminate an
                    // empty transfer. Empty saved-source lists otherwise refuse.
                    Some(Selected::FixedIncoming {
                        identity: row.incoming_reference,
                    })
                } else {
                    let sources = crate::borrow::transport_suppliers(borrow, state, handle).ok_or(
                        "the physical transfer has no authenticated complete source edges",
                    )?;
                    pending.extend(sources.iter().rev().map(|source| (*source, false)));
                    None
                }
            }
            BorrowLoanLineage::LifetimeDependency => {
                return Err("a lifetime dependency cannot supply a physical pointer");
            }
        };
        if let Some(selected) = terminal {
            if selected == Selected::Unknown {
                return Err("the selected terminal has no authenticated physical role");
            }
            let source = PhysicalSource {
                selected,
                backing: backing.clone(),
            };
            if !terminals.contains(&source) {
                terminals.push(source);
            }
        }
    }
    if terminals.is_empty() {
        Err("the selected physical source has no complete terminal alternatives")
    } else {
        Ok(terminals)
    }
}

/// The input association was published as one complete batch by
/// stored_call_inputs::install. Rejoin it to the original normal completion;
/// neither an input debt nor a destination with equal bytes is its receipt.
fn authenticate_stored_generation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &StateBorrowFact,
    loan: Handle<BorrowLoanFact>,
) -> Result<(), &'static str> {
    let borrow = &facts.borrow;
    let control = &facts.flow.control;
    let inputs = borrow
        .retained_call_inputs(loan)
        .ok_or("the selected saved generation has no complete original input association")?;
    let receipts = validation::completed_storage::loan_call_descriptors(
        program,
        facts,
        state.machine_symbol,
        state.state_symbol,
        loan,
    )?
    .filter(|rows| !rows.is_empty())
    .ok_or("the selected saved generation has no exact completed store receipt")?;
    let flow = control
        .unique_state(state.machine_symbol, state.state_symbol)
        .ok_or("the saved generation has no unique flow owner")?;
    for input in inputs {
        let InputOrigin::CompletedStore { capture } = input.origin else {
            return Err("the saved generation input is not from its completed store");
        };
        if input.machine_symbol != state.machine_symbol
            || input.state_symbol != state.state_symbol
            || !receipts
                .iter()
                .any(|receipt| receipt.loan == loan && receipt.capture == capture)
            || !control.calls.is_valid(input.call)
            || !flow.calls.handles().any(|call| call == input.call)
        {
            return Err("the saved generation input lost its exact original completion");
        }
        let invocation = control.calls.get(input.call);
        let actual = invocation
            .argument_operands()
            .handles()
            .nth(input.parameter_index)
            .ok_or("the saved generation input is absent from its original argument tuple")?;
        let path = borrow
            .call_loan_paths
            .span(input.capture_path)
            .ok_or("the saved generation input lost its complete captured path")?;
        match input.permission {
            InputPermission::Borrowed { resource } => {
                if !borrow.call_loan_resources.is_valid(resource) {
                    return Err("the saved generation input has no original borrowing resource");
                }
                let original = borrow.call_loan_resources.get(resource);
                if original.machine_symbol != state.machine_symbol
                    || original.state_symbol != state.state_symbol
                    || original.call != input.call
                    || original.owner_operand != actual
                    || original.parameter_index != input.parameter_index
                    || borrow.call_loan_paths.span(original.capture_path) != Some(path)
                    || original.capture_source != Some(input.source_kind)
                    || original
                        .access
                        .direct_reborrow_effect(&borrow.loans.get(loan).kind)
                        .is_none()
                    || original.parent_lifetime.machine_symbol != state.machine_symbol
                    || original.parent_lifetime.state_symbol != state.state_symbol
                {
                    return Err(
                        "the saved generation input disagrees with its original permission",
                    );
                }
            }
            InputPermission::Incoming {
                identity,
                access,
                root,
                path,
                owner_path,
            } => {
                if !borrow.state_owns_incoming_reference(state, identity)
                    || BorrowAccessKind::from(access)
                        .direct_reborrow_effect(&borrow.loans.get(loan).kind)
                        .is_none()
                    || borrow.incoming_references.get(identity).place_root() != root
                    || borrow.call_loan_backing_paths.span(path).is_none()
                    || borrow.call_loan_backing_paths.span(owner_path).is_none()
                    || !receipts
                        .iter()
                        .any(|receipt| receipt.call == input.call && receipt.capture == capture)
                {
                    return Err("the saved generation input lost its exact entry source");
                }
            }
        }
    }
    if receipts.iter().any(|receipt| receipt.loan != loan || !inputs.iter().any(|input|
        matches!(input.origin, InputOrigin::CompletedStore { capture } if capture == receipt.capture))) {
        return Err("the saved generation dropped an original completed-store alternative");
    }
    Ok(())
}

/// A stored generation can be the physical parent while its original caller
/// supplied the borrowing permission. Follow only that generation's exact
/// completed-store input association. Full unrelated lifetime parents never
/// choose the permission lending a returned view.
pub(crate) fn permission_sources(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &StateBorrowFact,
    selected: Selected,
    backing: CapturedPlace,
    access: &BorrowAccessKind,
) -> Result<Vec<PhysicalSource>, &'static str> {
    let mut pending = vec![(PhysicalSource { selected, backing }, false)];
    let mut active = Vec::new();
    let mut complete = Vec::new();
    let mut permissions = Vec::new();
    while let Some((source, closing)) = pending.pop() {
        if closing {
            if active.pop().as_ref() != Some(&source) {
                return Err("the returned permission lost its exact transfer predecessor");
            }
            complete.push(source);
            continue;
        }
        if complete.contains(&source) {
            continue;
        }
        if active.contains(&source) {
            return Err("the returned permission has a cyclic original-input association");
        }
        active.push(source.clone());
        pending.push((source.clone(), true));
        let physical = resolve(
            program,
            facts,
            state,
            source.selected,
            source.backing,
            access,
        )?;
        for source in physical {
            let inputs = match source.selected {
                Selected::SelectedLoan { loan } => facts.borrow.retained_call_inputs(loan),
                _ => None,
            };
            let Some(inputs) = inputs else {
                if !permissions.contains(&source) {
                    permissions.push(source);
                }
                continue;
            };
            // resolve authenticated the complete unique association and every
            // original store/call/resource. Each row came from that normal
            // descriptor's actual sources, not a widened lifetime-parent set.
            for input in inputs {
                let next = match input.permission {
                    InputPermission::Borrowed { resource } => {
                        let original = facts.borrow.call_loan_resources.get(resource);
                        if original.access.direct_reborrow_effect(access).is_none() {
                            return Err("the original input cannot lend the returned permission");
                        }
                        let selected = match original.source {
                            checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                                Selected::SelectedLoan { loan }
                            }
                            checked_trees::CheckedCallBorrowLoanSource::CallAccess {
                                selected,
                                ..
                            } => selected,
                        };
                        PhysicalSource {
                            selected,
                            backing: CapturedPlace {
                                root: original.parent_lifetime.root,
                                incoming_reference: original.backing_incoming_reference,
                                incoming_owner_path: facts
                                    .borrow
                                    .call_loan_backing_paths
                                    .span(original.backing_incoming_owner_path)
                                    .ok_or("the original input lost its incoming backing path")?
                                    .to_vec(),
                                segments: facts
                                    .borrow
                                    .call_loan_backing_paths
                                    .span(original.backing_path)
                                    .ok_or("the original input lost its captured backing path")?
                                    .to_vec(),
                            },
                        }
                    }
                    InputPermission::Incoming {
                        identity,
                        access: incoming_access,
                        root,
                        path,
                        owner_path,
                    } => {
                        if BorrowAccessKind::from(incoming_access)
                            .direct_reborrow_effect(access)
                            .is_none()
                        {
                            return Err(
                                "the retained entry source cannot lend the returned permission",
                            );
                        }
                        PhysicalSource {
                            selected: Selected::FixedIncoming { identity },
                            backing: CapturedPlace {
                                incoming_reference: identity,
                                root,
                                incoming_owner_path: facts
                                    .borrow
                                    .call_loan_backing_paths
                                    .span(owner_path)
                                    .ok_or(
                                        "the retained entry source lost its incoming owner path",
                                    )?
                                    .to_vec(),
                                segments: facts
                                    .borrow
                                    .call_loan_backing_paths
                                    .span(path)
                                    .ok_or(
                                        "the retained entry source lost its captured backing path",
                                    )?
                                    .to_vec(),
                            },
                        }
                    }
                };
                pending.push((next, false));
            }
        }
    }
    if permissions.is_empty() {
        Err("the returned view has no complete original permission alternatives")
    } else {
        Ok(permissions)
    }
}

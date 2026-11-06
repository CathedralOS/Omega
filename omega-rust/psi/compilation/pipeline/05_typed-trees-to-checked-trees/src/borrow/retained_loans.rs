//! A saved reference keeps the obligations of the descriptor it captured.
//! Follow its recorded parents, suppliers and lifetime cohort; a different
//! descriptor pointing at the same data is not part of that history.

use arena::Handle;
use checked_trees::{BorrowFacts, BorrowLoanFact, BorrowLoanLineage, StateBorrowFact};

pub(crate) struct RetainedLoanSourceScope<'a> {
    pub program: &'a dyn validation::CheckingInput,
    pub contexts: &'a checked_trees::FlowContextFacts,
    pub control: &'a checked_trees::FlowControlFacts,
    pub lifetimes: &'a checked_trees::FlowBorrowLifetimeFacts,
    pub calls: super::lifetime_contract::SourceCallScope,
}

pub(crate) fn same_carrier_generation(left: &BorrowLoanFact, right: &BorrowLoanFact) -> bool {
    right.owner_symbol.is_valid()
        && right.storage_owner.is_valid()
        && left.owner_symbol == right.owner_symbol
        && left.storage_owner == right.storage_owner
        && left.statement_index == right.statement_index
        && left.established_by_call == right.established_by_call
}

pub(crate) fn same_carrier_occurrence(
    borrow: &BorrowFacts,
    left: &BorrowLoanFact,
    right: &BorrowLoanFact,
) -> bool {
    same_carrier_generation(left, right)
        && borrow.loan_owner_path(left) == borrow.loan_owner_path(right)
}

/// Every possible obligation survives a copied descriptor. This is a union
/// of duties, not the common ancestry which may exempt an access conflict.
/// Only a physical descriptor brings its own lifetime cohort into the walk;
/// a captured debt follows its original suppliers without reviving another
/// descriptor that happened to retain the same data.
pub(crate) fn retained_loan_closure(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    root: Handle<BorrowLoanFact>,
    sources: &RetainedLoanSourceScope<'_>,
) -> Result<Vec<Handle<BorrowLoanFact>>, &'static str> {
    let mut completed = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(root, false)];
    while let Some((handle, closing)) = pending.pop() {
        if completed.contains(&handle) {
            continue;
        }
        if !borrow.state_owns_loan(state, handle) {
            return Err("the retained obligation has a foreign or unavailable loan");
        }
        if closing {
            if active.pop() != Some(handle) {
                return Err("the retained obligation traversal has no exact predecessor");
            }
            completed.push(handle);
            continue;
        }
        if active.contains(&handle) {
            return Err("the retained obligation has a cyclic supplier history");
        }
        let loan = borrow.loans.get(handle);
        if !loan.root.is_valid()
            || borrow.access_segments.span(loan.segments).is_none()
            || borrow.owner_segments.span(loan.owner_path).is_none()
        {
            return Err("the retained obligation has an incomplete captured source");
        }
        let suppliers = borrow
            .loan_suppliers
            .span(loan.supplying_loans)
            .ok_or("the retained obligation has no complete supplier roster")?;
        if loan.lineage == BorrowLoanLineage::UnretainedDerived
            && suppliers.is_empty()
            && !super::retained_input_terminal(borrow, state, loan)
        {
            let completed = validation::completed_storage::loan_call_descriptors_at_calls(
                sources.program,
                borrow,
                sources.contexts,
                sources.control,
                sources.calls.machine,
                sources.calls.state,
                sources.calls.calls,
                handle,
            )?;
            if completed.is_none_or(|rows| rows.is_empty()) {
                return Err("the retained transfer has no authenticated original source");
            }
        }
        active.push(handle);
        pending.push((handle, true));
        pending.extend(suppliers.iter().rev().map(|source| (*source, false)));
        if let BorrowLoanLineage::Reborrow { parent_loan } = loan.lineage {
            pending.push((parent_loan, false));
        }
        if loan.lineage != BorrowLoanLineage::LifetimeDependency {
            for (dependency, candidate) in borrow.state_loans(state) {
                if candidate.lineage != BorrowLoanLineage::LifetimeDependency
                    || !same_carrier_generation(loan, candidate)
                {
                    continue;
                }
                if borrow.owner_segments.span(candidate.owner_path).is_none() {
                    return Err("the retained descriptor has an unavailable lifetime cohort");
                }
                if same_carrier_occurrence(borrow, loan, candidate) {
                    pending.push((dependency, false));
                }
            }
        }
    }
    validate_incoming_sources(borrow, state, &completed, sources)?;
    Ok(completed)
}

fn validate_incoming_sources(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    closure: &[Handle<BorrowLoanFact>],
    sources: &RetainedLoanSourceScope<'_>,
) -> Result<(), &'static str> {
    use checked_trees::{FlowOperandCompletion, IncomingReferenceOrigin as Origin};
    if sources.calls.machine != state.machine_symbol || sources.calls.state != state.state_symbol {
        return Err("the retained sources belong to another invocation");
    }
    let mut checked = Vec::new();
    for handle in closure {
        let identity = borrow.loans.get(*handle).incoming_reference;
        if !identity.is_valid() || checked.contains(&identity) {
            continue;
        }
        if !borrow.incoming_references.is_valid(identity) {
            return Err("the retained source has an unavailable incoming identity");
        }
        let incoming = borrow.incoming_references.get(identity);
        if incoming.machine_symbol != state.machine_symbol
            || incoming.state_symbol != state.state_symbol
            || borrow.owner_segments.span(incoming.owner_path).is_none()
        {
            return Err("the retained incoming source belongs to another invocation");
        }
        let known = match incoming.origin {
            Origin::Parameter { .. } | Origin::ParameterReferent { .. } => {
                borrow.state_owns_incoming_reference(state, identity)
            }
            Origin::Output { .. } => closure.iter().any(|candidate| {
                borrow.loans.get(*candidate).incoming_reference == identity
                    && borrow.output_origin_call(*candidate).is_some()
            }),
            Origin::DescriptorLoad { .. } => {
                let loaded =
                    validation::reference_result_custody::completed_descriptor_load_at_activations(
                        sources.program,
                        borrow,
                        sources.control,
                        state.machine_symbol,
                        state.state_symbol,
                        sources.lifetimes.activations.iter().map(|(_, row)| row),
                        identity,
                    )?;
                let loan = sources
                    .control
                    .operands
                    .get(loaded.capture)
                    .descriptor_permission
                    .loaded_loan();
                closure.contains(&loan)
                    && !validation::completed_storage::descriptor_sources(
                        sources.program,
                        loaded.sources,
                    )?
                    .is_empty()
            }
            Origin::Invocation { expression, target } => {
                let mut origins = closure.iter().filter(|candidate| {
                    let row = borrow.loans.get(**candidate);
                    row.incoming_reference == identity
                        && row.lineage == BorrowLoanLineage::DirectRoot
                        && row.formation_expression == expression
                        && row.root == facts::PlaceRoot::Expression(expression)
                });
                let Some(origin) = origins.next().filter(|_| origins.next().is_none()) else {
                    return Err("the retained invocation has no unique original loan");
                };
                let mut activations = sources
                    .lifetimes
                    .activations
                    .iter()
                    .filter(|(_, row)| row.loan == *origin);
                let Some((_, activation)) =
                    activations.next().filter(|_| activations.next().is_none())
                else {
                    return Err("the retained invocation has no unique completed activation");
                };
                if !sources
                    .control
                    .operands
                    .is_valid(activation.formation_value)
                {
                    return Err("the retained invocation lost its evaluated result");
                }
                let capture = sources.control.operands.get(activation.formation_value);
                let FlowOperandCompletion::InvocationResult { invocation, .. } = capture.completion
                else {
                    return Err("the retained invocation has no completed result association");
                };
                if !sources
                    .calls
                    .calls
                    .handles()
                    .any(|handle| handle == invocation)
                    || !sources.control.calls.is_valid(invocation)
                {
                    return Err("the retained invocation has no original owning call");
                }
                let call = sources.control.calls.get(invocation);
                capture.state_symbol == state.state_symbol
                    && capture.expression == expression
                    && call.authored_expression == expression
                    && call.target_symbol == target
                    && call.statement_index == capture.statement_index
            }
        };
        if !known {
            return Err("the retained incoming source has no original checked producer");
        }
        checked.push(identity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::HandleSpan;
    use facts::PlaceRoot;
    use symbols::SymbolHandle;

    #[test]
    fn retained_loan_closure_keeps_the_captured_cohort_without_reviving_sibling_holders() {
        let output = SymbolHandle::from_parts(1, 1);
        let input = SymbolHandle::from_parts(2, 1);
        let saved = SymbolHandle::from_parts(3, 1);
        let data = PlaceRoot::Symbol(SymbolHandle::from_parts(4, 1));
        let program = typed_trees::TypedTrees::default();
        let contexts = checked_trees::FlowContextFacts::default();
        let control = checked_trees::FlowControlFacts::default();
        let lifetimes = checked_trees::FlowBorrowLifetimeFacts::default();
        let sources = RetainedLoanSourceScope {
            program: &program,
            contexts: &contexts,
            control: &control,
            lifetimes: &lifetimes,
            calls: super::super::lifetime_contract::SourceCallScope {
                machine: Default::default(),
                state: Default::default(),
                calls: HandleSpan::empty(),
            },
        };
        let mut borrow = BorrowFacts::default();
        let mut state = StateBorrowFact::default();
        let mut append = |borrow: &mut BorrowFacts, owner, statement, lineage, suppliers| {
            let supplying_loans = borrow.loan_suppliers.insert_many(suppliers);
            borrow.loans.append_to_span(
                &mut state.loans,
                BorrowLoanFact {
                    owner_symbol: owner,
                    storage_owner: PlaceRoot::Symbol(owner),
                    root: data,
                    statement_index: statement,
                    lineage,
                    supplying_loans,
                    ..Default::default()
                },
            )
        };
        let original = append(
            &mut borrow,
            output,
            0,
            BorrowLoanLineage::DirectRoot,
            vec![],
        );
        let physical = append(
            &mut borrow,
            output,
            1,
            BorrowLoanLineage::DirectRoot,
            vec![],
        );
        let debt = append(
            &mut borrow,
            output,
            1,
            BorrowLoanLineage::LifetimeDependency,
            vec![original],
        );
        let obsolete_input = append(
            &mut borrow,
            input,
            1,
            BorrowLoanLineage::LifetimeDependency,
            vec![original],
        );
        let child = append(
            &mut borrow,
            saved,
            2,
            BorrowLoanLineage::Reborrow {
                parent_loan: physical,
            },
            vec![physical],
        );
        let held = retained_loan_closure(&borrow, &state, child, &sources).unwrap();
        assert!(held.contains(&physical));
        assert!(held.contains(&debt));
        assert!(held.contains(&original));
        assert!(!held.contains(&obsolete_input));
        assert_eq!(
            retained_loan_closure(&borrow, &state, debt, &sources).unwrap(),
            vec![original, debt],
            "following one debt does not reopen another descriptor's cohort",
        );
        let foreign = StateBorrowFact {
            loans: HandleSpan::empty(),
            ..Default::default()
        };
        assert!(retained_loan_closure(&borrow, &foreign, child, &sources).is_err());
        let cycle = borrow.loan_suppliers.insert_many([child]);
        borrow.loans.get_mut(debt).supplying_loans = cycle;
        assert!(retained_loan_closure(&borrow, &state, child, &sources).is_err());
    }
}

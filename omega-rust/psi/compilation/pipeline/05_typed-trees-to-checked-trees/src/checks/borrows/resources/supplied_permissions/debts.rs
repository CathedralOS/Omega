//! A saved output keeps the helper's original input restrictions. Those
//! restrictions can cover several inputs and earlier outputs; they are not
//! the address of the saved pointer, or a borrow of its former variable.

use arena::Handle;
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, CapturedPlace, CheckFacts,
    CheckedCallBorrowLoanSource, CheckedOutputBorrowInput,
    CheckedOutputLifetimeDependencyResource as Debt,
    CheckedOutputLifetimeDependencySource as Source,
};
use diagnostics::Diagnostic;

pub(in crate::checks::borrows::resources) fn install_transfers(
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let borrow = &facts.borrow;
    let mut pending = borrow
        .states
        .iter()
        .flat_map(|(_, state)| {
            borrow.state_loans(state).filter_map(move |(loan, row)| {
                (row.lineage == BorrowLoanLineage::LifetimeDependency
                    && resource(borrow, loan).is_none())
                .then_some((state.machine_symbol, state.state_symbol, loan))
            })
        })
        .collect::<Vec<_>>();
    loop {
        let before = pending.len();
        let mut remaining = Vec::new();
        for (machine, state, loan) in pending {
            let row = facts.borrow.loans.get(loan);
            let Some(suppliers) = facts.borrow.loan_suppliers.span(row.supplying_loans) else {
                remaining.push((machine, state, loan));
                continue;
            };
            let mut parents = Vec::new();
            let mut inputs: Vec<CheckedOutputBorrowInput> = Vec::new();
            let mut complete = !suppliers.is_empty() && row.source_owner_symbol.is_valid();
            let Some((_, owner)) = facts.borrow.first_state_borrow(machine, state) else {
                return Err(vec![Diagnostic::error(
                    "transported output debt has no owning state",
                )]);
            };
            for source in suppliers {
                if !facts.borrow.state_owns_loan(owner, *source) {
                    complete = false;
                    break;
                }
                let Some(parent) = resource(&facts.borrow, *source) else {
                    complete = false;
                    break;
                };
                let source_row = facts.borrow.loans.get(*source);
                if source_row.owner_symbol != row.source_owner_symbol
                    || source_row.kind.direct_reborrow_effect(&row.kind).is_none()
                {
                    complete = false;
                    break;
                }
                let parent_row = facts.borrow.output_lifetime_dependencies.get(parent);
                let Some(captured) = facts.borrow.supplied_output_inputs.span(parent_row.inputs)
                else {
                    return Err(vec![Diagnostic::error(
                        "transported output debt lost its complete captured inputs",
                    )]);
                };
                for input in captured {
                    crate::shared::collections::push_unique(&mut inputs, *input);
                }
                crate::shared::collections::push_unique(&mut parents, parent);
            }
            if !complete {
                remaining.push((machine, state, loan));
                continue;
            }
            // An actual transfer keeps every exact supplier. Nothing here
            // establishes pointer permission or chooses a borrowing parent.
            let suppliers = facts
                .borrow
                .output_dependency_suppliers
                .insert_many(parents);
            let inputs = facts.borrow.supplied_output_inputs.insert_many(inputs);
            facts.borrow.output_lifetime_dependencies.insert(Debt {
                loan,
                source: Source::Transfer { suppliers },
                inputs,
            });
        }
        if remaining.len() == before {
            break;
        }
        pending = remaining;
    }
    Ok(())
}

fn resource(borrow: &BorrowFacts, loan: Handle<BorrowLoanFact>) -> Option<Handle<Debt>> {
    if !borrow.loans.is_valid(loan)
        || borrow.loans.get(loan).lineage != BorrowLoanLineage::LifetimeDependency
    {
        return None;
    }
    let mut rows = borrow
        .output_lifetime_dependencies
        .iter()
        .filter(|(_, row)| row.loan == loan);
    let (handle, _) = rows.next()?;
    rows.next().is_none().then_some(handle)
}

pub(in crate::checks::borrows) fn has_resource(
    borrow: &BorrowFacts,
    loan: Handle<BorrowLoanFact>,
) -> bool {
    matches!(windows(borrow, loan), Some(Ok(_)))
}

#[derive(Clone)]
pub(in crate::checks::borrows) struct HeldInput {
    pub(in crate::checks::borrows) place: CapturedPlace,
    pub(in crate::checks::borrows) access: checked_trees::BorrowAccessKind,
}

/// All input windows remain separate. In particular, a shared result cannot
/// turn an original whole-input Mutable restriction into Read, and another
/// output's lifetime frontier must be followed without making it a pointee.
pub(in crate::checks::borrows) fn windows(
    borrow: &BorrowFacts,
    loan: Handle<BorrowLoanFact>,
) -> Option<Result<Vec<HeldInput>, &'static str>> {
    let resource = resource(borrow, loan)?;
    Some(resolve_windows(borrow, resource))
}

fn resolve_windows(
    borrow: &BorrowFacts,
    debt_handle: Handle<Debt>,
) -> Result<Vec<HeldInput>, &'static str> {
    let row = borrow.output_lifetime_dependencies.get(debt_handle);
    let mut inputs = borrow
        .supplied_output_inputs
        .span(row.inputs)
        .ok_or("output debt's captured input roster is unavailable")?
        .iter()
        .map(|input| input.resource)
        .collect::<Vec<_>>();
    let mut debt_pending = vec![debt_handle];
    let mut debts = Vec::new();
    while let Some(resource) = debt_pending.pop() {
        if debts.contains(&resource) {
            continue;
        }
        debts.push(resource);
        match borrow.output_lifetime_dependencies.get(resource).source {
            Source::Completion { output } => {
                let output = borrow.supplied_output_resources.get(output);
                if !complete_inputs(borrow, output) {
                    return Err("output debt has an unrepresented input lifetime region");
                }
            }
            Source::Transfer { suppliers } => debt_pending.extend_from_slice(
                borrow
                    .output_dependency_suppliers
                    .span(suppliers)
                    .ok_or("output debt's original generations are unavailable")?,
            ),
        }
    }
    let mut input_seen = Vec::new();
    let mut loans = Vec::new();
    let mut loan_seen = Vec::new();
    let mut result = Vec::new();
    while !inputs.is_empty() || !loans.is_empty() {
        if let Some(input) = inputs.pop() {
            if input_seen.contains(&input) {
                continue;
            }
            input_seen.push(input);
            if !borrow.call_loan_resources.is_valid(input) {
                return Err("output debt names an unavailable input permission");
            }
            let input = borrow.call_loan_resources.get(input);
            result.push(HeldInput {
                place: CapturedPlace {
                    incoming_reference: input.backing_incoming_reference,
                    incoming_owner_path: borrow
                        .call_loan_backing_paths
                        .span(input.backing_incoming_owner_path)
                        .ok_or("output debt lost its captured incoming path")?
                        .to_vec(),
                    root: input.parent_lifetime.root,
                    segments: borrow
                        .call_loan_backing_paths
                        .span(input.backing_path)
                        .ok_or("output debt lost its captured input window")?
                        .to_vec(),
                },
                access: input.access.clone(),
            });
            match input.source {
                CheckedCallBorrowLoanSource::StoredLoan { loan } => loans.push(loan),
                CheckedCallBorrowLoanSource::CallAccess { parents, .. } => loans.extend_from_slice(
                    borrow
                        .call_loan_parents
                        .span(parents)
                        .ok_or("output debt lost its captured input ancestors")?,
                ),
            }
            continue;
        }
        let loan = loans.pop().ok_or("output debt traversal lost a source")?;
        if loan_seen.contains(&loan) {
            continue;
        }
        if !borrow.loans.is_valid(loan) {
            return Err("output debt has an unavailable input ancestor");
        }
        loan_seen.push(loan);
        let row = borrow.loans.get(loan);
        for (_, output) in borrow
            .supplied_output_resources
            .iter()
            .filter(|(_, output)| output.loan == loan)
        {
            if !complete_inputs(borrow, output) {
                return Err("earlier output has an unrepresented input lifetime region");
            }
            inputs.extend(
                borrow
                    .supplied_output_inputs
                    .span(output.inputs)
                    .ok_or("earlier output's captured inputs are unavailable")?
                    .iter()
                    .map(|input| input.resource),
            );
        }
        if let Some(debt) = resource(borrow, loan) {
            inputs.extend(
                borrow
                    .supplied_output_inputs
                    .span(borrow.output_lifetime_dependencies.get(debt).inputs)
                    .ok_or("earlier debt's captured inputs are unavailable")?
                    .iter()
                    .map(|input| input.resource),
            );
        }
        if let BorrowLoanLineage::Reborrow { parent_loan } = row.lineage {
            loans.push(parent_loan);
        }
        loans.extend_from_slice(
            borrow
                .loan_suppliers
                .span(row.supplying_loans)
                .ok_or("output debt's input ancestry is unavailable")?,
        );
    }
    Ok(result)
}

fn complete_inputs(
    borrow: &BorrowFacts,
    output: &checked_trees::CheckedSuppliedOutputBorrowResource,
) -> bool {
    let Some(inputs) = borrow.supplied_output_inputs.span(output.inputs) else {
        return false;
    };
    output
        .lifetime_sources
        .handles()
        .all(|source| inputs.iter().any(|input| input.source == source))
}

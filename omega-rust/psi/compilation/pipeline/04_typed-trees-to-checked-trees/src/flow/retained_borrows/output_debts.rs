//! Releasing an owned output closes original typed input windows, even when
//! the output's current descriptor has no representable pointer association.
//! The invocation-entry loan roster selects generations. Historical captures
//! supply membership only after replay of their original applied declaration.

use super::captured_inputs::RetainedSources;
use crate::places::place_at;
use arena::Handle;
use checked_trees::{
    BorrowCallFact, BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowControlFacts,
    FlowOperandFact,
};
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

pub(in crate::flow) fn sources(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    control: &FlowControlFacts,
    state_symbol: symbols::SymbolHandle,
    call: &BorrowCallFact,
    operand: &FlowOperandFact,
    contract: TypeReferenceHandle,
    stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
) -> Option<RetainedSources> {
    if validation::reference_shell(program, contract).is_some()
        || program.type_multiplicity(contract) == language_semantics::Multiplicity::Unrestricted
    {
        return None;
    }
    let holder = place_at(
        program,
        state_symbol,
        operand.statement_index,
        operand.expression,
    )?;
    if matches!(holder.root, facts::PlaceRoot::Expression(_)) || !stable_name(operand, &holder) {
        return None;
    }
    let roster = borrow.loan_suppliers.span(call.input_reference_loans?)?;
    if roster.iter().any(|loan| !borrow.loans.is_valid(*loan)) {
        return None;
    }
    let paths = validation::borrow_contracts::borrow_carrying_owner_paths(program, contract)?;
    let mut result = RetainedSources::default();
    let mut found = false;
    for path in paths {
        let mut path = crate::borrow::owner_place_segments(&path);
        let declared = validation::project_type_reference_from_segments(program, contract, &path)?;
        if !crate::checks::contracts::exclusive_referent_type(program, declared).is_valid() {
            // This query cannot replace the separate nominal-lifetime carrier
            // or shared-permission paths of captured_sources.
            return None;
        }
        let mut owner = holder.segments.clone();
        owner.append(&mut path);
        let selected = roster
            .iter()
            .copied()
            .filter(|handle| {
                let loan = borrow.loans.get(*handle);
                loan.storage_owner == holder.root
                    && same_path(
                        program,
                        &crate::borrow::owner_place_segments(borrow.loan_owner_path(loan)),
                        &owner,
                    )
            })
            .collect::<Vec<_>>();
        if selected.iter().any(|handle| {
            let loan = borrow.loans.get(*handle);
            !loan.kind.is_exclusive()
                || !matches!(
                    loan.lineage,
                    BorrowLoanLineage::SuppliedOutput | BorrowLoanLineage::LifetimeDependency
                )
        }) {
            return None;
        }
        let outputs = selected
            .iter()
            .copied()
            .filter(|handle| borrow.loans.get(*handle).lineage == BorrowLoanLineage::SuppliedOutput)
            .collect::<Vec<_>>();
        if outputs.is_empty() {
            return None;
        }
        let debts = selected
            .iter()
            .copied()
            .filter(|handle| {
                let loan = borrow.loans.get(*handle);
                loan.lineage == BorrowLoanLineage::LifetimeDependency && loan.kind.is_exclusive()
            })
            .collect::<Vec<_>>();
        if debts.is_empty() {
            return None;
        }
        let mut common: Option<RetainedSources> = None;
        for output in outputs {
            let origin = borrow.supplied_output_call(output)?;
            let output_loan = borrow.loans.get(output);
            let incoming = borrow
                .incoming_references
                .get(output_loan.incoming_reference);
            let checked_trees::IncomingReferenceOrigin::Output { identity } = incoming.origin
            else {
                return None;
            };
            let owner_path = crate::borrow::owner_place_segments(
                borrow.owner_segments.span(incoming.owner_path)?,
            );
            let mut receipts = control
                .output_lifetime_sources
                .iter()
                .filter(|(_, receipt)| {
                    receipt.output == identity
                        && control
                            .operand_retained_paths
                            .span(receipt.owner_path)
                            .is_some_and(|path| same_path(program, path, &owner_path))
                });
            let (_, receipt) = receipts.next()?;
            if receipts.next().is_some() {
                return None;
            }
            let sources = control.operand_retained_suppliers.span(receipt.sources?)?;
            if sources.is_empty() {
                return None;
            }
            let original = borrow.calls.get(origin);
            let original_debts = original
                .completed_reference_loans
                .handles()
                .filter(|handle| {
                    let loan = borrow.loans.get(*handle);
                    loan.lineage == BorrowLoanLineage::LifetimeDependency
                        && loan.storage_owner == output_loan.storage_owner
                        && borrow.loan_owner_path(loan) == borrow.loan_owner_path(output_loan)
                        && loan.kind.is_exclusive()
                })
                .collect::<Vec<_>>();
            // Geometry cannot pair an older same-slot loan with this output.
            // Every currently held debt must retain the same actual supplier
            // lineage as the performed generation's recorded input window.
            for debt in &debts {
                if !window_is_covered(program, borrow, *debt, &original_debts) {
                    return None;
                }
            }
            let mut restored = RetainedSources::default();
            for supplier in sources {
                let bound =
                    super::lifetime_contract::source_type(program, borrow, control, supplier)?;
                let supplied = super::captured_sources(
                    program,
                    borrow,
                    control,
                    control.operands.get(supplier.operand),
                    control.operand_retained_paths.span(supplier.projection)?,
                    bound,
                    stable_name,
                )?;
                restored.append(supplied);
            }
            restored.borrows.retain(|returned| {
                debts.iter().any(|handle| {
                    let debt = borrow.loans.get(*handle);
                    returned.place.root == debt.root
                        && same_path(
                            program,
                            &returned.place.segments,
                            borrow.loan_segments(debt),
                        )
                })
            });
            if debts.iter().any(|handle| {
                let debt = borrow.loans.get(*handle);
                !restored.borrows.iter().any(|returned| {
                    returned.place.root == debt.root
                        && same_path(
                            program,
                            &returned.place.segments,
                            borrow.loan_segments(debt),
                        )
                })
            }) {
                return None;
            }
            match &mut common {
                Some(common) => common.meet(&restored),
                None => common = Some(restored),
            }
        }
        result.append(common?);
        found = true;
    }
    if !found {
        return None;
    }
    if !result.holders.contains(&holder) {
        result.holders.push(holder);
    }
    Some(result)
}

/// The original generation is a complete roster of input alternatives. Every
/// current predecessor path must reach one of those exact original windows or
/// its immediate captured suppliers. Older shared ancestors are not a window
/// identity: two later loans can share an ancestor while owing different bounds.
fn window_is_covered(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    current: Handle<BorrowLoanFact>,
    originals: &[Handle<BorrowLoanFact>],
) -> bool {
    let current_loan = borrow.loans.get(current);
    let mut boundary = Vec::new();
    for original in originals {
        if !borrow.loans.is_valid(*original) {
            return false;
        }
        let loan = borrow.loans.get(*original);
        if loan.root != current_loan.root
            || loan.kind != current_loan.kind
            || !same_path(
                program,
                borrow.loan_segments(loan),
                borrow.loan_segments(current_loan),
            )
        {
            continue;
        }
        // Validate the entire original history for cycles/unknown handles,
        // without flattening those older ancestors into permitted boundaries.
        if !supplier_graph_is_known(borrow, *original) {
            return false;
        }
        boundary.push(*original);
        let Some(sources) = borrow.loan_suppliers.span(loan.supplying_loans) else {
            return false;
        };
        for source in sources {
            if !boundary.contains(source) {
                boundary.push(*source);
            }
        }
    }
    if boundary.is_empty() {
        return false;
    }
    let mut resolved = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(current, false)];
    while let Some((handle, exiting)) = pending.pop() {
        if !borrow.loans.is_valid(handle) {
            return false;
        }
        if boundary.contains(&handle) || resolved.contains(&handle) {
            continue;
        }
        if exiting {
            active.pop();
            resolved.push(handle);
            continue;
        }
        if active.contains(&handle) {
            return false;
        }
        let Some(suppliers) = borrow
            .loan_suppliers
            .span(borrow.loans.get(handle).supplying_loans)
        else {
            return false;
        };
        if suppliers.is_empty() {
            return false;
        }
        active.push(handle);
        pending.push((handle, true));
        pending.extend(suppliers.iter().rev().map(|supplier| (*supplier, false)));
    }
    true
}

fn supplier_graph_is_known(borrow: &BorrowFacts, root: Handle<BorrowLoanFact>) -> bool {
    let mut resolved = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(root, false)];
    while let Some((handle, exiting)) = pending.pop() {
        if !borrow.loans.is_valid(handle) {
            return false;
        }
        if resolved.contains(&handle) {
            continue;
        }
        if exiting {
            active.pop();
            resolved.push(handle);
            continue;
        }
        if active.contains(&handle) {
            return false;
        }
        let Some(suppliers) = borrow
            .loan_suppliers
            .span(borrow.loans.get(handle).supplying_loans)
        else {
            return false;
        };
        active.push(handle);
        pending.push((handle, true));
        pending.extend(suppliers.iter().rev().map(|supplier| (*supplier, false)));
    }
    true
}

fn same_path(
    program: &dyn validation::CheckingInput,
    left: &[PlaceSegment],
    right: &[PlaceSegment],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            validation::storage_path_relations::canonical_place_segments_equal(
                program, *left, *right,
            )
        })
}

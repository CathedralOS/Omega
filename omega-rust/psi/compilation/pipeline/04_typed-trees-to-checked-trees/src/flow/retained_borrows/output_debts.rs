//! Releasing an owned output closes original typed input windows, even when
//! the output's current descriptor has no representable pointer association.
//! The invocation-entry loan roster selects generations. Historical captures
//! supply membership only after replay of their original applied declaration.

use super::captured_inputs::RetainedSources;
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
    let holder = validation::canonical_place_from_expression_in_state(
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
                if !original_debts
                    .iter()
                    .any(|original| same_window(program, borrow, *debt, *original))
                {
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

fn same_window(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    current: Handle<BorrowLoanFact>,
    original: Handle<BorrowLoanFact>,
) -> bool {
    let left = borrow.loans.get(current);
    let right = borrow.loans.get(original);
    if left.root != right.root
        || left.kind != right.kind
        || !same_path(
            program,
            borrow.loan_segments(left),
            borrow.loan_segments(right),
        )
    {
        return false;
    }
    fn ancestors(
        borrow: &BorrowFacts,
        root: Handle<BorrowLoanFact>,
    ) -> Option<Vec<Handle<BorrowLoanFact>>> {
        let mut result = Vec::new();
        let mut pending = vec![root];
        while let Some(handle) = pending.pop() {
            if !borrow.loans.is_valid(handle) {
                return None;
            }
            if result.contains(&handle) {
                continue;
            }
            result.push(handle);
            pending.extend_from_slice(
                borrow
                    .loan_suppliers
                    .span(borrow.loans.get(handle).supplying_loans)?,
            );
        }
        Some(result)
    }
    let (Some(left), Some(right)) = (ancestors(borrow, current), ancestors(borrow, original))
    else {
        return false;
    };
    left.iter().any(|source| right.contains(source))
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

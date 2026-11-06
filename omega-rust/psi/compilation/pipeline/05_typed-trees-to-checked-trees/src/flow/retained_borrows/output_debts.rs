//! Releasing an owned output closes original typed input windows, even when
//! the output's current descriptor has no representable pointer association.
//! The invocation-entry loan roster selects generations. Historical captures
//! supply membership only after replay of their original applied declaration.

use super::captured_inputs::RetainedSources;
use crate::borrow::owner_place_segments;
use crate::published::places::place_at;
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
    scope: crate::borrow::lifetime_contract::SourceCallScope,
    state_symbol: symbols::SymbolHandle,
    call: &BorrowCallFact,
    operand: &FlowOperandFact,
    contract: TypeReferenceHandle,
    stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
) -> Result<RetainedSources, &'static str> {
    if validation::reference_shell(program, contract).is_some()
        || program.type_multiplicity(contract) == language_semantics::Multiplicity::Unrestricted
    {
        return Err("the argument is not an owned affine output carrier");
    }
    let holder = place_at(
        program,
        state_symbol,
        operand.statement_index,
        operand.expression,
    )
    .ok_or("the output argument has no captured holder place")?;
    if matches!(holder.root, facts::PlaceRoot::Expression(_)) || !stable_name(operand, &holder) {
        return Err("the consumed output has no stable named holder");
    }
    let roster = borrow
        .loan_suppliers
        .span(
            call.input_reference_loans
                .ok_or("the consuming call has no complete entry loan roster")?,
        )
        .ok_or("the consuming call entry roster is unavailable")?;
    let machine = program
        .machine_holding_state(state_symbol)
        .ok_or("the output handback has no owning machine")?;
    let (_, state) = borrow
        .first_state_borrow(machine.symbol, state_symbol)
        .ok_or("the output handback has no borrowing state")?;
    if roster.iter().any(|loan| !borrow.loans.is_valid(*loan)) {
        return Err("the call-entry output roster contains an unavailable loan");
    }
    let paths = validation::borrow_contracts::borrow_carrying_owner_paths(program, contract)
        .ok_or("the output contract has no complete reference-leaf roster")?;
    let mut result = RetainedSources::default();
    let mut found = false;
    for path in paths {
        let path = owner_place_segments(&path);
        let declared = validation::project_type_reference_from_segments(program, contract, &path)
            .ok_or("the output contract field has no declared type")?;
        if !crate::checks::contracts::exclusive_referent_type(program, declared).is_valid() {
            // This query cannot replace the separate nominal-lifetime carrier
            // or shared-permission paths of captured_sources.
            return Err("the output field has no exclusive returned duty");
        }
        // The receiving contract and installed holder can use distinct
        // prepared members of one applied record. Match the entire current
        // generation roster in the holder's own typed coordinates.
        let owner = validation::completed_storage::project_captured_storage_place(
            program,
            state_symbol,
            operand.statement_index,
            holder.clone(),
            &path,
        )
        .ok_or("the consumed output field has no applied member correspondence")?;
        let root_type = validation::canonical_place_type_reference(
            program,
            state_symbol,
            operand.statement_index,
            &CanonicalPlace {
                root: holder.root,
                segments: Vec::new(),
            },
        )
        .ok_or("the consumed output holder has no declared type")?;
        let mut selected = Vec::new();
        for handle in roster.iter().copied() {
            let loan = borrow.loans.get(handle);
            if loan.storage_owner != holder.root {
                continue;
            }
            let loan_path = validation::corresponding_loan_owner_path(
                program,
                root_type,
                borrow.loan_owner_path(loan),
            )
            .ok_or("a current output loan has no applied owner correspondence")?;
            if same_path(program, &owner_place_segments(&loan_path), &owner.segments) {
                selected.push(handle);
            }
        }
        if selected.iter().any(|handle| {
            let loan = borrow.loans.get(*handle);
            !loan.kind.is_exclusive()
                || !matches!(
                    loan.lineage,
                    BorrowLoanLineage::SuppliedOutput
                        | BorrowLoanLineage::UnretainedDerived
                        | BorrowLoanLineage::LifetimeDependency
                )
        }) {
            return Err("the current output field has a different permission role");
        }
        let mut outputs = Vec::new();
        for current in selected.iter().copied().filter(|handle| {
            borrow.loans.get(*handle).lineage != BorrowLoanLineage::LifetimeDependency
        }) {
            for original in
                crate::borrow::supplied_outputs::completed_output_origins(borrow, state, current)?
            {
                if !outputs.contains(&original) {
                    outputs.push(original);
                }
            }
        }
        if outputs.is_empty() {
            return Err("the current field has no completed output generation");
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
            return Err("the current output has no exclusive input windows");
        }
        let mut common: Option<RetainedSources> = None;
        for output in outputs {
            let origin = borrow
                .supplied_output_call(output)
                .ok_or("the original output has no exact normal completion")?;
            let output_loan = borrow.loans.get(output);
            let incoming = borrow
                .incoming_references
                .get(output_loan.incoming_reference);
            let checked_trees::IncomingReferenceOrigin::Output { identity } = incoming.origin
            else {
                return Err("the completed output has no output namespace");
            };
            let owner_path = owner_place_segments(
                borrow
                    .owner_segments
                    .span(incoming.owner_path)
                    .ok_or("the original output lost its descriptor path")?,
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
            let (_, receipt) = receipts
                .next()
                .ok_or("the original output has no captured input receipt")?;
            if receipts.next().is_some() {
                return Err("the output has ambiguous captured input receipts");
            }
            let sources = control
                .operand_retained_suppliers
                .span(
                    receipt
                        .sources
                        .ok_or("the output input receipt is incomplete")?,
                )
                .ok_or("the output input receipt roster is unavailable")?;
            if sources.is_empty() {
                return Err("the output has an empty captured input roster");
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
                    return Err(
                        "a current debt does not retain the original completed input window",
                    );
                }
            }
            let mut restored = RetainedSources::default();
            for supplier in sources {
                for alternatives in crate::borrow::lifetime_contract::retained_inputs(
                    program, borrow, control, Some(scope), supplier,
                )
                .inspect_err(|reason| {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!("CHECKSITE original-output-input-unavailable output={output:?} supplier={supplier:?} reason={reason}");
                    }
                })?
                {
                    let mut common: Option<RetainedSources> = None;
                    for input in alternatives {
                        let supplied = match input {
                            crate::borrow::lifetime_contract::RetainedLifetimeInput::Value {
                                operand,
                                path,
                                bound,
                            } => {
                                if !control.operands.is_valid(operand) {
                                    return Err("an original input has no completed capture");
                                }
                                super::captured_sources(
                                    program,
                                    borrow,
                                    control,
                                    scope,
                                    control.operands.get(operand),
                                    path,
                                    bound,
                                    stable_name,
                                )
                                .ok_or("the captured input has no complete typed duty inventory")?
                            }
                            crate::borrow::lifetime_contract::RetainedLifetimeInput::Incoming {
                                bound,
                            } => {
                                if crate::checks::contracts::exclusive_referent_type(program, bound)
                                    .is_valid()
                                {
                                    return Err(
                                        "an incoming exclusive duty has no completed owned handback",
                                    );
                                }
                                RetainedSources::default()
                            }
                        };
                        match &mut common {
                            Some(common) => common.meet(&supplied),
                            None => common = Some(supplied),
                        }
                    }
                    restored.append(common.ok_or("the stored input has no source alternatives")?);
                }
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
                !restored.duty_places.iter().any(|place| {
                    place.root == debt.root
                        && same_path(program, &place.segments, borrow.loan_segments(debt))
                })
            }) {
                if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    let describe = |handle: &Handle<BorrowLoanFact>| {
                        let loan = borrow.loans.get(*handle);
                        (
                            *handle,
                            loan.root,
                            borrow.loan_segments(loan),
                            &loan.kind,
                            &loan.lineage,
                            borrow.loan_suppliers.span(loan.supplying_loans),
                        )
                    };
                    eprintln!(
                        "CHECKSITE output-duty-coverage-unavailable state={state_symbol:?} statement={} output={output:?} current_debts={:?} original_debts={:?} restored={restored:?}",
                        operand.statement_index,
                        debts.iter().map(describe).collect::<Vec<_>>(),
                        original_debts.iter().map(describe).collect::<Vec<_>>()
                    );
                }
                return Err("a current debt has no complete captured duty coverage");
            }
            match &mut common {
                Some(common) => common.meet(&restored),
                None => common = Some(restored),
            }
        }
        result.append(common.ok_or("the output has no completed alternatives")?);
        found = true;
    }
    if !found {
        return Err("the owned argument has no output field");
    }
    if !result.holders.contains(&holder) {
        result.holders.push(holder);
    }
    Ok(result)
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

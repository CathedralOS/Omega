//! A call whose value lands in a structural result binding. Shared operand
//! admission owns current storage, type and access; this operation owns the
//! returned contents and custody, not another whitelist of operand producers.
//! Returned references retain exact captured loans, and claim-bearing results
//! retain their checked frontier. Both use one ordinary call whose custody is
//! decided here and recorded in the plan (`with_structural_custody`).

use checked_trees::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};

use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::machine_binders;
use crate::execution::terminal_unit::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralResultBindingPlan,
    Multiplicity, SymbolHandle,
};

pub(super) fn build(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    trace: &LocalConstructionTrace,
    planned: super::call_operations::PlannedCall<'_>,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    let super::call_operations::PlannedCall {
        coordinate,
        source_site,
        call,
        target_machine,
        target_state,
        target_contract,
        scalar_arguments,
        erased_scalar_arguments,
        erased_proof_arguments,
        structural_arguments,
        transfers,
    } = planned;
    let call_target = CheckedUnitCallTarget {
        machine: target_machine.symbol,
        state: target_state.symbol,
        contract_report_fingerprint: target_contract.report_fingerprint,
        service_reach: call.service_reach,
    };
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
    phase("call operation: structural result loan");
    let forwarded_reference_sources =
        validation::reference_result_custody::terminal_call_reference_sources(
            program, facts, state, call, result,
        )
        .unwrap_or_default();
    let reference_result = crate::execution::terminal_unit::reference_results::has_borrowed_parts(
        program,
        target_state.return_type,
    );
    let reference_loans = if !forwarded_reference_sources.is_empty() {
        arena::HandleSpan::empty()
    } else if reference_result {
        // Every access retains the exact call's loan, activation and release.
        // Origin reconstruction belongs to the shared reference-result owner;
        // a private view walker must not miss ordinary forwarded completions.
        crate::execution::terminal_unit::reference_results::result_loans(
            program,
            facts,
            machine.symbol,
            state,
            call,
            result,
        )?
    } else {
        arena::HandleSpan::empty()
    };
    // A result whose contents hold no reference or slice cannot retain a loan
    // of its operands: whatever the callee read or wrote through a borrowed
    // argument stays in the lender's storage. A nominal cleanup obligation
    // travels with the owned result without retaining an operand loan.
    let represented_copy_result = result.multiplicity == Multiplicity::Unrestricted
        && validation::has_contents(
            program,
            target_state.return_type,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        );
    // Operand preparation already admitted source identity, declared types,
    // projections and exact access. Do not apply a second source-family
    // whitelist here: a named home's current value is not its initializer.
    // The result judgment below owns returned contents and loans; the shared
    // custody judgment joins this occurrence's exact claim and loan roster.
    // An ordinary call keeps one route and result binding whatever claims it
    // consumes. Consumed linear claim-carrying inputs become the custody's
    // input transfer roster, and the custody judgment fences that result side
    // itself; a call that consumes none must also carry admitted result
    // contents.
    if (!transfers.is_empty()
            || represented_copy_result
            || reference_result
            || !forwarded_reference_sources.is_empty()
            || crate::execution::terminal_unit::reference_results::is_reference_record(
                program,
                target_state.return_type,
            )
            // Closed contents and the retained body determine the carrier,
            // including lifetime-only applications. Named syntax is not an
            // extra ownership proof; the classifier still rejects open types,
            // carried loans and unknown qualifications. Nominal cleanup remains
            // attached to the transported owner and is checked at its death.
            //
            // The judgment is the one the result binding answered to: the
            // contents a structural literal establishes. It carries the
            // scalar members whose declared plain domain is represented as
            // well as the nominal-cleanup owners — the callee's completion
            // proved each member's obligation and this route only transports
            // the storage, so a transported record is admitted exactly where
            // a constructed one is. A top-level qualification on the result
            // still refuses: the binding classifier rejects one before this
            // route can drop it.
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_contents(
        program,
        target_state.return_type,
        &[],
        validation::ContentsRequirement::StructuralConstruction
    ))
            || (result.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(
                    program,
                    target_state.return_type,
                ))
            // A sum or record carrying shared-view members — the same
            // descriptor loans the bare-view arm below names, nested inside
            // owned contents. The classifier admits no loan that mutates, no
            // erased or generic member, and no cleanup owner, so the result
            // keeps the same claim-free custody the plain-contents arm gives.
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_contents(
        program,
        target_state.return_type,
        &[],
        validation::ContentsRequirement::OwnedOrSharedView
    ))
            // Shared descriptors and materialized reference carriers borrow
            // storage but retain their distinct result-custody obligations.
            || (result.multiplicity
                == validation::reference_result_custody::result_multiplicity(
                    program,
                    target_state.return_type,
                )
                && (crate::execution::terminal_unit::types::borrowed_slice_result(
                    program,
                    target_state.return_type,
                ) || crate::execution::terminal_unit::types::borrowed_named_view(
                    program,
                    target_state.return_type,
                ))))
        && program
            .machine_states(target_machine)
            .first()
            .is_some_and(|entry| entry.symbol == target_state.symbol)
        && machine_binders(program, target_machine).is_empty()
    {
        // An owned affine result without retained loans owes an independent
        // return-discard. A borrowed-parts result is a carrier descriptor
        // whose retirement the loan schedule owns, so an empty span here —
        // including the anonymous operand case `result_loans` reports as an
        // empty roster — marks scheduled custody, not owned storage.
        let discard_mint = result.multiplicity == Multiplicity::Affine
            && !reference_result
            && reference_loans.is_empty();
        return with_structural_custody(
            program,
            facts,
            machine.symbol,
            state.symbol,
            trace,
            structural_custody_parts(
                coordinate,
                source_site,
                call_target,
                target_contract.commitment,
                erased_scalar_arguments,
                erased_proof_arguments,
                scalar_arguments,
                structural_arguments,
                result.clone(),
                discard_mint,
            ),
        );
    }
    None
}

/// The Module-route structural-custody call parts both admissible sites build.
pub(super) fn structural_custody_parts(
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    source_site: Option<checked_trees::NominalMachineUseSite>,
    target: checked_trees::CheckedUnitCallTarget,
    commitment: checked_trees::MachineContractCommitment,
    erased_scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    erased_proof_arguments: Vec<checked_trees::CheckedProofTerm>,
    scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    structural_arguments: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
    binding: checked_trees::CheckedUnitStructuralResultBindingPlan,
    discard_result_on_return: bool,
) -> checked_trees::CheckedUnitCallParts {
    checked_trees::CheckedUnitCallParts {
        coordinate,
        completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
        source_site,
        target,
        route: CheckedUnitCallRoute::Module {
            contract_commitment: Some(commitment),
            erased_scalar_arguments,
            erased_proof_arguments,
        },
        scalar_arguments,
        structural_arguments,
        result: CheckedUnitCallResult::Structural {
            binding,
            discard_result_on_return,
        },
        transfers: CheckedUnitCallTransfers::StructuralCustody(Default::default()),
    }
}

/// Admit one ordinary structural call and record its custody: the checker's
/// single judgment over the exact call and ownership facts. Lowering reads
/// the recorded custody; a call the judgment refuses is not planned.
pub(super) fn with_structural_custody(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    trace: &LocalConstructionTrace,
    parts: CheckedUnitCallParts,
) -> Option<CheckedUnitEffectOperationPlan> {
    let mut operation = CheckedUnitEffectOperationPlan::Call(CheckedUnitCallPlan::new(parts)?);
    let custody = match validation::structural_call_custody(
        program,
        facts,
        caller_machine,
        caller_state,
        &operation,
    ) {
        Ok(custody) => custody,
        Err(reason) => {
            // Keep the owning judgment's precise refusal on the demanded
            // body's gap instead of losing it behind the call family label.
            trace.phase(reason);
            return None;
        }
    };
    *operation.call_mut()?.structural_custody_mut()? = custody;
    Some(operation)
}

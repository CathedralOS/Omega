//! Settle declaration-body calls against this invocation's completed value-call
//! admission and public contract plans. The application retains its own input
//! formation; this join neither creates a statement nor replays its proof.

pub(super) fn bind(
    applications: &mut checked_trees::CheckedRankArrivalPlan,
    admitted: &[validation::SpecificationCallRequest],
    contracts: &checked_trees::MachineContractPlans,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let calls = applications
        .application_calls
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in calls {
        let call = applications.application_calls.get_mut(handle);
        if !admitted.iter().any(|request| {
            request.call_expression == call.expression
                && request.target_machine == call.target_machine
                && request.target_state == call.target_state
        }) {
            return Err(vec![diagnostics::Diagnostic::error(
                "declaration call lost its admitted denotational invocation",
            )]);
        }
        let Some(contract) = contracts.for_machine(call.target_machine) else {
            return Err(vec![diagnostics::Diagnostic::error(
                "declaration call target has no completed public contract plan",
            )]);
        };
        call.target_contract = Some(checked_trees::MachineContractIdentity {
            report_fingerprint: contract.report_fingerprint,
            commitment: contract.commitment,
        });
    }
    Ok(())
}

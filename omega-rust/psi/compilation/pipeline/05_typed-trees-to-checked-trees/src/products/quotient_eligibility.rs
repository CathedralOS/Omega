//! Finish quotient eligibility from the same checking invocation's crash facts.
//! Schema and termination admission precede body checking; no new effect walk
//! or crash inference runs here. Missing invocation evidence is not purity.
use crate::flow::expressions;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

pub(crate) fn check(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for operation in &facts.quotient_operations {
        if let Err(reason) = check_operation(program, facts, operation) {
            diagnostics.push(Diagnostic::error(format!(
                "quotient operation `{}` lacks checked normal-result eligibility: {reason}",
                program.symbols.display_path(operation.public_machine, "::"),
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_operation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operation: &checked_trees::CheckedQuotientOperation,
) -> Result<(), &'static str> {
    let state = facts
        .flow
        .control
        .state_by_symbol(operation.public_state)
        .ok_or("public flow state is absent")?;
    let mut calls = crate::flow::call_flows(&facts.flow.control, state.calls)
        .iter()
        .filter(|call| call.authored_expression == operation.request);
    let call = calls.next().ok_or("selected invocation is absent")?;
    if calls.next().is_some() {
        return Err("selected invocation is ambiguous");
    }
    let (representative, _) =
        validation::find_machine_by_entry_state(program, operation.representative_state)
            .ok_or("selected representative is absent")?;
    let plan = facts
        .contract_plans
        .for_machine(operation.public_machine)
        .ok_or("public checked contract is absent")?;
    let checked = plan
        .crash
        .checked_call_at(
            operation.public_state,
            u32::try_from(call.statement_index).map_err(|_| "statement coordinate overflow")?,
            u32::try_from(call.call_ordinal).map_err(|_| "call coordinate overflow")?,
        )
        .ok_or("selected representative crash evidence is unresolved")?;
    if checked.target_machine() != representative.symbol
        || checked.target_state() != operation.representative_state
    {
        checked.executable_contract_identity(
            &facts.contract_plans,
            &facts.proof,
            operation.public_machine,
            representative.symbol,
            operation.representative_state,
        )?;
    }
    if !checked.surviving_buckets().is_empty() {
        return Err("selected representative retains a possible crash in the public invocation");
    }
    let request = expressions(program)
        .call_node(operation.request)
        .and_then(|call| call.quotient_operation.as_ref())
        .ok_or("selected request is absent")?;
    for theorem in &request.theorem_evidence {
        let (machine, _) =
            validation::find_machine_by_entry_state(program, theorem.application.symbol)
                .ok_or("selected theorem body is absent")?;
        let plan = facts
            .contract_plans
            .for_machine(machine.symbol)
            .ok_or("selected theorem checked contract is absent")?;
        if !plan.crash.checked_sites().is_empty()
            || !plan.crash.trapping_sites().is_empty()
            || plan
                .crash
                .checked_calls()
                .iter()
                .any(|call| !call.surviving_buckets().is_empty())
            || plan
                .crash
                .checked_operators()
                .iter()
                .any(|operator| !operator.surviving().is_empty())
        {
            return Err("selected theorem retains checked crash behavior");
        }
        for (_, state) in facts
            .flow
            .control
            .machine_states(machine.symbol)
            .into_iter()
            .map(|state| ((), state))
        {
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let site = validation::find_call_site(
                    program,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                .ok_or("selected theorem call site is unresolved")?;
                if matches!(site, validation::CallSite::TransitionNamed { .. }) {
                    continue;
                }
                if let Some((target_machine, target_state)) =
                    crate::published::facts::selected_invocation_target(
                        program,
                        site,
                        call.target_symbol,
                    )
                {
                    let checked = plan
                        .crash
                        .checked_call_at(
                            state.state_symbol,
                            u32::try_from(call.statement_index)
                                .map_err(|_| "theorem statement coordinate overflow")?,
                            u32::try_from(call.call_ordinal)
                                .map_err(|_| "theorem call coordinate overflow")?,
                        )
                        .ok_or("selected theorem invocation crash evidence is unresolved")?;
                    if checked.target_machine() != target_machine
                        || checked.target_state() != target_state
                    {
                        checked.executable_contract_identity(
                            &facts.contract_plans,
                            &facts.proof,
                            plan.machine,
                            target_machine,
                            target_state,
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

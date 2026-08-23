use crate::BackendReportInput;
use omega_control_flow::ProofObligationOwner;
use psi_symbols::SymbolHandle;

pub(super) fn write_checked_semantics_section(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Checked Semantics\n");
    output.push_str(&format!(
        "proof obligations: {}\n",
        backend_plan
            .control_flow
            .semantics
            .facts
            .proof_obligations
            .len()
    ));
    if backend_plan
        .control_flow
        .semantics
        .facts
        .proof_obligations
        .is_empty()
    {
        output.push_str("none\n");
    } else {
        for (_, obligation) in backend_plan
            .control_flow
            .semantics
            .facts
            .proof_obligations
            .iter()
        {
            output.push_str(&format!(
                "- {:?}: {}\n",
                obligation.kind,
                proof_obligation_owner_display_name(backend_plan, &obligation.owner)
            ));
        }
    }

    output.push_str(&format!(
        "invariants: {}\n",
        backend_plan.control_flow.semantics.facts.invariants.len()
    ));
    if backend_plan
        .control_flow
        .semantics
        .facts
        .invariants
        .is_empty()
    {
        output.push_str("none\n");
    } else {
        for (_, invariant) in backend_plan.control_flow.semantics.facts.invariants.iter() {
            output.push_str(&format!(
                "- `{}` constraints {}\n",
                invariant.name, invariant.constraint_count
            ));
        }
    }
    output.push('\n');
}

fn proof_obligation_owner_display_name(
    backend_plan: &BackendReportInput<'_>,
    owner: &ProofObligationOwner,
) -> String {
    match owner {
        ProofObligationOwner::Unknown => "unknown".to_owned(),
        ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            format!("machine `{machine_name}` state `{state_name}`")
        }
        ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
            ..
        } => {
            let machine_name = proof_machine_name(backend_plan, *machine_symbol);
            let data_name = backend_plan
                .control_flow
                .machine_owned_data_by_symbol(*machine_symbol, *data_symbol)
                .map(|data| data.name.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned());
            format!("machine `{machine_name}` owned data `{data_name}`")
        }
        ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            let parameter_name = proof_state_parameter_name(
                backend_plan,
                *machine_symbol,
                *state_symbol,
                *parameter_symbol,
            );
            format!("machine `{machine_name}` state `{state_name}` parameter `{parameter_name}`")
        }
        ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            format!("machine `{machine_name}` state `{state_name}` return")
        }
        ProofObligationOwner::CallParameter {
            machine_symbol,
            state_symbol,
            target_symbol,
            parameter_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            let target_name = proof_state_name(backend_plan, *machine_symbol, *target_symbol);
            let parameter_name = proof_state_parameter_name(
                backend_plan,
                *machine_symbol,
                *target_symbol,
                *parameter_symbol,
            );
            format!(
                "machine `{machine_name}` state `{state_name}` call `{target_name}` parameter `{parameter_name}`"
            )
        }
        ProofObligationOwner::TransitionParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            let parameter_name = proof_state_parameter_name(
                backend_plan,
                *machine_symbol,
                *state_symbol,
                *parameter_symbol,
            );
            format!(
                "machine `{machine_name}` state `{state_name}` transition parameter `{parameter_name}`"
            )
        }
    }
}

fn proof_machine_name(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: SymbolHandle,
) -> String {
    backend_plan
        .control_flow
        .machine_by_symbol(machine_symbol)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn proof_state_names(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> (String, String) {
    backend_plan
        .control_flow
        .state_key_by_symbols(machine_symbol, state_symbol)
        .and_then(|key| backend_plan.control_flow.state_names_by_key(key))
        .map(|(machine, state)| (machine.to_string(), state.to_string()))
        .unwrap_or_else(|| ("<unknown>".to_owned(), "<unknown>".to_owned()))
}

fn proof_state_name(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> String {
    proof_state_names(backend_plan, machine_symbol, state_symbol).1
}

fn proof_state_parameter_name(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
) -> String {
    backend_plan
        .control_flow
        .state_key_by_symbols(machine_symbol, state_symbol)
        .and_then(|key| backend_plan.control_flow.state_by_key(key))
        .and_then(|state| {
            backend_plan
                .control_flow
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == parameter_symbol)
        })
        .map(|parameter| parameter.name.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

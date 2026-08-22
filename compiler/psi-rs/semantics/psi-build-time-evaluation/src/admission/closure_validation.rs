use psi_language_semantics::{MachineSupplyMode, TerminationGuarantee};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;

use super::BuildTimeCallEdge;

pub(super) fn checked_closure_violation(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: &Machine,
) -> Option<String> {
    let mut completed = Vec::new();
    let mut active = Vec::new();
    let mut path = Vec::new();
    machine_termination_violation(
        call_edges,
        program,
        root.symbol,
        &mut completed,
        &mut active,
        &mut path,
    )
}

fn machine_termination_violation(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    completed: &mut Vec<SymbolHandle>,
    active: &mut Vec<SymbolHandle>,
    path: &mut Vec<String>,
) -> Option<String> {
    if completed.contains(&machine_symbol) {
        return None;
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    path.push(machine.name.as_str().to_owned());

    if let Some(violation) = machine_precondition_violation(program, machine, path) {
        path.pop();
        return Some(violation);
    }
    if let Some(violation) = machine_linear_carrier_violation(program, machine, path) {
        path.pop();
        return Some(violation);
    }

    if active.contains(&machine_symbol) {
        let violation = format!(
            "recursive machine-call cycle has no ordinary termination proof along `{}`",
            path.join(" -> ")
        );
        path.pop();
        return Some(violation);
    }
    active.push(machine_symbol);

    let locally_terminates = machine.supply_mode == MachineSupplyMode::CheckedBody
        && matches!(
            psi_typed_trees_to_checked_trees::infer_machine_termination_summary(
                program,
                machine_symbol
            ),
            Some(TerminationGuarantee::Terminates { .. })
        );
    if !locally_terminates {
        let violation = format!(
            "machine call path `{}` has no ordinary checked `Terminates` guarantee",
            path.join(" -> ")
        );
        active.retain(|active_symbol| *active_symbol != machine_symbol);
        path.pop();
        return Some(violation);
    }

    for call in call_edges
        .iter()
        .filter(|call| call.source_machine_symbol == machine_symbol)
    {
        let target_machine_symbol = if call.target_machine_symbol.is_valid() {
            Some(call.target_machine_symbol)
        } else if call.target_state_symbol.is_valid()
            && program.symbols.get(call.target_state_symbol).kind == SymbolKind::Machine
        {
            // Unmeasured terminal recursion deliberately remains a
            // machine-symbol call until validation can diagnose the
            // missing measure. Semantic evaluation runs earlier,
            // so its admission closure must retain that edge too.
            Some(call.target_state_symbol)
        } else {
            None
        };
        if let Some(target_machine_symbol) = target_machine_symbol {
            if let Some(violation) = machine_termination_violation(
                call_edges,
                program,
                target_machine_symbol,
                completed,
                active,
                path,
            ) {
                active.retain(|active_symbol| *active_symbol != machine_symbol);
                path.pop();
                return Some(violation);
            }
        } else if let Some(violation) =
            callable_contract_violation(program, call.target_state_symbol, path)
        {
            active.retain(|active_symbol| *active_symbol != machine_symbol);
            path.pop();
            return Some(violation);
        }
    }

    active.retain(|active_symbol| *active_symbol != machine_symbol);
    completed.push(machine_symbol);
    path.pop();
    None
}

fn machine_precondition_violation(
    program: &TypedTrees,
    machine: &Machine,
    path: &[String],
) -> Option<String> {
    if has_authored_requires(program.machine_contracts(machine)) {
        return Some(format!(
            "machine `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
            machine.name,
            path.join(" -> ")
        ));
    }
    program.machine_states(machine).iter().find_map(|state| {
        has_authored_requires(program.state_contracts(state)).then(|| {
            format!(
                "state `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
                state.name,
                path.join(" -> ")
            )
        })
    })
}

fn has_authored_requires(contracts: &[psi_typed_trees::signature::SignatureContract]) -> bool {
    contracts.iter().any(|contract| {
        contract.kind == psi_typed_trees::signature::SignatureContractKind::Requires
    })
}

fn machine_linear_carrier_violation(
    program: &TypedTrees,
    machine: &Machine,
    path: &[String],
) -> Option<String> {
    let describe = |context: &str, type_reference| {
        (program.type_multiplicity(type_reference)
            == psi_language_semantics::Multiplicity::Linear)
            .then(|| {
                format!(
                    "{context} has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                    program.display_type_reference(type_reference),
                    path.join(" -> ")
                )
            })
    };

    if let Some(attached_data) = machine.attached_data.as_ref()
        && program.data_definitions().iter().any(|definition| {
            definition.name.as_str() == attached_data.as_str()
                && definition.properties.multiplicity
                    == psi_language_semantics::Multiplicity::Linear
        })
    {
        return Some(format!(
            "machine instance `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
            machine.name,
            attached_data,
            path.join(" -> ")
        ));
    }

    for owned in program.machine_owned_data(machine) {
        if let Some(violation) = describe(
            &format!("machine-owned value `{}`", owned.name),
            owned.type_reference,
        ) {
            return Some(violation);
        }
    }

    for state in program.machine_states(machine) {
        for parameter in program.state_parameters(state) {
            if parameter.is_self {
                continue;
            }
            if let Some(violation) = describe(
                &format!("state `{}` parameter `{}`", state.name, parameter.name),
                parameter.type_reference,
            ) {
                return Some(violation);
            }
        }
        if let Some(violation) =
            describe(&format!("state `{}` result", state.name), state.return_type)
        {
            return Some(violation);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            let psi_typed_trees::statement::StatementNode::LocalData(local) = statement else {
                continue;
            };
            if let Some(violation) = describe(
                &format!("state `{}` local `{}`", state.name, local.name),
                local.type_reference,
            ) {
                return Some(violation);
            }
        }
    }

    None
}

fn callable_contract_violation(
    program: &TypedTrees,
    symbol: SymbolHandle,
    path: &[String],
) -> Option<String> {
    if !symbol.is_valid()
        || matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::BuiltinFunction | SymbolKind::Operator
        )
    {
        return None;
    }

    let signature = program
        .machine_parameter_signature(symbol)
        .map(|(_, signature)| signature)
        .or_else(|| {
            program.traits().iter().find_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == symbol)
            })
        });
    let signature = signature?;

    if has_authored_requires(program.state_signature_contracts(signature)) {
        return Some(format!(
            "callable contract `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
            signature.name,
            path.join(" -> ")
        ));
    }

    for parameter in program.state_signature_parameters(signature) {
        if parameter.is_self {
            continue;
        }
        if program.type_multiplicity(parameter.type_reference)
            == psi_language_semantics::Multiplicity::Linear
        {
            return Some(format!(
                "callable contract `{}` parameter `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                signature.name,
                parameter.name,
                program.display_type_reference(parameter.type_reference),
                path.join(" -> ")
            ));
        }
    }
    if program.type_multiplicity(signature.return_type)
        == psi_language_semantics::Multiplicity::Linear
    {
        return Some(format!(
            "callable contract `{}` result has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
            signature.name,
            program.display_type_reference(signature.return_type),
            path.join(" -> ")
        ));
    }

    if signature.terminates_guarantee {
        return None;
    }

    let name = signature.name.as_str();
    Some(format!(
        "callable contract `{name}` reached from `{}` publishes no `Terminates` guarantee",
        path.join(" -> ")
    ))
}

//! Source-independent validation of retained closed reach substitutions.

use terminal_psi::{ClosedReachParameter, TerminalModule};

/// Check retained substitution data at external ingress. The original checked
/// dependency is producer-trusted; its closed union, bounds and executable
/// occurrence joins are independently reconstructible without source trees.
pub fn validate_closed_reach_applications(module: &TerminalModule) -> Result<(), &'static str> {
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{ClosedReachArgument, Operation, TerminalIdentityIndex};

    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    if machines.len() != module.machines.len() {
        return Err("closed reach machine identities are ambiguous");
    }
    let services = module
        .services
        .iter()
        .map(|service| (service.id, service))
        .collect::<BTreeMap<_, _>>();
    if services.len() != module.services.len() {
        return Err("closed reach service identities are ambiguous");
    }
    let identity = |coordinate: TerminalIdentityIndex| {
        module
            .identities
            .text(coordinate)
            .filter(|text| !text.is_empty())
            .ok_or("closed reach has an invalid or empty identity")
    };
    let row = |row: &[semantic_vocabulary::ServiceId]| -> Result<(), &'static str> {
        if row.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("closed reach service row is not strictly ordered");
        }
        for service in row {
            let declaration = services
                .get(service)
                .ok_or("closed reach has an unknown service")?;
            if declaration
                .parents
                .iter()
                .any(|parent| row.binary_search(parent).is_err())
            {
                return Err("closed reach service row lost its parent closure");
            }
        }
        Ok(())
    };
    for machine in &module.machines {
        let operations = machine
            .operations()
            .map(|operation| (operation.id, operation))
            .collect::<BTreeMap<_, _>>();
        if operations.len() != machine.operations().count() {
            return Err("closed reach owner operation identities are ambiguous");
        }
        let Some(application) = &machine.closed_reach_application else {
            if operations
                .values()
                .any(|operation| operation.static_reach_binding.is_some())
            {
                return Err("closed reach operation marker has no retained application");
            }
            continue;
        };
        identity(application.template_identity)?;
        if application.template_commitment == [0; 32]
            || application.specialization_commitment == [0; 32]
            || machine
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.specialization.as_ref())
                .map(|specialization| specialization.commitment)
                != Some(application.specialization_commitment)
        {
            return Err("closed reach application differs from its checked declaration commitment");
        }
        row(&application.fixed)?;
        row(&machine.published_service_ceiling)?;
        for parameter in &application.telescope {
            match parameter {
                ClosedReachParameter::Type { argument }
                | ClosedReachParameter::Const { argument }
                | ClosedReachParameter::Proposition { argument }
                    if argument.is_empty() =>
                {
                    return Err("closed reach telescope has an empty argument");
                }
                ClosedReachParameter::Machine(binding) => {
                    identity(binding.selected_identity)?;
                    if binding.selected_contract_commitment == [0; 32]
                        || binding
                            .nominal_requirement
                            .as_ref()
                            .is_some_and(String::is_empty)
                    {
                        return Err("closed reach machine binder lost its public contract");
                    }
                    row(&binding.upper_bound)?;
                    row(&binding.selected_reach)?;
                    if binding
                        .selected_reach
                        .iter()
                        .any(|service| binding.upper_bound.binary_search(service).is_err())
                    {
                        return Err("closed reach selected contract exceeds its requirement bound");
                    }
                    if let Some(callee) = binding.callee {
                        let callee = machines
                            .get(&callee)
                            .ok_or("closed reach selected callee is absent")?;
                        if callee
                            .declaration
                            .as_ref()
                            .map(|declaration| declaration.contract)
                            != Some(binding.selected_contract_commitment)
                            || callee.published_service_ceiling != binding.selected_reach
                        {
                            return Err(
                                "closed reach selection differs from its retained callee contract",
                            );
                        }
                    }
                    if let Some(schema) = &binding.schema {
                        identity(schema.template_identity)?;
                        if schema.template_commitment == [0; 32] {
                            return Err("closed reach schema has an empty template commitment");
                        }
                    }
                }
                _ => {}
            }
        }
        if application
            .dependencies
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err("closed reach dependency binders are not strictly ordered");
        }
        let mut union = application.fixed.iter().copied().collect::<BTreeSet<_>>();
        for dependency in &application.dependencies {
            let Some(ClosedReachParameter::Machine(binding)) =
                application.telescope.get(*dependency as usize)
            else {
                return Err("closed reach dependency is outside its machine telescope");
            };
            if binding.nominal_requirement.is_none() {
                return Err("closed reach dependency selects a structural fixed-row binder");
            }
            union.extend(binding.selected_reach.iter().copied());
        }
        if union.into_iter().collect::<Vec<_>>() != machine.published_service_ceiling {
            return Err("closed reach finite union differs from its published service row");
        }
        if application
            .calls
            .windows(2)
            .any(|pair| pair[0].operation >= pair[1].operation)
        {
            return Err("closed reach call occurrences are not strictly ordered");
        }
        for call in &application.calls {
            let operation: &&Operation = operations
                .get(&call.operation)
                .ok_or("closed reach call has no exact owner operation")?;
            if operation.static_reach_binding != Some(call.binder) {
                return Err("closed reach call differs from its operation binder marker");
            }
            let Some(ClosedReachParameter::Machine(binding)) =
                application.telescope.get(call.binder as usize)
            else {
                return Err("closed reach call is outside its machine telescope");
            };
            let callee = operation
                .kind
                .direct_callee()
                .ok_or("closed reach marked operation is not a direct call")?;
            match (&binding.schema, &call.application) {
                (None, None) if binding.callee == Some(callee) => {}
                (Some(schema), Some(expected)) if expected.callee == callee => {
                    let selected = machines
                        .get(&callee)
                        .ok_or("closed reach schema callee is absent")?;
                    let selected = selected
                        .closed_reach_application
                        .as_ref()
                        .ok_or("closed reach schema callee lost its retained application")?;
                    if identity(schema.template_identity)? != identity(selected.template_identity)?
                        || schema.template_commitment != selected.template_commitment
                        || expected.specialization_commitment != selected.specialization_commitment
                        || expected.arguments.len() != selected.telescope.len()
                    {
                        return Err(
                            "closed reach schema call differs from its exact retained application",
                        );
                    }
                    for (argument, parameter) in expected.arguments.iter().zip(&selected.telescope)
                    {
                        let coordinate = match argument {
                            ClosedReachArgument::Type(identity)
                            | ClosedReachArgument::Const(identity)
                            | ClosedReachArgument::Proposition(identity) => *identity,
                            ClosedReachArgument::Machine {
                                identity,
                                contract_commitment,
                            } => {
                                if *contract_commitment == [0; 32] {
                                    return Err(
                                        "closed reach call tuple has an empty machine contract",
                                    );
                                }
                                *identity
                            }
                        };
                        identity(coordinate)?;
                        if !crate::closed_conformances::argument_matches(
                            module, argument, parameter,
                        ) {
                            return Err(
                                "closed reach call tuple differs from its retained telescope",
                            );
                        }
                    }
                }
                _ => return Err("closed reach direct call lost its exact selection or schema"),
            }
        }
        for operation in operations.values() {
            if let Some(binder) = operation.static_reach_binding {
                let call = application
                    .calls
                    .binary_search_by_key(&operation.id, |call| call.operation)
                    .ok()
                    .and_then(|position| application.calls.get(position));
                if call.is_none_or(|call| call.binder != binder) {
                    return Err("closed reach operation marker has no exact call occurrence");
                }
            }
        }
    }
    Ok(())
}

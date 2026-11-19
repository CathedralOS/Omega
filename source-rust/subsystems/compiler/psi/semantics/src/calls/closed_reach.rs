use terminal_psi::{ClosedReachParameter, TerminalModule};

fn validate_telescope(
    kinds: &[u8],
    telescope: &[ClosedReachParameter],
) -> Result<(), &'static str> {
    if kinds.len() != telescope.len()
        || !kinds.iter().zip(telescope).all(|(kind, parameter)| {
            *kind
                == match parameter {
                    ClosedReachParameter::Type { .. } => 0,
                    ClosedReachParameter::Const { .. } => 1,
                    ClosedReachParameter::Proposition { .. } => 2,
                    ClosedReachParameter::Machine(_) => 3,
                }
        })
    {
        return Err("closed reach arguments differ from their complete declaration telescope");
    }
    if telescope.iter().any(|parameter| {
        matches!(
            parameter,
            ClosedReachParameter::Const { argument } if !argument.is_canonical_static_value()
        )
    }) {
        return Err("closed reach telescope has no canonical static carrier and value");
    }
    Ok(())
}

/// Copies of an application commitment do not bind its arguments. Recompute
/// it from the exact template and retained scalar/type arguments before
/// those arguments can identify a caller.
pub fn validate_machine_application_arguments(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
) -> Result<(), &'static str> {
    let specialization = machine
        .declaration
        .as_ref()
        .and_then(|declaration| declaration.specialization.as_ref())
        .ok_or("machine application has no checked specialization")?;
    let application = machine
        .closed_reach_application
        .as_ref()
        .ok_or("machine application has no retained arguments")?;
    if specialization.capture.is_some() || application.capture.is_some() {
        return Err("machine application needs its complete capture argument commitment");
    }
    if specialization.template_semantic_declaration != application.template_identity
        || specialization.commitment != application.specialization_commitment
    {
        return Err("machine application differs from its checked declaration");
    }
    validate_telescope(&application.parameter_kinds, &application.telescope)?;
    let callable = module
        .identities
        .text(specialization.template_callable)
        .ok_or("machine application has no original callable identity")?;
    let mut commitment =
        application_identity::conformance_identity::MachineApplicationCommitment::new(
            callable,
        )?;
    for parameter in &application.telescope {
        match parameter {
            ClosedReachParameter::Type { argument } => commitment.type_argument(argument)?,
            ClosedReachParameter::Const { argument } => commitment.const_argument(argument)?,
            ClosedReachParameter::Proposition { .. } | ClosedReachParameter::Machine(_) => {
                return Err(
                    "machine application needs its complete machine or proof argument commitment",
                );
            }
        }
    }
    if commitment.finish() != specialization.commitment {
        return Err("machine application arguments differ from their retained commitment");
    }
    Ok(())
}

fn validate_operational_contract<'identity>(
    base: [u8; 32],
    commitment: [u8; 32],
    parameters: &[&ClosedReachParameter],
    suspension: &terminal_psi::ContractDependency<bool, Vec<u32>>,
    blocking: &terminal_psi::ContractDependency<bool, Vec<u32>>,
    carry: &[terminal_psi::ConditionalCarryRequirement],
    service_identity: impl Fn(semantic_vocabulary::ServiceId) -> Option<&'identity str>,
) -> Result<(bool, bool), &'static str> {
    if base == [0; 32]
        || terminal_psi::closed_reach_template_commitment(
            base,
            parameters.iter().copied(),
            suspension,
            blocking,
            carry,
            service_identity,
        )? != commitment
    {
        return Err(
            "closed callback contract differs from its committed bounds, dependencies, or conditional carry",
        );
    }
    let evaluate = |term: &terminal_psi::ContractDependency<bool, Vec<u32>>,
                    suspension_axis: bool|
     -> Result<bool, &'static str> {
        let mut possible = term.concrete;
        for parameter in &term.parameters {
            let Some(ClosedReachParameter::Machine(binding)) =
                parameters.get(*parameter as usize).copied()
            else {
                return Err(
                    "closed callback dependency is outside its exact machine telescope",
                );
            };
            possible |= if suspension_axis {
                binding.selected_may_suspend
            } else {
                binding.selected_may_block
            };
        }
        Ok(possible)
    };
    for requirement in carry {
        // Only the suspension permission is guarded. A true guard leaves
        // the ordinary closed-call carry plan responsible for the crossing;
        // this metadata cannot grant ownership or acquisition permission.
        evaluate(&requirement.condition, true)?;
    }
    Ok((evaluate(suspension, true)?, evaluate(blocking, false)?))
}

pub fn validate_closed_reach_applications(module: &TerminalModule) -> Result<(), &'static str> {
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{ClosedReachArgument, TerminalIdentityIndex};

    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    if machines.len() != module.machines.len() {
        return Err("closed reach machine identities are ambiguous");
    }
    let services = module
        .interfaces.services
        .iter()
        .map(|service| (service.id, service))
        .collect::<BTreeMap<_, _>>();
    if services.len() != module.interfaces.services.len() {
        return Err("closed reach service identities are ambiguous");
    }
    let identity = |coordinate: TerminalIdentityIndex| {
        module
            .identities
            .text(coordinate)
            .filter(|text| !text.is_empty())
            .ok_or("closed reach has an invalid or empty identity")
    };
    let service_identity = |service| identity(services.get(&service)?.identity).ok();
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
        if operations.values().any(|operation| {
            operation.static_reach_binding.is_some()
                && operation.kind.module_invocation().is_none()
        }) {
            return Err("closed reach marker is not an ordinary module invocation");
        }
        let invocations = machine
            .module_invocations()
            .map(|invocation| (invocation.site, invocation))
            .collect::<BTreeMap<_, _>>();
        if invocations.len() != machine.module_invocations().count() {
            return Err("closed reach invocation identities are ambiguous");
        }
        let declaration = machine
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.specialization.as_ref());
        let Some(application) = &machine.closed_reach_application else {
            if declaration.is_some() {
                return Err("checked specialization has no retained application");
            }
            if invocations
                .values()
                .any(|invocation| invocation.static_reach_binding.is_some())
            {
                return Err("closed reach invocation marker has no retained application");
            }
            continue;
        };
        identity(application.template_identity)?;
        if application.template_commitment == [0; 32]
            || application.specialization_commitment == [0; 32]
            || declaration.map(|specialization| specialization.commitment)
                != Some(application.specialization_commitment)
        {
            return Err(
                "closed reach application differs from its checked declaration commitment",
            );
        }
        if declaration.map(|specialization| specialization.template_semantic_declaration)
            != Some(application.template_identity)
        {
            return Err("closed reach application differs from its checked template identity");
        }
        match (
            declaration.and_then(|declaration| declaration.capture.as_ref()),
            &application.capture,
        ) {
            (None, None) => {}
            (Some(expected), Some(capture))
                if expected.template_identity == capture.template_identity
                    && expected.template_commitment == capture.template_commitment
                    && expected.specialization_commitment
                        == capture.specialization_commitment =>
            {
                identity(expected.template_identity)?;
            }
            _ => {
                return Err(
                    "closed reach capture differs from its independent checked declaration",
                );
            }
        }
        validate_telescope(&application.parameter_kinds, &application.telescope)?;
        if application.capture.is_none()
            && application.telescope.iter().all(|parameter| {
                matches!(
                    parameter,
                    ClosedReachParameter::Type { .. } | ClosedReachParameter::Const { .. }
                )
            })
        {
            validate_machine_application_arguments(module, machine)?;
        }
        if let Some(capture) = &application.capture {
            validate_telescope(&capture.parameter_kinds, &capture.telescope)?;
            identity(capture.template_identity)?;
            if capture.template_commitment == [0; 32]
                || capture.specialization_commitment == [0; 32]
            {
                return Err("closed reach capture lost its checked application commitment");
            }
        }
        let parameters = application.parameters().collect::<Vec<_>>();
        let (may_suspend, may_block) = validate_operational_contract(
            application.template_base_commitment,
            application.template_commitment,
            &parameters,
            &application.suspension_dependency,
            &application.blocking_dependency,
            &application.conditional_carry,
            service_identity,
        )?;
        let published = machine
            .declaration
            .as_ref()
            .ok_or("closed callback application lost its checked public declaration")?;
        if may_suspend != published.published_may_suspend
            || may_block != published.published_may_block
        {
            return Err(
                "closed callback substitution differs from its published operational contract",
            );
        }
        if let Some(capture) = &application.capture {
            validate_operational_contract(
                capture.template_base_commitment,
                capture.template_commitment,
                // The capture's template still owns the method binders.
                // A closed method supplies those after its capture prefix.
                &parameters,
                &capture.suspension_dependency,
                &capture.blocking_dependency,
                &capture.conditional_carry,
                service_identity,
            )?;
        }
        row(&application.fixed)?;
        row(&machine.published_service_ceiling)?;
        for parameter in application.parameters() {
            match parameter {
                ClosedReachParameter::Type { argument }
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
                    if binding.requirement_parameter_kinds.len()
                        != binding.requirement_arguments.len()
                        || binding
                            .requirement_parameter_kinds
                            .iter()
                            .any(|kind| *kind > 5)
                        || (binding.nominal_requirement.is_none()
                            && !binding.requirement_arguments.is_empty())
                    {
                        return Err(
                            "closed reach requirement lost its full declaration telescope",
                        );
                    }
                    for argument in &binding.requirement_arguments {
                        identity(*argument)?;
                    }
                    if (binding.selected_may_suspend && !binding.upper_bound_may_suspend)
                        || (binding.selected_may_block && !binding.upper_bound_may_block)
                    {
                        return Err(
                            "selected callback operational contract exceeds its requirement bound",
                        );
                    }
                    // effects.md: `crashes none` on a callback binder
                    // admits only crash-free selections, inferred routes
                    // included.
                    if binding.selected_may_crash && binding.upper_bound_crashes_none {
                        return Err(
                            "selected callback may crash, and its requirement declares `crashes none`",
                        );
                    }
                    row(&binding.selected_reach)?;
                    if let Some(upper_bound) = &binding.upper_bound {
                        row(upper_bound)?;
                        if binding
                            .selected_reach
                            .iter()
                            .any(|service| upper_bound.binary_search(service).is_err())
                        {
                            return Err(
                                "closed reach selected contract exceeds its requirement bound",
                            );
                        }
                    }
                    if let Some(callee) = binding.callee {
                        let callee = machines
                            .get(&callee)
                            .ok_or("closed reach selected callee is absent")?;
                        if callee.declaration.as_ref().map(|declaration| {
                            (declaration.semantic_declaration, declaration.contract)
                        }) != Some((
                            binding.selected_identity,
                            binding.selected_contract_commitment,
                        )) || callee.published_service_ceiling != binding.selected_reach
                            || callee.declaration.as_ref().is_none_or(|declaration| {
                                declaration.published_may_suspend
                                    != binding.selected_may_suspend
                                    || declaration.published_may_block
                                        != binding.selected_may_block
                            })
                            || callee.crash_routes.is_empty() == binding.selected_may_crash
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
        let called_binders = application
            .calls
            .iter()
            .map(|call| call.binder)
            .collect::<BTreeSet<_>>();
        for dependency in &application.dependencies {
            let Some(ClosedReachParameter::Machine(binding)) =
                application.parameter(*dependency)
            else {
                return Err("closed reach dependency is outside its machine telescope");
            };
            if binding.nominal_requirement.is_none() && !called_binders.contains(dependency) {
                return Err("closed reach structural dependency has no exact call occurrence");
            }
            union.extend(binding.selected_reach.iter().copied());
        }
        if union.into_iter().collect::<Vec<_>>() != machine.published_service_ceiling {
            return Err("closed reach finite union differs from its published service row");
        }
        if application
            .calls
            .windows(2)
            .any(|pair| pair[0].site >= pair[1].site)
        {
            return Err("closed reach call occurrences are not strictly ordered");
        }
        for call in &application.calls {
            let invocation = invocations
                .get(&call.site)
                .ok_or("closed reach call has no exact owner invocation")?;
            if invocation.static_reach_binding != Some(call.binder) {
                return Err("closed reach call differs from its invocation binder marker");
            }
            let Some(ClosedReachParameter::Machine(binding)) =
                application.parameter(call.binder)
            else {
                return Err("closed reach call is outside its machine telescope");
            };
            let callee = invocation.inputs.callee;
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
                    if identity(schema.template_identity)?
                        != identity(selected.template_identity)?
                        || schema.template_commitment != selected.template_commitment
                        || expected.specialization_commitment
                            != selected.specialization_commitment
                        || expected.arguments.len() != selected.telescope.len()
                    {
                        return Err(
                            "closed reach schema call differs from its exact retained application",
                        );
                    }
                    for (argument, parameter) in
                        expected.arguments.iter().zip(&selected.telescope)
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
                        if !crate::calls::closed_conformances::argument_matches(
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
        for invocation in invocations.values() {
            if let Some(binder) = invocation.static_reach_binding {
                let call = application
                    .calls
                    .binary_search_by_key(&invocation.site, |call| call.site)
                    .ok()
                    .and_then(|position| application.calls.get(position));
                if call.is_none_or(|call| call.binder != binder) {
                    return Err("closed reach invocation marker has no exact call occurrence");
                }
            }
        }
    }
    Ok(())
}

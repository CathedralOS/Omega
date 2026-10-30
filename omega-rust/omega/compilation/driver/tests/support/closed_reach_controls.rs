//! Mutate real source-produced reach records independently of their call bodies.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{ClosedReachApplication, ClosedReachParameter, TerminalModule};

fn parameter_mut(
    application: &mut ClosedReachApplication,
    binder: u32,
) -> Option<&mut ClosedReachParameter> {
    let prefix = application
        .capture
        .as_ref()
        .map_or(0, |capture| capture.telescope.len());
    if (binder as usize) < prefix {
        application
            .capture
            .as_mut()?
            .telescope
            .get_mut(binder as usize)
    } else {
        application.telescope.get_mut(binder as usize - prefix)
    }
}

pub(super) fn inspect(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let (owner, dependency) = module
        .machines
        .iter()
        .enumerate()
        .find_map(|(owner, machine)| {
            if !machine.published_service_ceiling.is_empty() {
                return None;
            }
            let application = machine.closed_reach_application.as_ref()?;
            application
                .dependencies
                .first()
                .copied()
                .map(|binder| (owner, binder))
        })
        .ok_or("reach controls need a pure original nominal callback dependency")?;
    let application = module.machines[owner]
        .closed_reach_application
        .as_ref()
        .unwrap();
    let mut controls: Vec<(&str, &str, TerminalModule)> = Vec::new();

    let mut bad = module.clone();
    let fixed = &mut bad.machines[owner]
        .closed_reach_application
        .as_mut()
        .unwrap()
        .fixed;
    if let Some(service) = module
        .services
        .iter()
        .filter(|service| service.parents.is_empty())
        .map(|service| service.id)
        .find(|service| fixed.binary_search(service).is_err())
    {
        fixed.push(service);
        fixed.sort();
    } else if fixed.pop().is_none() {
        return Err("reach controls need a service to distinguish the closed union".to_owned());
    }
    controls.push(("changed fixed contribution", "finite union", bad));

    let mut bad = module.clone();
    bad.machines[owner]
        .closed_reach_application
        .as_mut()
        .unwrap()
        .dependencies
        .insert(0, dependency);
    controls.push(("duplicate dependency binder", "dependency binders", bad));

    let mut bad = module.clone();
    bad.machines[owner]
        .closed_reach_application
        .as_mut()
        .unwrap()
        .dependencies
        .push(u32::MAX);
    controls.push((
        "incomplete dependency telescope",
        "outside its machine telescope",
        bad,
    ));

    let mut bad = module.clone();
    let ClosedReachParameter::Machine(binding) = parameter_mut(
        bad.machines[owner]
            .closed_reach_application
            .as_mut()
            .unwrap(),
        dependency,
    )
    .unwrap() else {
        return Err("dependency is not a machine".to_owned());
    };
    binding.selected_contract_commitment = [0; 32];
    controls.push(("missing selected public contract", "public contract", bad));

    if !application.calls.is_empty() {
        let mut bad = module.clone();
        bad.machines[owner]
            .closed_reach_application
            .as_mut()
            .unwrap()
            .calls
            .remove(0);
        controls.push((
            "removed original call occurrence",
            "no exact call occurrence",
            bad,
        ));

        // This control specifically duplicates an operation owner. Retain a
        // real normal invocation; a terminal edge has no operation identity.
        let operation_id = application
            .calls
            .iter()
            .find_map(|call| call.site.operation())
            .ok_or("reach controls need a normal operation-owned call occurrence")?;
        let mut bad = module.clone();
        let operation = bad.machines[owner]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| operation.id == operation_id)
            .ok_or("missing marked operation")?;
        operation.static_reach_binding = Some(u32::MAX);
        controls.push((
            "redirected operation binder",
            "invocation binder marker",
            bad,
        ));

        let mut bad = module.clone();
        let block = bad.machines[owner]
            .blocks
            .iter_mut()
            .find(|block| {
                block
                    .operations
                    .iter()
                    .any(|operation| operation.id == operation_id)
            })
            .ok_or("missing marked block")?;
        let operation = block
            .operations
            .iter()
            .find(|operation| operation.id == operation_id)
            .unwrap()
            .clone();
        block.operations.push(operation);
        controls.push((
            "ambiguous owner operation",
            "operation identities are ambiguous",
            bad,
        ));

        let mut bad = module.clone();
        bad.machines[owner].closed_reach_application = None;
        controls.push((
            "removed retained application",
            "no retained application",
            bad,
        ));
    }

    let mut changed_selection = false;
    for (position, machine) in module.machines.iter().enumerate() {
        let Some(application) = &machine.closed_reach_application else {
            continue;
        };
        for (binder, parameter) in application.parameters().enumerate() {
            let ClosedReachParameter::Machine(binding) = parameter else {
                continue;
            };
            if binding.selected_reach.is_empty() || binding.callee.is_none() {
                continue;
            }
            let mut bad = module.clone();
            let ClosedReachParameter::Machine(binding) = parameter_mut(
                bad.machines[position]
                    .closed_reach_application
                    .as_mut()
                    .unwrap(),
                binder as u32,
            )
            .unwrap() else {
                unreachable!()
            };
            binding.selected_reach.clear();
            controls.push(("removed selected reach", "retained callee contract", bad));
            changed_selection = true;
            break;
        }
        if changed_selection {
            break;
        }
    }

    let redirected = module
        .machines
        .iter()
        .enumerate()
        .find_map(|(position, machine)| {
            let application = machine.closed_reach_application.as_ref()?;
            application
                .calls
                .iter()
                .enumerate()
                .find_map(|(call_index, call)| {
                    let invocation = machine
                        .module_invocations()
                        .find(|invocation| invocation.site == call.site)?;
                    let callee = module.machine(invocation.inputs.callee)?;
                    let declaration = callee.declaration.as_ref()?;
                    let alternative = module.machines.iter().find(|alternative| {
                        alternative.id != callee.id
                            && alternative.declaration.as_ref().is_some_and(|other| {
                                other.parameter_types == declaration.parameter_types
                                    && other.result_type == declaration.result_type
                            })
                    })?;
                    Some((position, call_index, call.binder, alternative.id))
                })
        });
    if let Some((position, call, binder, alternative)) = redirected {
        let mut bad = module.clone();
        let application = bad.machines[position]
            .closed_reach_application
            .as_mut()
            .unwrap();
        if let Some(expected) = &mut application.calls[call].application {
            expected.callee = alternative;
        } else {
            let ClosedReachParameter::Machine(binding) =
                parameter_mut(application, binder).unwrap()
            else {
                unreachable!()
            };
            binding.callee = Some(alternative);
        }
        controls.push((
            "redirected same-shaped selected callee",
            "closed reach",
            bad,
        ));
    } else {
        return Err("reach controls need two distinct same-shaped selected callables".to_owned());
    }

    // Exact schema applications own a second tuple independent of the callee's
    // retained telescope. A missing element cannot be inferred from its body.
    for (position, machine) in module.machines.iter().enumerate() {
        let Some(application) = &machine.closed_reach_application else {
            continue;
        };
        if let Some((call, _)) = application.calls.iter().enumerate().find(|(_, call)| {
            call.application
                .as_ref()
                .is_some_and(|application| !application.arguments.is_empty())
        }) {
            let mut bad = module.clone();
            bad.machines[position]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[call]
                .application
                .as_mut()
                .unwrap()
                .arguments
                .pop();
            controls.push((
                "incomplete schema invocation tuple",
                "exact retained application",
                bad,
            ));
            break;
        }
    }
    for (position, machine) in module.machines.iter().enumerate() {
        let Some(application) = &machine.closed_reach_application else {
            continue;
        };
        if !application.telescope.is_empty() {
            let mut bad = module.clone();
            bad.machines[position]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope
                .pop();
            // Omit the corresponding consumer coordinates too: the independent
            // declaration roster, rather than tuple disagreement, must reject.
            for consumer in &mut bad.machines {
                if let Some(application) = &mut consumer.closed_reach_application {
                    for call in &mut application.calls {
                        if let Some(invocation) = &mut call.application
                            && invocation.callee == machine.id
                        {
                            invocation.arguments.pop();
                        }
                    }
                }
            }
            controls.push((
                "omitted declaration argument",
                "complete declaration telescope",
                bad,
            ));
        }
        if let Some((binder, _)) = application.parameters().enumerate().find(|(_, parameter)| {
            matches!(parameter, ClosedReachParameter::Machine(binding) if !binding.requirement_arguments.is_empty())
        }) {
            let mut bad = module.clone();
            let ClosedReachParameter::Machine(binding) = parameter_mut(
                bad.machines[position].closed_reach_application.as_mut().unwrap(), binder as u32,
            ).unwrap() else { unreachable!() };
            binding.requirement_arguments.pop();
            controls.push(("omitted nominal requirement argument", "full declaration telescope", bad));
        }
    }
    for (name, reason, bad) in controls {
        super::reject_module_at_ingress(&bad, name, reason)?;
    }
    Ok(())
}

pub(super) fn inspect_captures(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let owners = module
        .machines
        .iter()
        .enumerate()
        .filter_map(|(position, machine)| {
            machine
                .closed_reach_application
                .as_ref()?
                .capture
                .as_ref()?;
            Some(position)
        })
        .collect::<Vec<_>>();
    if owners.is_empty() {
        return Err("capture controls need a retained source capture".to_owned());
    }
    for position in owners {
        let mut bad = module.clone();
        bad.machines[position]
            .closed_reach_application
            .as_mut()
            .unwrap()
            .capture = None;
        super::reject_module_at_ingress(
            &bad,
            "omitted whole capture prefix",
            "independent checked declaration",
        )?;
        let mut bad = module.clone();
        bad.machines[position].closed_reach_application = None;
        super::reject_module_at_ingress(
            &bad,
            "omitted unused captured application",
            "no retained application",
        )?;
        let mut bad = module.clone();
        bad.machines[position]
            .declaration
            .as_mut()
            .unwrap()
            .specialization
            .as_mut()
            .unwrap()
            .capture = None;
        super::reject_module_at_ingress(
            &bad,
            "missing independent capture declaration",
            "independent checked declaration",
        )?;
        let capture = module.machines[position]
            .closed_reach_application
            .as_ref()
            .unwrap()
            .capture
            .as_ref()
            .unwrap();
        if !capture.telescope.is_empty() {
            let mut bad = module.clone();
            bad.machines[position]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .capture
                .as_mut()
                .unwrap()
                .telescope
                .pop();
            super::reject_module_at_ingress(
                &bad,
                "incomplete capture telescope",
                "complete declaration telescope",
            )?;
        }
        if let Some(other) = module
            .machines
            .iter()
            .enumerate()
            .find_map(|(other, machine)| {
                let candidate = machine
                    .closed_reach_application
                    .as_ref()?
                    .capture
                    .as_ref()?;
                (other != position
                    && candidate.specialization_commitment != capture.specialization_commitment)
                    .then_some(candidate)
            })
        {
            let mut bad = module.clone();
            bad.machines[position]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .capture = Some(other.clone());
            super::reject_module_at_ingress(
                &bad,
                "redirected capture selection",
                "independent checked declaration",
            )?;
        }
    }
    Ok(())
}

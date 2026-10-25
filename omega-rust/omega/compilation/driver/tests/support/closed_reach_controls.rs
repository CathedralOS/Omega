//! Mutate real source-produced reach records independently of their call bodies.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{ClosedReachParameter, TerminalModule};

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
    let ClosedReachParameter::Machine(binding) = &mut bad.machines[owner]
        .closed_reach_application
        .as_mut()
        .unwrap()
        .telescope[dependency as usize]
    else {
        return Err("dependency is not a machine".to_owned());
    };
    binding.selected_contract_commitment = [0; 32];
    controls.push(("missing selected public contract", "public contract", bad));

    if let Some(call) = application.calls.first() {
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

        let mut bad = module.clone();
        let operation = bad.machines[owner]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| operation.id == call.operation)
            .ok_or("missing marked operation")?;
        operation.static_reach_binding = Some(u32::MAX);
        controls.push((
            "redirected operation binder",
            "operation binder marker",
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
                    .any(|operation| operation.id == call.operation)
            })
            .ok_or("missing marked block")?;
        let operation = block
            .operations
            .iter()
            .find(|operation| operation.id == call.operation)
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
        for (binder, parameter) in application.telescope.iter().enumerate() {
            let ClosedReachParameter::Machine(binding) = parameter else {
                continue;
            };
            if binding.selected_reach.is_empty() || binding.callee.is_none() {
                continue;
            }
            let mut bad = module.clone();
            let ClosedReachParameter::Machine(binding) = &mut bad.machines[position]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[binder]
            else {
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
                    let operation = machine
                        .operations()
                        .find(|operation| operation.id == call.operation)?;
                    let callee = module.machine(operation.kind.direct_callee()?)?;
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
                &mut application.telescope[binder as usize]
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
    for (name, reason, bad) in controls {
        super::reject_module_at_ingress(&bad, name, reason)?;
    }
    Ok(())
}

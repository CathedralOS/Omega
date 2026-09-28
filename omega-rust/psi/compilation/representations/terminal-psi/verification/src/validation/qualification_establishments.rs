//! Establishment bindings replay each result membership's authorized route
//! against the retained issuer authority of its exact callee.
//!
//! A binding claims the enclosing operation is the establishing occurrence
//! the domain declaration authorized: `boundary` callees replay against the
//! declaration's retained requirement identity, ordinary callees against a
//! conformance application owned by that callee naming the requirement row,
//! and dynamic call sites against the dispatch row's selected requirement
//! and realization identities. A binding whose enclosing operation has no
//! retained issuer identity — or names a route the operation cannot have
//! exercised — is rejected rather than trusted.

use std::collections::BTreeMap;

use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use terminal_psi::{OperationKind, OperationResult, StructuralEstablishmentRoute, TerminalModule};

use super::ModuleError;

/// Selected requirement and realization callable identities one dynamic call
/// operation may name. Materialized descriptor dispatches retain both on the
/// dispatch row; a parameter descriptor's requirement resolves through its
/// declared interface slot, and its realization is chosen by the incoming
/// table per invocation, so no static realization identity exists for it.
fn dynamic_dispatch_identities(
    module: &TerminalModule,
    caller: MachineId,
    operation: OperationId,
) -> (Vec<&'_ str>, Vec<&'_ str>) {
    let mut requirements = Vec::new();
    let mut realizations = Vec::new();
    for (dispatched, requirement, realization) in module
        .dynamic_dispatch
        .direct_dispatches
        .iter()
        .map(|dispatch| {
            (
                dispatch.operation,
                dispatch.public_requirement_identity.as_str(),
                dispatch.realization_callable_identity.as_str(),
            )
        })
        .chain(
            module
                .dynamic_dispatch
                .indirect_dispatches
                .iter()
                .map(|dispatch| {
                    (
                        dispatch.operation,
                        dispatch.public_requirement_identity.as_str(),
                        dispatch.realization_callable_identity.as_str(),
                    )
                }),
        )
        .chain(
            module
                .dynamic_dispatch
                .stored_dispatches
                .iter()
                .map(|dispatch| {
                    (
                        dispatch.operation,
                        dispatch.public_requirement_identity.as_str(),
                        dispatch.realization_callable_identity.as_str(),
                    )
                }),
        )
    {
        if dispatched == operation {
            requirements.push(requirement);
            realizations.push(realization);
        }
    }
    for dispatch in &module.dynamic_dispatch.parameter_dispatches {
        if dispatch.operation != operation {
            continue;
        }
        let Some(parameter) = module.dynamic_dispatch.parameters.iter().find(|parameter| {
            parameter.owner == caller && parameter.ordinal == dispatch.parameter_ordinal
        }) else {
            continue;
        };
        if let Some(requirement) = parameter
            .requirements
            .get(usize::try_from(dispatch.requirement_slot).unwrap_or(usize::MAX))
        {
            requirements.push(requirement.public_requirement_identity.as_str());
        }
    }
    (requirements, realizations)
}

pub(super) fn validate(module: &TerminalModule) -> Result<(), ModuleError> {
    let domains = module
        .structural_domains
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let boundary_identities = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<BTreeMap<BoundaryMachineId, &str>>();
    // `owner` names the machine the closed conformance application was
    // selected for — the callee under its valid conformance — and each row
    // joins a public requirement identity to the exact realization row.
    let conformance_applications = module.closed_conformance_applications.iter().fold(
        BTreeMap::<MachineId, Vec<_>>::new(),
        |mut applications, application| {
            applications
                .entry(application.owner)
                .or_default()
                .push(application);
            applications
        },
    );
    let callable_identities = module
        .closed_conformance_applications
        .iter()
        .flat_map(|application| application.realization_callables.iter())
        .fold(
            BTreeMap::<MachineId, Vec<&str>>::new(),
            |mut identities, callable| {
                identities
                    .entry(callable.machine)
                    .or_default()
                    .push(callable.source_callable_identity.as_str());
                identities
            },
        );
    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                let OperationResult::Structural(result) = &operation.result else {
                    continue;
                };
                if result.qualification_establishments.is_empty() {
                    continue;
                }
                if result
                    .qualification_establishments
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                {
                    return Err(ModuleError::MalformedQualificationEstablishment {
                        operation: operation.id,
                        domain: None,
                    });
                }
                for binding in &result.qualification_establishments {
                    let declaration = domains
                        .get(&binding.domain)
                        .ok_or(ModuleError::UnknownStructuralDomain(binding.domain))?;
                    let qualified = result.qualifications.contains(&binding.domain)
                        || result
                            .projected_qualifications
                            .iter()
                            .any(|projection| projection.domain == binding.domain);
                    if !qualified {
                        return Err(ModuleError::MalformedQualificationEstablishment {
                            operation: operation.id,
                            domain: Some(binding.domain),
                        });
                    }
                    let Some(route) = declaration
                        .establishment_routes
                        .get(usize::try_from(binding.route).unwrap_or(usize::MAX))
                    else {
                        return Err(ModuleError::MalformedQualificationEstablishment {
                            operation: operation.id,
                            domain: Some(binding.domain),
                        });
                    };
                    let replayed = match route {
                        StructuralEstablishmentRoute::BoundaryRequirement { requirement } => {
                            match &operation.kind {
                                OperationKind::BoundaryCall { boundary, .. } => {
                                    boundary_identities.get(boundary) == Some(&requirement.as_str())
                                }
                                _ => false,
                            }
                        }
                        StructuralEstablishmentRoute::Requirement { requirement } => {
                            match &operation.kind {
                                OperationKind::Call { callee, .. }
                                | OperationKind::CallUnit { callee, .. }
                                | OperationKind::CallStructuralScalar { callee, .. }
                                | OperationKind::CallStructural { callee, .. }
                                | OperationKind::CallStructuralWithScalarArguments {
                                    callee, ..
                                } => conformance_applications
                                    .get(callee)
                                    .into_iter()
                                    .flatten()
                                    .flat_map(|application| application.rows.iter())
                                    .any(|row| row.public_requirement_identity == *requirement),
                                OperationKind::CallDynamicScalar { .. }
                                | OperationKind::CallDynamicParameterScalar { .. }
                                | OperationKind::CallDynamicUnit { .. }
                                | OperationKind::CallDynamicParameterUnit { .. } => {
                                    dynamic_dispatch_identities(module, machine.id, operation.id)
                                        .0
                                        .iter()
                                        .any(|identity| *identity == requirement)
                                }
                                _ => false,
                            }
                        }
                        StructuralEstablishmentRoute::ExactMachine { machine: expected } => {
                            match &operation.kind {
                                OperationKind::Call { callee, .. }
                                | OperationKind::CallUnit { callee, .. }
                                | OperationKind::CallStructuralScalar { callee, .. }
                                | OperationKind::CallStructural { callee, .. }
                                | OperationKind::CallStructuralWithScalarArguments {
                                    callee, ..
                                } => callable_identities
                                    .get(callee)
                                    .into_iter()
                                    .flatten()
                                    .any(|identity| *identity == expected),
                                OperationKind::CallDynamicScalar { .. }
                                | OperationKind::CallDynamicParameterScalar { .. }
                                | OperationKind::CallDynamicUnit { .. }
                                | OperationKind::CallDynamicParameterUnit { .. } => {
                                    dynamic_dispatch_identities(module, machine.id, operation.id)
                                        .1
                                        .iter()
                                        .any(|identity| *identity == expected)
                                }
                                _ => false,
                            }
                        }
                    };
                    if !replayed {
                        return Err(ModuleError::MalformedQualificationEstablishment {
                            operation: operation.id,
                            domain: Some(binding.domain),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

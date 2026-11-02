//! Close the prepared calling census over exact emitted declarations.
//!
//! Storage was collected in checking's one type namespace and published by
//! stage05. This owner retains ordered policy requests and direct invocation
//! associations while their real Source coordinates are still available. No
//! identity is rediscovered from a callable's display name or equal ABI shape.

use checked_trees::{CheckedTrees, PreparedProviderInventory};
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use symbols::SymbolHandle;
use terminal_psi::{
    CallingPolicyInstance, CallingPolicyRequest, CallingSignatureDeclaration,
    TargetMachineCallable, TerminalModule,
};

pub(crate) fn capture(
    checked: &CheckedTrees,
    inventory: &PreparedProviderInventory,
    module: &mut TerminalModule,
    boundaries: &[(SymbolHandle, BoundaryMachineId)],
    machines: &[(SymbolHandle, MachineId)],
) -> Result<(), Diagnostic> {
    let services = inventory.services_for(checked)?;
    let signatures = services.signatures();
    for &(source, boundary) in boundaries {
        let Some(application) = boundary_application(checked, signatures, source) else {
            // Synthetic/runtime family interfaces are not authored ABI rows.
            // Their absence is not a default contract or permission to invoke.
            continue;
        };
        module.calling_signatures.push(CallingSignatureDeclaration {
            boundary,
            application,
        });
    }
    module.calling_signatures.sort_by_key(|row| row.boundary);
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    let calling = signatures.calling();
    for request in calling.requests() {
        let declaration = checked
            .typed
            .normalized_hermetic_symbol_identity(request.boundary)
            .map(|identity| identities.intern(&identity));
        let instances = (|| {
            let source_instances = calling.instances(request).map_err(|errors| {
                errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect::<Vec<_>>()
            })?;
            let mut instances = Vec::with_capacity(source_instances.len());
            let owner = checked
                .typed
                .trait_definition_by_symbol(request.boundary)
                .ok_or_else(|| vec!["calling policy request lost its exact trait".to_owned()])?;
            for instance in source_instances {
                let scope = typed_trees::trait_definition::AppliedTraitScope::new(
                    &checked.typed,
                    owner,
                    typed_trees::trait_definition::TraitApplicationArguments {
                        lexical_owner: instance.argument_owner,
                        enclosing_data_applications: &[],
                        arguments: calling.arguments(instance),
                        lifetimes: &[],
                    },
                )
                .ok_or_else(|| {
                    vec!["calling policy instance lost its complete application".to_owned()]
                })?;
                let outer = scope
                    .portable_application_identity(&checked.typed)
                    .and_then(|identity| checked.typed.type_identity_text(identity))
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| {
                        vec!["calling policy instance lost its portable application".to_owned()]
                    })?;
                let policy = instance
                    .policy
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|source| {
                        let mut found =
                            machines
                                .iter()
                                .filter(|(owner, _)| owner == source)
                                .map(|(_, identity)| TargetMachineCallable::Machine(*identity))
                                .chain(boundaries.iter().filter(|(owner, _)| owner == source).map(
                                    |(_, identity)| TargetMachineCallable::Boundary(*identity),
                                ));
                        let callable = found
                            .next()
                            .ok_or("calling policy has no exact published callable")?;
                        if found.next().is_some() {
                            return Err("calling policy has multiple published owners".into());
                        }
                        Ok(callable)
                    });
                let applications = (|| {
                    let mut applications = Vec::new();
                    for application in calling.signatures(instance) {
                        if let Some(reason) = &application.abi_refusal {
                            return Err(reason.clone());
                        }
                        applications.extend(application.abi_signatures.iter().copied());
                    }
                    Ok(applications)
                })();
                instances.push(CallingPolicyInstance {
                    declaring_application: identities.intern(&outer),
                    policy,
                    signatures: applications,
                });
            }
            Ok(instances)
        })();
        module.calling_policy_requests.push(CallingPolicyRequest {
            declaration,
            display_name: request.name.clone(),
            instances,
        });
    }
    module.identities = identities.build();
    Ok(())
}

/// Direct calls to an external satisfier cross the public requirement's ABI.
/// Keep that checked Source association here; native consumers receive the
/// same exact declaration key whether the call names the requirement or leaf.
fn boundary_application(
    checked: &CheckedTrees,
    signatures: &typed_trees::service_declarations::signatures::PreparedBoundarySignatures,
    source: SymbolHandle,
) -> Option<Result<semantic_vocabulary::CallingSignatureId, String>> {
    if let Some(application) = signatures.abi_for_boundary(source) {
        return Some(application.clone());
    }
    let typed = &checked.typed;
    let machine = typed.machine_by_symbol(source)?;
    if !matches!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::ExternalRealization { .. }
    ) {
        return None;
    }
    let mut applications = Vec::new();
    for edge in typed.machine_trait_conformances(machine) {
        let mut edge_applications = Vec::new();
        let scope = typed
            .trait_machine_signature(edge.requirement_symbol)
            .map(|(owner, _)| {
                typed_trees::trait_definition::AppliedTraitScope::from_machine_conformance(
                    typed, machine, edge,
                )
                .and_then(|scope| scope.declaring_scope(typed, owner.symbol))
                .ok_or_else(|| {
                    "external satisfier lost its complete declaring application".to_owned()
                })
            })
            .transpose();
        let scope = match scope {
            Ok(scope) => scope,
            Err(reason) => return Some(Err(reason)),
        };
        // The direct satisfaction owns its closed tuples. A shared realization
        // must not borrow a named implementation's selecting declaration.
        for association in checked
            .facts
            .provider_closed_requirements
            .iter()
            .filter(|association| {
                association.provider == source
                    && association.selecting_declaration == source
                    && association.template == edge.requirement_symbol
            })
        {
            let receiving_matches = match &scope {
                Some(scope) => typed
                    .closed_requirement_application(association.closed_requirement)
                    .is_some_and(|application| {
                        application.template == association.template
                            && application.trait_application_identity
                                == association.receiving_application
                            && typed.requirement_application_matches_scope(
                                association.closed_requirement,
                                scope,
                            )
                    }),
                None => association.receiving_application == Default::default(),
            };
            if !receiving_matches {
                return Some(Err(
                    "external satisfier changed its checked receiving application".into(),
                ));
            }
            edge_applications.push(
                signatures
                    .abi_for_boundary(association.closed_requirement)
                    .cloned()
                    .unwrap_or_else(|| {
                        Err("external satisfier lost its checked closed declaration ABI".into())
                    }),
            );
        }
        match scope {
            Some(scope) => {
                let Some(outer) = scope
                    .portable_application_identity(typed)
                    .and_then(|identity| typed.type_identity_text(identity))
                else {
                    return Some(Err(
                        "external satisfier lost its portable declaring application".into(),
                    ));
                };
                // Empty local operands use the declaration application itself.
                // Every closed local tuple instead keeps its checked fact above.
                edge_applications.extend(
                    signatures
                        .abi_signatures()
                        .iter()
                        .filter(|declaration| {
                            declaration
                                .source_templates
                                .contains(&edge.requirement_symbol)
                                && declaration.declaring_application == outer
                                && declaration.operands.is_empty()
                                && declaration
                                    .formals
                                    .iter()
                                    .all(|formal| formal.callback.is_some())
                        })
                        .map(|declaration| Ok(declaration.id)),
                );
            }
            None => {
                if let Some(application) = signatures.abi_for_boundary(edge.requirement_symbol) {
                    edge_applications.push(application.clone());
                }
            }
        }
        if edge_applications.is_empty() {
            return Some(Err(
                "external satisfaction edge has no exact checked declaration ABI application"
                    .into(),
            ));
        }
        for application in edge_applications {
            if let Err(reason) = &application {
                return Some(Err(reason.clone()));
            }
            if !applications.contains(&application) {
                applications.push(application);
            }
        }
    }
    Some(match applications.as_slice() {
        [application] => application.clone(),
        [] => Err("external satisfier has no exact checked declaration ABI application".into()),
        _ => Err("external satisfier has competing checked declaration ABI applications".into()),
    })
}

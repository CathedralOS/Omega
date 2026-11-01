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
        let Some(application) = signatures.abi_for_boundary(source) else {
            // Synthetic/runtime family interfaces are not authored ABI rows.
            // Their absence is not a default contract or permission to invoke.
            continue;
        };
        module.calling_signatures.push(CallingSignatureDeclaration {
            boundary,
            application: application.clone(),
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

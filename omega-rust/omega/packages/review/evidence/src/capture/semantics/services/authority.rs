//! Source-owned service and progress declarations, never display-name lookup.

mod progress;

use super::*;
use crate::capture::behavior::{project_service_row, project_synchronous_invocations};
use crate::record::PackagePolicyServiceAuthority;

pub(super) fn project(
    compilation: &CheckedCompilation,
    declaration: ProviderSchemaDeclaration,
    requirement: SymbolHandle,
) -> Result<PackagePolicyServiceAuthority, Vec<Diagnostic>> {
    let (row, implicit_service, invocations, parameters, guarantee) = match declaration {
        ProviderSchemaDeclaration::BoundaryTrait(symbol) => {
            let owners = compilation
                .traits()
                .iter()
                .filter(|owner| owner.symbol == symbol)
                .collect::<Vec<_>>();
            let [owner] = owners.as_slice() else {
                return Err(rejected("service authority has no unique declaring trait"));
            };
            let requirements = compilation
                .trait_machine_signatures(owner)
                .iter()
                .filter(|signature| signature.symbol == requirement)
                .collect::<Vec<_>>();
            let [signature] = requirements.as_slice() else {
                return Err(rejected(
                    "service authority has no unique declaring signature",
                ));
            };
            (
                signature.service_reach_row,
                owner.is_boundary.then_some(symbol),
                flow_effects::declared_signature_invocations(&compilation.typed, signature),
                compilation.state_signature_parameters(signature),
                &signature.termination_guarantee,
            )
        }
        ProviderSchemaDeclaration::BoundaryRequirement(symbol) => {
            let machines = compilation
                .machines()
                .iter()
                .filter(|machine| machine.symbol == symbol && symbol == requirement)
                .collect::<Vec<_>>();
            let [machine] = machines.as_slice() else {
                return Err(rejected(
                    "service authority has no unique top-level requirement",
                ));
            };
            let entry = compilation
                .machine_states(machine)
                .first()
                .ok_or_else(|| rejected("service authority lacks its requirement entry"))?;
            let guarantee = machine
                .termination_plan
                .interface
                .published()
                .ok_or_else(|| {
                    rejected("service authority lacks its published termination interface")
                })?;
            (
                machine.service_reach_row,
                None,
                flow_effects::declared_machine_invocations(&compilation.typed, machine),
                compilation.state_parameters(entry),
                guarantee,
            )
        }
        ProviderSchemaDeclaration::BoundaryOperator(_) => {
            // ServiceSchema::from_typed_operator exposes no service-reach,
            // invocation or progress rows; operator contracts stay separate.
            return Ok(PackagePolicyServiceAuthority {
                service_reach: Vec::new(),
                synchronous_invocations: Vec::new(),
                progress_premises: Vec::new(),
            });
        }
    };
    let mut service_reach = project_service_row(compilation, row)?;
    if let Some(symbol) = implicit_service {
        let service = compilation
            .service_reaches
            .id_for_symbol(symbol)
            .ok_or_else(|| rejected("declaring boundary has no exact service identity"))?;
        let mut closure = Vec::new();
        compilation
            .service_reaches
            .extend_closure(service, &mut closure);
        for service in closure {
            let definition = compilation
                .service_reaches
                .definition(service)
                .ok_or_else(|| rejected("declaring boundary closure has an unknown service"))?;
            service_reach.push(nominal_identity(compilation, definition.symbol)?);
        }
    }
    service_reach.sort();
    service_reach.dedup();
    Ok(PackagePolicyServiceAuthority {
        service_reach,
        synchronous_invocations: project_synchronous_invocations(compilation, &invocations)?,
        progress_premises: progress::project(compilation, guarantee, parameters)?,
    })
}

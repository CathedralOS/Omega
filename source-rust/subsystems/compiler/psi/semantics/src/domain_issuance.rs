//! Which machines may establish a domain under its checked issuance plan.

use semantic_vocabulary::{DomainSemanticId, EdgeId, MachineId};
use terminal_psi::{StructuralEstablishmentRoute, TerminalMachine, TerminalModule};

/// The cast has no membership to forward and its machine is outside the
/// domain's checked issuer routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeniedEstablishment {
    pub machine: MachineId,
    pub edge: EdgeId,
    pub domain: DomainSemanticId,
}

/// Boundary requirements establish membership on the exact completed call.
/// Only guest routes authorize a cast in the body of this machine.
pub(crate) fn excludes_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
    routes: &[StructuralEstablishmentRoute],
) -> bool {
    machine.declaration.as_ref().is_some_and(|declaration| {
        routes
            .iter()
            .all(|route| !issuer_route_authorizes(module, machine, declaration, route))
    })
}

/// Whether `machine`, by its checked declaration, is the callable a route
/// names: the exact machine (or the template a specialization instantiates),
/// or a closed conformance realizing the exact requirement with it.
pub(crate) fn issuer_route_authorizes(
    module: &TerminalModule,
    machine: &TerminalMachine,
    declaration: &terminal_psi::CheckedMachineDeclaration,
    route: &StructuralEstablishmentRoute,
) -> bool {
    match route {
        StructuralEstablishmentRoute::ExactMachine {
            declaration: issuer,
            machine_identity,
        } => {
            (module
                .identities
                .text(*machine_identity)
                .zip(module.identities.text(declaration.callable))
                .is_some_and(|(issued, callable)| {
                    issued
                        == terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
                            callable,
                            declaration
                                .specialization
                                .as_ref()
                                .map(|application| &application.commitment),
                        )
                })
                && issuer.identity() == declaration.semantic_declaration)
                || declaration
                    .specialization
                    .as_ref()
                    .is_some_and(|specialization| {
                        *machine_identity == specialization.template_callable
                            && issuer.identity() == specialization.template_semantic_declaration
                            && module
                                .identities
                                .text(specialization.template_callable)
                                .is_some_and(|identity| !identity.is_empty())
                    })
        }
        StructuralEstablishmentRoute::CheckedRequirement {
            declaration: issuer,
            requirement_identity,
        } => module
            .proofs
            .closed_conformance_applications
            .iter()
            .any(|application| {
                application.rows.iter().any(|row| {
                    row.public_requirement_identity == *requirement_identity
                        && row.requirement_declaration == *issuer
                        && row.realization_callable_identity == Some(declaration.callable)
                        && application.realization_callables.iter().any(|callable| {
                            callable.source_callable_identity == declaration.callable
                                && callable.callable
                                    == terminal_psi::TargetMachineCallable::Machine(machine.id)
                        })
                })
            }),
        StructuralEstablishmentRoute::BoundaryRequirement { .. } => false,
    }
}

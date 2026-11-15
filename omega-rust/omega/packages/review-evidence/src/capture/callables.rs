//! Build runs under its own judgment before Product is prepared. Read that
//! retained owner, and rejoin its original declaration to the final package;
//! Product syntax or configuration values cannot supply checked Build facts.

use super::PackageReviewInput;
use super::declarations::{nominal_identity, unavailable};
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyCrash, PackagePolicyCrashGuard,
    PackagePolicyCrashRoute, PackagePolicyInferredCrash, PackagePolicyMutation,
    PackagePolicyServiceReachDependency, PackagePolicyTargetContext, PackagePolicyTermination,
    PackageReviewCallableSupply, PackageReviewCheckedServiceReach, PackageReviewCrashCause,
    PackageReviewCrashInterface, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSignatureParameter, PackageReviewSynchronousInvocation, PackageReviewTypeIdentity,
    PackageReviewWriteFrameCompleteness,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::{MachineId, PackageKeyIdentity, ServiceId, TargetMachineCallable};
use terminal_check_tables::edges::Callee;
use terminal_psi::{CrashRouteGuard, MachineContract, TerminalModule};

impl PackageReviewInput<'_> {
    pub(super) fn build_callable(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Option<PackagePolicyCallable>, Vec<Diagnostic>> {
        let computed = self.compilation().build();
        let Some(selected) = computed.selected_build_machine_symbol else {
            return Ok(None);
        };
        let accepted = computed
            .accepted_build
            .as_ref()
            .ok_or_else(|| unavailable("the selected Build's accepted judgment"))?;
        if !accepted.is_invocation() {
            return Err(unavailable("Build's own invocation judgment"));
        }
        let entry = accepted
            .entry()
            .ok_or_else(|| unavailable("the accepted Build entry"))?;
        let mut origins = accepted
            .closure()
            .elaboration
            .machines
            .iter()
            .filter(|(machine, _)| *machine == entry);
        let (_, original) = origins
            .next()
            .ok_or_else(|| unavailable("Build's original declaration"))?;
        if origins.next().is_some() {
            return Err(unavailable("an unambiguous accepted Build declaration"));
        }
        let source = accepted.source();
        let retained = symbols::RetainedDeclaration::capture(&source.symbols, *original)
            .map_err(unavailable)?;
        let rejoined = retained
            .rejoin(
                &self.typed().symbols,
                self.typed().machines().iter().map(|machine| machine.symbol),
            )
            .map_err(unavailable)?;
        if rejoined != selected
            || computed.selected_machine_identity(self.typed())?
                != self
                    .compilation()
                    .build_admission()
                    .selected_build_machine_identity
        {
            return Err(unavailable("the exact selected Build/source association"));
        }
        let identity = self.nominal(rejoined)?;
        if identity.owner != PackageReviewNominalOwner::Package(package) {
            return Err(unavailable(
                "a selected Build owned by the reviewed package",
            ));
        }
        let declaration = source
            .machine(*original)
            .ok_or_else(|| unavailable("Build's admitted source machine"))?;
        if !declaration.supplies_checked_body()
            || !declaration.type_parameters.is_empty()
            || !declaration.conformance_bounds.is_empty()
            || !declaration.satisfies.is_empty()
            || !declaration.lifetime_parameters.is_empty()
            || declaration.target.is_some()
            || declaration.authored_target.is_some()
            || declaration.target_family_interface
        {
            return Err(unavailable(
                "this Build's complete generic or target contract",
            ));
        }
        let state = source
            .machine_states(declaration)
            .first()
            .ok_or_else(|| unavailable("Build's source entry signature"))?;
        let header = accepted
            .module()
            .machine(entry)
            .ok_or_else(|| unavailable("Build's checked body"))?;
        if source.state_parameters(state).len() != header.parameter_order.len()
            || !header.requires.is_empty()
            || !header.source_checked_requires.is_empty()
            || !header.ensures.is_empty()
            || !header.outcome_specific_ensures.is_empty()
        {
            return Err(unavailable(
                "Build's complete checked parameter/contract clauses",
            ));
        }
        let exact_sources =
            source::source_identity::toolchain_source_identities(source.symbols.source_files())
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let type_identity = |reference| {
            source
                .exact_owner_type_identity(
                    typed_trees::type_identity::ExactOwnerTypeIdentityRequest {
                        lifetime_scope: None,
                        type_reference: reference,
                        binders: &[],
                        substitutions: &[],
                        exact_toolchain_sources: &exact_sources,
                    },
                )
                .and_then(|identity| source.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .map(|canonical| PackageReviewTypeIdentity { canonical })
                .ok_or_else(|| unavailable("an exact Build signature type"))
        };
        let parameters = source
            .state_parameters(state)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewSignatureParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: type_identity(parameter.type_reference)?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let service_names = service_identities(accepted, &exact_sources)?;
        let ([reach, entries], [checked_may_suspend, checked_may_block]) =
            body_effects(accepted, entry)?;
        let realized_reach = reviewed_services(&service_names, &reach)?;
        let realized_entries = reviewed_services(&service_names, &entries)?
            .into_iter()
            .map(PackageReviewSynchronousInvocation::Service)
            .collect();
        let declared_service_reach = header
            .declared_service_reach
            .as_deref()
            .map(|row| reviewed_services(&service_names, row))
            .transpose()?;
        let declared_synchronous_invocations = header
            .declared_synchronous_entries
            .as_deref()
            .map(|row| {
                reviewed_services(&service_names, row).map(|row| {
                    row.into_iter()
                        .map(PackageReviewSynchronousInvocation::Service)
                        .collect()
                })
            })
            .transpose()?;
        let mutation = match &header.published_writes {
            None => PackagePolicyMutation {
                completeness: PackageReviewWriteFrameCompleteness::Opaque,
                paths: Vec::new(),
            },
            Some(paths) if paths.is_empty() => PackagePolicyMutation {
                completeness: PackageReviewWriteFrameCompleteness::Complete,
                paths: Vec::new(),
            },
            Some(_) => return Err(unavailable("Build's nonempty checked write frame")),
        };
        let declared_termination = declaration
            .termination_plan
            .interface
            .published()
            .map(|guarantee| match guarantee {
                language_semantics::TerminationGuarantee::NoGuarantee => {
                    Ok(PackagePolicyTermination::NoGuarantee)
                }
                language_semantics::TerminationGuarantee::Terminates { premises }
                    if premises.is_empty() =>
                {
                    Ok(PackagePolicyTermination::Terminates {
                        premises: Vec::new(),
                    })
                }
                _ => Err(unavailable("Build's progress premises")),
            })
            .transpose()?;
        Ok(Some(PackagePolicyCallable {
            target_context: PackagePolicyTargetContext::TargetIndependent,
            role: PackagePolicyCallableRole::Build,
            identity,
            supply: PackageReviewCallableSupply::CheckedBody,
            spelling: declaration.spelling,
            lifetime_parameter_count: 0,
            type_parameters: Vec::new(),
            conformance_bounds: Vec::new(),
            parameters,
            return_type: state
                .return_type
                .is_valid()
                .then(|| type_identity(state.return_type))
                .transpose()?,
            conformances: Vec::new(),
            operator_realizations: Vec::new(),
            requirement_realizations: Vec::new(),
            contracts: Vec::new(),
            declared_service_reach,
            service_reach_dependency: PackagePolicyServiceReachDependency {
                concrete: realized_reach.clone(),
                parameters: Vec::new(),
            },
            checked_service_reach: PackageReviewCheckedServiceReach::CheckedBody {
                realized: realized_reach.clone(),
                concrete: realized_reach,
            },
            unresolved_installation_reaches: Vec::new(),
            declared_synchronous_invocations,
            realized_synchronous_invocations: realized_entries,
            capability_flows: Vec::new(),
            reachable_capability_flows: Vec::new(),
            checked_may_suspend,
            checked_may_block,
            declared_may_suspend: header.declared_suspension,
            declared_may_block: header.declared_blocking,
            declared_termination,
            checked_termination: match header.termination {
                semantic_vocabulary::Termination::NoGuarantee => {
                    PackagePolicyTermination::NoGuarantee
                }
                semantic_vocabulary::Termination::Terminates => {
                    PackagePolicyTermination::Terminates {
                        premises: Vec::new(),
                    }
                }
            },
            checked_crash: crashes(&header.contract)?,
            mutation,
        }))
    }
}

/// A service keeps its exact source declaration through the already-retained
/// boundary correspondence. Equal-looking names cannot replace this edge, and
/// the shared application owner must reproduce the canonical service identity.
fn service_identities(
    accepted: &source_compiler::AcceptedTerminalSource,
    exact_sources: &[(source::SourceId, [u8; 32])],
) -> Result<Vec<(ServiceId, PackageReviewNominalIdentity)>, Vec<Diagnostic>> {
    use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
    let source = accepted.source();
    let module = accepted.module();
    let mut origins = Vec::with_capacity(module.boundary_machines.len());
    for boundary in &module.boundary_machines {
        let Some(service) = boundary.owning_service else {
            continue;
        };
        let original = *accepted
            .closure()
            .elaboration
            .boundaries
            .get(boundary.id)
            .ok_or_else(|| unavailable("a service's original boundary declaration"))?;
        if !source.symbols.symbols().nodes().is_valid(original) {
            return Err(unavailable(
                "a stale boundary declaration in Build's service inventory",
            ));
        }
        let owner = source.symbols.get(original).parent;
        origins.push((service, owner));
    }
    origins.sort_by_key(|(service, _)| *service);
    let mut identities = Vec::with_capacity(module.services.len());
    let mut previous = None;
    for (service, owner) in origins {
        if let Some((known_service, known_owner)) = previous
            && known_service == service
        {
            if known_owner != owner {
                return Err(unavailable(
                    "a service associated with two source declarations",
                ));
            }
            continue;
        }
        previous = Some((service, owner));
        let declaration = source
            .trait_definition_by_symbol(owner)
            .filter(|declaration| declaration.is_boundary)
            .ok_or_else(|| unavailable("a service's exact boundary trait declaration"))?;
        // This is the same declared boundary scope the reader publishes.
        // Missing actual arguments stay unavailable, never an empty fallback.
        let scope = AppliedTraitScope::new(
            source,
            declaration,
            TraitApplicationArguments {
                lexical_owner: declaration.symbol,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .ok_or_else(|| unavailable("a service's complete applied arguments"))?;
        let application = scope
            .portable_application_identity(source)
            .and_then(|identity| source.type_identity_text(identity))
            .ok_or_else(|| unavailable("a service's canonical applied identity"))?;
        let position = module
            .services
            .binary_search_by_key(&service, |row| row.id)
            .map_err(|_| unavailable("a service absent from the accepted declaration inventory"))?;
        let actual = module
            .identities
            .text(module.services[position].identity)
            .ok_or_else(|| unavailable("an accepted service's declaration identity"))?;
        if actual != application {
            return Err(unavailable(
                "a service/source application identity mismatch",
            ));
        }
        identities.push((service, nominal_identity(source, exact_sources, owner)?));
    }
    Ok(identities)
}

fn reviewed_services(
    identities: &[(ServiceId, PackageReviewNominalIdentity)],
    row: &[ServiceId],
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    let mut projected = row
        .iter()
        .map(|service| {
            let position = identities
                .binary_search_by_key(service, |(id, _)| *id)
                .map_err(|_| unavailable("a service without its original applied declaration"))?;
            Ok(identities[position].1.clone())
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn body_effects(
    accepted: &source_compiler::AcceptedTerminalSource,
    machine: MachineId,
) -> Result<([Vec<ServiceId>; 2], [bool; 2]), Vec<Diagnostic>> {
    let module = accepted.module();
    if !module
        .root_service_reach
        .installation_dependencies
        .is_empty()
        || !module.dynamic_dispatch.interfaces.is_empty()
        || !module.dynamic_dispatch.tables.is_empty()
    {
        return Err(unavailable(
            "Build's unresolved installation or dynamic effects",
        ));
    }
    let body = module
        .machine(machine)
        .ok_or_else(|| unavailable("Build's effect body"))?;
    let original = module
        .unoptimized_machine_behavior(machine)
        .ok_or_else(|| unavailable("Build's original checked behavior inventory"))?;
    if module
        .unoptimized_behavior
        .iter()
        .flat_map(|body| &body.operations)
        .any(|operation| {
            matches!(
                operation,
                terminal_psi::UnoptimizedBehaviorOperation::InstructionService { .. }
            )
        })
        || module
            .machines
            .iter()
            .flat_map(|body| body.operations())
            .any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::RegisterAccess { .. }
                )
            })
    {
        return Err(unavailable("Build's instruction authority"));
    }
    let tables = accepted
        .closure()
        .elaboration
        .tables
        .machines
        .get(machine)
        .ok_or_else(|| unavailable("Build's checked call inventory"))?;
    let mut rows = [Vec::new(), Vec::new()];
    let mut operational = [false; 2];
    let mut include = |callee, required| -> Result<(), Vec<Diagnostic>> {
        let ([reach, entries], [may_suspend, may_block]) = contribution(module, callee, required)?;
        rows[0].extend_from_slice(reach);
        rows[1].extend_from_slice(entries);
        operational[0] |= may_suspend;
        operational[1] |= may_block;
        Ok(())
    };
    for (_, call) in tables.invocations.iter() {
        let Callee::Declared(callee) = call.callee else {
            return Err(unavailable("Build's indirect call effects"));
        };
        include(callee, call.required_contract)?;
    }
    // Edge cleanup calls and recursive frame reuse retain their calls apart
    // from explicit invocation rows. Their public contributions still apply.
    for operation in &original.operations {
        if let terminal_psi::UnoptimizedBehaviorOperation::NominalCleanup { callee, .. } = operation
        {
            include((*callee).into(), None)?;
        }
    }
    for formation in &body.call_formations {
        if let Some(callee) = formation.tail_callee {
            include(callee.into(), None)?;
        }
    }
    for row in &mut rows {
        row.sort_unstable();
        row.dedup();
    }
    Ok((rows, operational))
}

fn contribution(
    module: &TerminalModule,
    callee: TargetMachineCallable,
    required: Option<semantic_vocabulary::ContractId>,
) -> Result<([&[ServiceId]; 2], [bool; 2]), Vec<Diagnostic>> {
    match callee {
        TargetMachineCallable::Machine(machine) => {
            let header = module
                .invocation_machine_contract(machine, required)
                .ok_or_else(|| unavailable("a Build call's exact public contract"))?;
            Ok((
                [
                    &header.published_service_ceiling,
                    &header.published_synchronous_entries,
                ],
                [header.published_may_suspend, header.published_may_block],
            ))
        }
        TargetMachineCallable::Boundary(boundary) => {
            if required.is_some() {
                return Err(unavailable("a selected boundary call's public association"));
            }
            let header = module
                .boundary_machine(boundary)
                .ok_or_else(|| unavailable("a Build call's boundary contract"))?;
            let declaration = header
                .declaration
                .as_ref()
                .ok_or_else(|| unavailable("Build's checked boundary operational contract"))?;
            // A boundary contributes its public rows, not its owning trait
            // name. This is the same rule as the accepted ceiling judgment.
            Ok((
                [
                    &header.published_service_ceiling,
                    &header.published_synchronous_entries,
                ],
                [
                    declaration.source.published_may_suspend,
                    declaration.source.published_may_block,
                ],
            ))
        }
    }
}

fn crashes(header: &MachineContract) -> Result<PackagePolicyCrash, Vec<Diagnostic>> {
    let cause = |cause| match cause {
        semantic_vocabulary::CrashCause::Trap => PackageReviewCrashCause::Trap,
        semantic_vocabulary::CrashCause::Abort => PackageReviewCrashCause::Abort,
    };
    let Some(declared) = &header.declared_crash_routes else {
        let mut causes: Vec<_> = header
            .crash_routes
            .iter()
            .map(|route| cause(route.cause))
            .collect();
        causes.sort();
        causes.dedup();
        return Ok(PackagePolicyCrash {
            interface: PackageReviewCrashInterface::InternalInferred,
            published: Vec::new(),
            structural_runtime_requirements: None,
            inferred: PackagePolicyInferredCrash::Complete { causes },
        });
    };
    let mut published = Vec::new();
    for route in declared {
        if route.alternatives.as_slice() != [CrashRouteGuard::Truth] {
            return Err(unavailable("Build's conditional published crash guards"));
        }
        published.push(PackagePolicyCrashRoute {
            cause: cause(route.cause),
            alternative_guards: vec![PackagePolicyCrashGuard::Truth],
        });
    }
    published.sort_by_key(|route| route.cause);
    Ok(PackagePolicyCrash {
        interface: PackageReviewCrashInterface::PublishedCeiling,
        published,
        structural_runtime_requirements: None,
        inferred: PackagePolicyInferredCrash::Unknown,
    })
}

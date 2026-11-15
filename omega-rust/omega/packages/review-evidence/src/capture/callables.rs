//! Review keeps the contracts of the declarations the package judgment accepted.
//! Build retains its separate invocation judgment; public functions and
//! requirements borrow Product's accepted module. Neither owner may infer a
//! body for a declaration supplied elsewhere.

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
        self.callable(
            accepted,
            *original,
            identity,
            PackagePolicyCallableRole::Build,
            PackageReviewCallableSupply::CheckedBody,
            Some(entry),
        )
        .map(Some)
    }

    pub(super) fn callables(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<PackagePolicyCallable>, Vec<Diagnostic>> {
        let mut rows: Vec<_> = self.build_callable(package)?.into_iter().collect();
        let accepted = self.compilation().terminal_publication().accepted()?;
        for declaration in self.typed().machines() {
            if !self.owns(declaration.symbol, package)?
                || (!declaration.is_public && declaration.supplies_checked_body())
            {
                continue;
            }
            let (role, supply) = match declaration.supply_mode {
                language_semantics::MachineSupplyMode::CheckedBody => (
                    PackagePolicyCallableRole::Public,
                    PackageReviewCallableSupply::CheckedBody,
                ),
                language_semantics::MachineSupplyMode::Requirement => (
                    PackagePolicyCallableRole::Public,
                    PackageReviewCallableSupply::Requirement,
                ),
                language_semantics::MachineSupplyMode::TopLevelRequirement => (
                    PackagePolicyCallableRole::Boundary,
                    PackageReviewCallableSupply::TopLevelRequirement,
                ),
                _ => return Err(unavailable("this callable's supply and authority contract")),
            };
            let body = if declaration.supplies_checked_body() {
                let mut matching = accepted
                    .closure()
                    .elaboration
                    .machines
                    .iter()
                    .filter(|(_, original)| **original == declaration.symbol);
                let (entry, _) = matching
                    .next()
                    .ok_or_else(|| unavailable("the public callable's accepted body"))?;
                if matching.next().is_some() {
                    return Err(unavailable("an unambiguous public callable application"));
                }
                Some(entry)
            } else {
                None
            };
            rows.push(self.callable(
                accepted,
                declaration.symbol,
                self.nominal(declaration.symbol)?,
                role,
                supply,
                body,
            )?);
        }
        rows.sort_by(|a, b| a.identity.cmp(&b.identity));
        Ok(rows)
    }

    fn callable(
        &self,
        accepted: &source_compiler::AcceptedTerminalSource,
        original: symbols::SymbolHandle,
        identity: PackageReviewNominalIdentity,
        role: PackagePolicyCallableRole,
        supply: PackageReviewCallableSupply,
        body: Option<MachineId>,
    ) -> Result<PackagePolicyCallable, Vec<Diagnostic>> {
        let source = accepted.source();
        let declaration = source
            .machine(original)
            .ok_or_else(|| unavailable("the callable's admitted source machine"))?;
        if !declaration.type_parameters.is_empty()
            || !declaration.conformance_bounds.is_empty()
            || !declaration.satisfies.is_empty()
            || !declaration.lifetime_parameters.is_empty()
            || declaration.target.is_some()
            || declaration.authored_target.is_some()
            || declaration.target_family_interface
        {
            return Err(unavailable(
                "the callable's complete generic or target contract",
            ));
        }
        let state = source
            .machine_states(declaration)
            .first()
            .ok_or_else(|| unavailable("the callable's source entry signature"))?;
        let header = match body {
            Some(entry) => {
                &accepted
                    .module()
                    .machine(entry)
                    .ok_or_else(|| unavailable("the accepted callable body"))?
                    .contract
            }
            None => {
                let exact = source
                    .exact_owner_nominal_identity(
                        original,
                        self.compilation().sources().exact_toolchain_sources(),
                    )
                    .and_then(|identity| source.type_identity_text(identity))
                    .ok_or_else(|| unavailable("a bodyless callable's exact identity"))?;
                let mut matching =
                    accepted
                        .module()
                        .declaration_contracts
                        .iter()
                        .filter(|header| {
                            header.declaration.as_ref().is_some_and(|declaration| {
                                accepted
                                    .module()
                                    .identities
                                    .text(declaration.machine.identity())
                                    == Some(exact.as_str())
                            })
                        });
                let header = matching
                    .next()
                    .ok_or_else(|| unavailable("the accepted bodyless contract"))?;
                if matching.next().is_some() {
                    return Err(unavailable("an unambiguous bodyless contract application"));
                }
                header
            }
        };
        if source.state_parameters(state).len() != header.parameter_order.len() {
            return Err(unavailable(
                "the callable's complete checked parameter/contract clauses",
            ));
        }
        let contracts = if role == PackagePolicyCallableRole::Build {
            if !header.requires.is_empty()
                || !header.source_checked_requires.is_empty()
                || !header.ensures.is_empty()
                || !header.outcome_specific_ensures.is_empty()
            {
                return Err(unavailable("Build's complete checked clauses"));
            }
            Vec::new()
        } else {
            self.callable_contracts(declaration, state)?
        };
        let exact_sources =
            source::source_identity::toolchain_source_identities(source.symbols.source_files())
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let type_identity = |reference| {
            if role != PackagePolicyCallableRole::Build {
                return self.type_identity(reference);
            }
            source
                .exact_owner_type_identity(
                    typed_trees::type_identity::ExactOwnerTypeIdentityRequest {
                        type_reference: reference,
                        binders: &[],
                        substitutions: &[],
                        exact_toolchain_sources: &exact_sources,
                    },
                )
                .and_then(|identity| source.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .map(|canonical| PackageReviewTypeIdentity { canonical })
                .ok_or_else(|| unavailable("an exact callable signature type"))
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
        let ([reach, entries], [checked_may_suspend, checked_may_block]) = match body {
            Some(entry) => body_effects(accepted, entry)?,
            None => ([Vec::new(), Vec::new()], [false; 2]),
        };
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
        let mutation = mutation(accepted.module(), header)?;
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
                _ => Err(unavailable("the callable's progress premises")),
            })
            .transpose()?;
        Ok(PackagePolicyCallable {
            target_context: PackagePolicyTargetContext::TargetIndependent,
            role,
            identity,
            supply,
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
            contracts,
            declared_service_reach,
            service_reach_dependency: PackagePolicyServiceReachDependency {
                concrete: reviewed_services(&service_names, &header.published_service_ceiling)?,
                parameters: Vec::new(),
            },
            checked_service_reach: if body.is_some() {
                PackageReviewCheckedServiceReach::CheckedBody {
                    realized: realized_reach.clone(),
                    concrete: realized_reach,
                }
            } else {
                PackageReviewCheckedServiceReach::NoCheckedBody
            },
            unresolved_installation_reaches: Vec::new(),
            declared_synchronous_invocations: if role == PackagePolicyCallableRole::Build {
                declared_synchronous_invocations
            } else {
                Some(
                    reviewed_services(&service_names, &header.published_synchronous_entries)?
                        .into_iter()
                        .map(PackageReviewSynchronousInvocation::Service)
                        .collect(),
                )
            },
            realized_synchronous_invocations: realized_entries,
            capability_flows: Vec::new(),
            reachable_capability_flows: Vec::new(),
            checked_may_suspend,
            checked_may_block,
            declared_may_suspend: if role == PackagePolicyCallableRole::Build {
                header.declared_suspension
            } else {
                Some(header.published_may_suspend)
            },
            declared_may_block: if role == PackagePolicyCallableRole::Build {
                header.declared_blocking
            } else {
                Some(header.published_may_block)
            },
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
            checked_crash: crashes(header)?,
            mutation,
        })
    }
    fn callable_contracts(
        &self,
        declaration: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Result<Vec<crate::record::PackageReviewCallableContract>, Vec<Diagnostic>> {
        use crate::record::{
            PackageReviewCallableContract, PackageReviewContractFact, PackageReviewContractKind,
            PackageReviewResultCaseIdentity,
        };
        use typed_trees::signature::SignatureContractKind;
        let typed = self.typed();
        let mut rows = Vec::new();
        for clause in typed
            .machine_contracts(declaration)
            .iter()
            .chain(typed.state_contracts(state))
        {
            let (kind, result_case) = match clause.kind {
                SignatureContractKind::Requires => (PackageReviewContractKind::Requires, None),
                SignatureContractKind::Ensures => (PackageReviewContractKind::Ensures, None),
                SignatureContractKind::EnsuresForResultCase {
                    result_data,
                    result_case,
                } => (
                    PackageReviewContractKind::Ensures,
                    Some(PackageReviewResultCaseIdentity {
                        result_data: self.nominal(result_data)?,
                        result_case: self.nominal(result_case)?,
                    }),
                ),
                SignatureContractKind::Crashes { .. } => continue,
            };
            let scope = super::declarations::ContractScope {
                owner: state.symbol,
                subject: None,
                parameters: typed.state_parameters(state),
                result: (kind == PackageReviewContractKind::Ensures).then_some(state.return_type),
            };
            let facts = typed
                .proof_facts
                .span(clause.facts)
                .ok_or_else(|| unavailable("a callable clause's retained facts"))?;
            for fact in facts {
                let fact = match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        PackageReviewContractFact::Expression(self.scoped_expression(
                            *expression,
                            scope,
                            0,
                        )?)
                    }
                    _ => {
                        return Err(unavailable(
                            "this callable clause's membership or proposition binding",
                        ));
                    }
                };
                rows.push(PackageReviewCallableContract {
                    kind,
                    result_case: result_case.clone(),
                    binding: clause.binding.as_ref().map(|name| name.as_str().to_owned()),
                    evidence_lane_position: None,
                    fact,
                });
            }
        }
        rows.sort();
        rows.dedup();
        Ok(rows)
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

/// Write frames use module-local field numbers. Review retains the formal's
/// authored position and the exact type/field identities instead, so another
/// compilation's numbering cannot change which sibling storage may be written.
fn mutation(
    module: &TerminalModule,
    header: &MachineContract,
) -> Result<PackagePolicyMutation, Vec<Diagnostic>> {
    let Some(written) = &header.published_writes else {
        return Ok(PackagePolicyMutation {
            completeness: PackageReviewWriteFrameCompleteness::Opaque,
            paths: Vec::new(),
        });
    };
    header.validate_parameter_order().map_err(unavailable)?;
    let mut paths = Vec::with_capacity(written.len());
    for (ordinal, fields) in written {
        let position = usize::try_from(*ordinal)
            .map_err(|_| unavailable("a write frame's structural formal ordinal"))?;
        let formal = header
            .structural_parameters
            .get(position)
            .ok_or_else(|| unavailable("a write frame's structural formal"))?;
        let authored_position = header
            .parameter_order
            .iter()
            .enumerate()
            .filter(|(_, kind)| **kind == terminal_psi::BoundaryParameterKind::Structural)
            .nth(position)
            .map(|(position, _)| position)
            .ok_or_else(|| unavailable("a write frame's authored parameter position"))?;
        let mut path = String::new();
        crate::record::write_framed_identity(
            &mut path,
            "parameter",
            [authored_position.to_string().as_str()],
        )
        .map_err(|_| unavailable("a write frame's parameter identity"))?;
        let mut tip = Some(formal.structural_type);
        for (position, field) in fields.iter().enumerate() {
            let owner = terminal_psi::unique_structural_declaration(
                &module.structural_types,
                tip.ok_or_else(|| unavailable("a write frame traversing a scalar leaf"))?,
                &[],
            )
            .ok_or_else(|| unavailable("a write frame's unique field owner"))?;
            let candidates = owner
                .shape
                .direct_fields()
                .ok_or_else(|| unavailable("a write frame's declared record fields"))?;
            let mut matching = candidates
                .iter()
                .filter(|candidate| candidate.id == *field && !candidate.relevance.is_erased());
            let member = matching
                .next()
                .ok_or_else(|| unavailable("a write frame's live declared field"))?;
            if matching.next().is_some() {
                return Err(unavailable("an ambiguous write-frame field"));
            }
            let owner_identity = module
                .identities
                .text(owner.identity)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| unavailable("a write frame's owner identity"))?;
            let field_identity = module
                .identities
                .text(member.identity)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| unavailable("a write frame's field identity"))?;
            crate::record::write_framed_identity(
                &mut path,
                "field",
                [owner_identity, field_identity],
            )
            .map_err(|_| unavailable("a write frame's field identity"))?;
            if position + 1 < fields.len() {
                tip = member.field_type.as_structural().or(member.inline_carrier);
            }
        }
        paths.push(path);
    }
    paths.sort();
    paths.dedup();
    Ok(PackagePolicyMutation {
        completeness: PackageReviewWriteFrameCompleteness::Complete,
        paths,
    })
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

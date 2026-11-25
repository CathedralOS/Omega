//! Keeps, for review, the contracts of the declarations the package judgment
//! accepted. Build keeps its own invocation judgment; public functions and
//! requirements borrow Product's accepted module. Neither may infer a body for
//! a declaration supplied elsewhere.

use super::declarations::unavailable;
use super::input::PolicySource;
use super::services::{self, ReviewedService, reviewed_service, reviewed_services};
use diagnostics::Diagnostic;
use review_evidence::record::PackageReviewServiceIdentity;
use review_evidence::record::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyCrash, PackagePolicyCrashGuard,
    PackagePolicyCrashRoute, PackagePolicyInferredCrash, PackagePolicyMutation,
    PackagePolicyServiceReachDependency, PackagePolicyTargetContext, PackagePolicyTermination,
    PackageReviewCallableSupply, PackageReviewCheckedServiceReach, PackageReviewCrashCause,
    PackageReviewCrashInterface, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSemanticDependencyExposure, PackageReviewSignatureParameter,
    PackageReviewSynchronousInvocation, PackageReviewTypeIdentity, PackageReviewWriteFrameCompleteness,
};
use semantic_vocabulary::{MachineId, PackageKeyIdentity, ServiceId, TargetMachineCallable};
use terminal_psi::{CrashRouteGuard, MachineContract, TerminalModule};

pub(super) fn exposes_machine_interface(
    machine: &source_compiler::representations::source_state::machine::Machine,
) -> bool {
    use source_compiler::representations::declaration_selection::AuthoredDeclarationSelectionExposure;
    machine.interface_exposure == AuthoredDeclarationSelectionExposure::PublicInterface
}

struct CallableInventory<'a> {
    source: &'a source_compiler::representations::source_state::TypedTrees,
    module: &'a TerminalModule,
    exact_sources: &'a [(source_text::SourceId, [u8; 32])],
    service_names: &'a [ReviewedService],
}

impl PolicySource<'_> {
    pub(super) fn build_callable(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Option<PackagePolicyCallable>, Vec<Diagnostic>> {
        let Some(selected) = &self.build().executed_entry else {
            return Ok(None);
        };
        let symbol = selected.rejoin(self.typed())?;
        let code = self
            .build()
            .accepted_builds
            .last()
            .ok_or_else(|| unavailable("the selected Build's accepted code"))?;
        let module = code.view().module();
        let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
            return Err(unavailable("the selected Build's machine entry"));
        };
        let declaration = module
            .machine(entry)
            .and_then(|machine| machine.contract.declaration.as_ref())
            .ok_or_else(|| unavailable("the selected Build's public callable contract"))?;
        if module.identities.text(declaration.callable) != Some(selected.callable_identity.as_str())
        {
            return Err(unavailable("the exact selected Build/source association"));
        }
        let identity = self.nominal(symbol)?;
        if identity.owner != PackageReviewNominalOwner::Package(package) {
            return Err(unavailable(
                "a selected Build owned by the reviewed package",
            ));
        }
        validate_body_effect_inventory(module)?;
        let names = services::identities(
            self.typed(),
            module,
            self.sources().exact_toolchain_sources(),
            source_provenance::DependencyScope::Build,
        )?;
        self.callable(
            &CallableInventory {
                source: self.typed(),
                module,
                exact_sources: self.sources().exact_toolchain_sources(),
                service_names: &names,
            },
            symbol,
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
        service_names: &[ReviewedService],
    ) -> Result<Vec<PackagePolicyCallable>, Vec<Diagnostic>> {
        let mut rows: Vec<_> = self.build_callable(package)?.into_iter().collect();
        let accepted = self.accepted();
        let exact_sources = self.sources().exact_toolchain_sources();
        let mut inventory = None;
        let mut effects_validated = false;
        let mut bodies: Vec<_> = accepted
            .closure()
            .elaboration()
            .machines
            .iter()
            .map(|(entry, original)| (*original, entry))
            .collect();
        let key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
        bodies.sort_unstable_by_key(|(original, _)| key(*original));
        for declaration in self.typed().machines() {
            if !self.owns(declaration.symbol, package)?
                || (!exposes_machine_interface(declaration) && declaration.supplies_checked_body())
            {
                continue;
            }
            let (role, supply) = match declaration.supply_mode {
                source_declarations::MachineSupplyMode::CheckedBody => (
                    PackagePolicyCallableRole::Public,
                    PackageReviewCallableSupply::CheckedBody,
                ),
                source_declarations::MachineSupplyMode::Requirement => (
                    PackagePolicyCallableRole::Public,
                    PackageReviewCallableSupply::Requirement,
                ),
                source_declarations::MachineSupplyMode::TopLevelRequirement => (
                    PackagePolicyCallableRole::Boundary,
                    PackageReviewCallableSupply::TopLevelRequirement,
                ),
                _ => return Err(unavailable("this callable's supply and authority contract")),
            };
            let identity = self.nominal(declaration.symbol)?;
            let body = if declaration.supplies_checked_body() {
                let first = bodies
                    .partition_point(|(original, _)| key(*original) < key(declaration.symbol));
                let end = bodies
                    .partition_point(|(original, _)| key(*original) <= key(declaration.symbol));
                let [(original, entry)] = &bodies[first..end] else {
                    let mut diagnostic = Diagnostic::error(format!(
                        "package review expected one checked body for public callable `{}`, found {}",
                        identity.path,
                        end - first,
                    ));
                    diagnostic.source_span = self
                        .typed()
                        .symbols
                        .symbol_provenance_source_span(declaration.symbol);
                    return Err(vec![diagnostic]);
                };
                debug_assert_eq!(*original, declaration.symbol);
                Some(*entry)
            } else {
                None
            };
            if inventory.is_none() {
                inventory = Some(CallableInventory {
                    module: accepted.module(),
                    source: accepted.source(),
                    exact_sources,
                    service_names,
                });
            }
            if body.is_some() && !effects_validated {
                validate_body_effect_inventory(accepted.module())?;
                effects_validated = true;
            }
            rows.push(self.callable(
                inventory.as_ref().expect("prepared callable inventory"),
                declaration.symbol,
                identity,
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
        inventory: &CallableInventory<'_>,
        original: symbols::SymbolHandle,
        identity: PackageReviewNominalIdentity,
        role: PackagePolicyCallableRole,
        supply: PackageReviewCallableSupply,
        body: Option<MachineId>,
    ) -> Result<PackagePolicyCallable, Vec<Diagnostic>> {
        let module = inventory.module;
        let exact_sources = inventory.exact_sources;
        let service_names = inventory.service_names;
        let source = inventory.source;
        let declaration = source
            .machine(original)
            .ok_or_else(|| unavailable("the callable's admitted source machine"))?;
        if !declaration.satisfies.is_empty() || declaration.target_family_interface {
            return Err(unavailable(
                "the callable's conformance realizations or target family contract",
            ));
        }
        // Target selection can clear the active target marker; the authored
        // declaration retains the scope which must enter the public policy.
        let target_context = match (&declaration.authored_target, &declaration.target) {
            (Some(authored), Some(active)) if authored.as_str() != active.as_str() => {
                return Err(unavailable(
                    "inconsistent active and authored target scopes",
                ));
            }
            (Some(authored), _) => PackagePolicyTargetContext::TargetMember {
                target: authored.as_str().to_owned(),
            },
            (None, None) => PackagePolicyTargetContext::TargetIndependent,
            (None, Some(_)) => return Err(unavailable("the authored target family association")),
        };
        let state = source
            .machine_states(declaration)
            .first()
            .ok_or_else(|| unavailable("the callable's source entry signature"))?;
        let body = body
            .map(|entry| {
                module
                    .machine(entry)
                    .ok_or_else(|| unavailable("the accepted callable body"))
            })
            .transpose()?;
        let header = match body {
            Some(body) => &body.contract,
            None => {
                let exact = source
                    .exact_owner_nominal_identity(
                        original,
                        self.sources().exact_toolchain_sources(),
                    )
                    .and_then(|identity| source.type_identity_text(identity))
                    .ok_or_else(|| unavailable("a bodyless callable's exact identity"))?;
                let mut matching = module
                    .execution
                    .declaration_contracts
                    .iter()
                    .filter(|header| {
                        header.declaration.as_ref().is_some_and(|declaration| {
                            module.identities.text(declaration.machine.identity())
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
        let source_parameters = source.state_parameters(state);
        let (parameter_types, result_type, elided_lifetimes) = if role
            == PackagePolicyCallableRole::Build
        {
            let type_identity = |reference| {
                source
                    .exact_owner_type_identity(
                        source_compiler::representations::source_state::type_identity::ExactOwnerTypeIdentityRequest {
                            lifetime_scope: None,
                            type_reference: reference,
                            binders: &[],
                            substitutions: &[],
                            exact_toolchain_sources: exact_sources,
                        },
                    )
                    .and_then(|identity| source.type_identity_text(identity))
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| unavailable("an exact callable signature type"))
            };
            let parameters = source_parameters
                .iter()
                .map(|parameter| type_identity(parameter.type_reference))
                .collect::<Result<Vec<_>, _>>()?;
            let result = state
                .return_type
                .is_valid()
                .then(|| type_identity(state.return_type))
                .transpose()?;
            (parameters, result, 0)
        } else {
            for reference in source_parameters
                .iter()
                .map(|parameter| parameter.type_reference)
                .chain(state.return_type.is_valid().then_some(state.return_type))
            {
                self.require_type_dependencies(
                    original,
                    reference,
                    PackageReviewSemanticDependencyExposure::PublicInterface,
                )?;
            }
            let (parameters, result, elided) = source
                .exact_machine_definition_types(declaration, exact_sources)
                .ok_or_else(|| unavailable("the callable's complete declared signature"))?;
            (
                parameters,
                state.return_type.is_valid().then_some(result),
                elided,
            )
        };
        let parameters = source_parameters
            .iter()
            .zip(parameter_types)
            .map(|(parameter, canonical)| PackageReviewSignatureParameter {
                name: parameter.name.as_str().to_owned(),
                type_identity: PackageReviewTypeIdentity { canonical },
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
            })
            .collect();
        let ([reach, entries], [checked_may_suspend, checked_may_block]) = match body {
            Some(body) => {
                let original = module
                    .unoptimized_machine_behavior(body.id)
                    .ok_or_else(|| {
                        unavailable("the callable's original checked behavior inventory")
                    })?;
                body_effects(module, body, original)?
            }
            None => ([Vec::new(), Vec::new()], [false; 2]),
        };
        let realized_reach = reviewed_services(service_names, &reach)?;
        let realized_entries = reviewed_services(service_names, &entries)?
            .into_iter()
            .map(PackageReviewSynchronousInvocation::Service)
            .collect();
        let declared_service_reach = header
            .declared_service_reach
            .as_deref()
            .map(|row| reviewed_services(service_names, row))
            .transpose()?;
        let declared_synchronous_invocations = header
            .declared_synchronous_entries
            .as_deref()
            .map(|row| {
                reviewed_services(service_names, row).map(|row| {
                    row.into_iter()
                        .map(PackageReviewSynchronousInvocation::Service)
                        .collect()
                })
            })
            .transpose()?;
        let mutation = mutation(module, header)?;
        let declared_termination = declaration
            .termination_plan
            .interface
            .published()
            .map(|guarantee| match guarantee {
                source_declarations::TerminationGuarantee::NoGuarantee => {
                    Ok(PackagePolicyTermination::NoGuarantee)
                }
                source_declarations::TerminationGuarantee::Terminates { premises }
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
            target_context,
            role,
            identity,
            supply,
            spelling: declaration.spelling,
            lifetime_parameter_count: declaration
                .lifetime_parameters
                .len()
                .checked_add(elided_lifetimes)
                .ok_or_else(|| unavailable("a representable callable lifetime telescope"))?,
            type_parameters: self.type_parameters(original)?,
            conformance_bounds: self.conformance_bounds(
                original,
                &declaration.lifetime_parameters,
                &declaration.conformance_bounds,
            )?,
            parameters,
            return_type: result_type.map(|canonical| PackageReviewTypeIdentity { canonical }),
            conformances: Vec::new(),
            operator_realizations: Vec::new(),
            requirement_realizations: Vec::new(),
            contracts,
            declared_service_reach,
            service_reach_dependency: PackagePolicyServiceReachDependency {
                concrete: if body.is_some() {
                    realized_reach.clone()
                } else {
                    reviewed_services(service_names, &header.published_service_ceiling)?
                },
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
                    reviewed_services(service_names, &header.published_synchronous_entries)?
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
        declaration: &source_compiler::representations::source_state::machine::Machine,
        state: &source_compiler::representations::source_state::state::State,
    ) -> Result<Vec<review_evidence::record::PackageReviewCallableContract>, Vec<Diagnostic>> {
        let typed = self.typed();
        self.signature_contracts(
            typed
                .machine_contracts(declaration)
                .iter()
                .chain(typed.state_contracts(state)),
            super::declarations::ContractScope {
                owner: state.symbol,
                subject: None,
                parameters: typed.state_parameters(state),
                statics: typed.machine_type_parameters(declaration),
                result: None,
            },
            state.return_type,
        )
    }

    pub(super) fn signature_contracts<'clause>(
        &self,
        clauses: impl IntoIterator<
            Item = &'clause source_compiler::representations::source_state::signature::SignatureContract,
        >,
        scope: super::declarations::ContractScope<'_>,
        return_type: source_compiler::representations::source_state::types::TypeReferenceHandle,
    ) -> Result<Vec<review_evidence::record::PackageReviewCallableContract>, Vec<Diagnostic>> {
        use review_evidence::record::{
            PackageReviewCallableContract, PackageReviewContractKind,
            PackageReviewResultCaseIdentity,
        };
        use source_compiler::representations::source_state::signature::SignatureContractKind;
        let typed = self.typed();
        let mut rows = Vec::new();
        for clause in clauses {
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
                result: (kind == PackageReviewContractKind::Ensures).then_some(return_type),
                ..scope
            };
            let facts = typed
                .proof_facts
                .span(clause.facts)
                .ok_or_else(|| unavailable("a callable clause's retained facts"))?;
            for fact in facts {
                let fact = self.review_fact(fact, scope)?;
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
    pub(super) fn review_fact(
        &self,
        fact: &source_compiler::representations::source_state::domain::ProofFact,
        scope: super::declarations::ContractScope<'_>,
    ) -> Result<review_evidence::record::PackageReviewContractFact, Vec<Diagnostic>> {
        use review_evidence::record::PackageReviewContractFact;
        Ok(match fact {
            source_compiler::representations::source_state::domain::ProofFact::Expression(
                expression,
            ) => PackageReviewContractFact::Expression(self.scoped_expression(
                *expression,
                scope,
                0,
            )?),
            source_compiler::representations::source_state::domain::ProofFact::Membership(
                membership,
            ) => {
                let application = self
                    .typed()
                    .runtime_membership_domain_identity(
                        membership,
                        self.sources().exact_toolchain_sources(),
                        &[],
                        &[],
                    )
                    .map_err(unavailable)?;
                PackageReviewContractFact::Membership {
                    value: self.scoped_expression(membership.value, scope, 0)?,
                    domain: self.nominal(membership.domain_symbol)?,
                    application,
                }
            }
            _ => {
                return Err(unavailable("this callable clause's proposition binding"));
            }
        })
    }
}

/// Join the portable identities in the accepted module to the exact source
/// declarations. Memberless service labels may appear in reach even though
/// there is no boundary method whose correspondence could name their owner.
pub(super) fn reviewed_invocations(
    identities: &[ReviewedService],
    envelope: &terminal_psi::TerminalCallableEnvelope,
) -> Result<Vec<PackageReviewSynchronousInvocation>, Vec<Diagnostic>> {
    use terminal_psi::TerminalCallableInvocation;
    let mut invocations = envelope
        .invocations
        .iter()
        .map(|entry| match entry {
            TerminalCallableInvocation::Parameter(position) => {
                Ok(PackageReviewSynchronousInvocation::Parameter(*position))
            }
            TerminalCallableInvocation::Service(service) => reviewed_service(identities, *service)
                .map(PackageReviewSynchronousInvocation::Service),
        })
        .collect::<Result<Vec<_>, _>>()?;
    invocations.sort_unstable();
    Ok(invocations)
}

/// Exact service declarations named by the published contract or its checked body.
pub(super) fn referenced_services(
    callable: &PackagePolicyCallable,
) -> impl Iterator<Item = &PackageReviewServiceIdentity> {
    let invocations = callable
        .declared_synchronous_invocations
        .iter()
        .flatten()
        .chain(&callable.realized_synchronous_invocations);
    let checked = &callable.checked_service_reach;
    callable
        .declared_service_reach
        .iter()
        .flatten()
        .chain(checked.realized().unwrap_or_default())
        .chain(checked.concrete().unwrap_or_default())
        .chain(invocations.filter_map(PackageReviewSynchronousInvocation::service))
}

// These authority inventories belong to the accepted module, so inspect them
// once before projecting its checked bodies rather than once per public helper.
pub(super) fn validate_body_effect_inventory(
    module: &TerminalModule,
) -> Result<(), Vec<Diagnostic>> {
    if !module.execution.dynamic_dispatch.interfaces.is_empty()
        || !module.execution.dynamic_dispatch.tables.is_empty()
    {
        return Err(unavailable("the accepted module's dynamic effects"));
    }
    if module
        .execution
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
        return Err(unavailable("the accepted module's instruction authority"));
    }
    Ok(())
}

pub(super) fn body_effects(
    module: &TerminalModule,
    body: &terminal_psi::TerminalMachine,
    original: &terminal_psi::UnoptimizedMachineBehavior,
) -> Result<([Vec<ServiceId>; 2], [bool; 2]), Vec<Diagnostic>> {
    if body.callable_invocations().next().is_some() {
        return Err(unavailable("the callable’s indirect call effects"));
    }
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
    for block in &body.blocks {
        for call in block
            .operations
            .iter()
            .filter_map(terminal_psi::Operation::declared_invocation)
        {
            include(call.callee, call.required_contract)?;
        }
        if let Some((_, call)) = block.terminator.declared_invocation() {
            include(call.callee, call.required_contract)?;
        }
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
                .ok_or_else(|| unavailable("a call's exact public contract"))?;
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
                .ok_or_else(|| unavailable("a call's boundary contract"))?;
            let declaration = header
                .declaration
                .as_ref()
                .ok_or_else(|| unavailable("the checked boundary operational contract"))?;
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
        review_evidence::record::write_framed_identity(
            &mut path,
            "parameter",
            [authored_position.to_string().as_str()],
        )
        .map_err(|_| unavailable("a write frame's parameter identity"))?;
        let mut tip = Some(formal.structural_type);
        for (position, field) in fields.iter().enumerate() {
            let owner = terminal_psi::unique_structural_declaration(
                &module.types.structural_types,
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
            review_evidence::record::write_framed_identity(
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
    let Some(declared) = &header.declared_crash_routes else {
        let mut causes: Vec<_> = header
            .crash_routes
            .iter()
            .map(|route| reviewed_crash_cause(route.cause))
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
    Ok(PackagePolicyCrash {
        interface: PackageReviewCrashInterface::PublishedCeiling,
        published: published_crash_routes(declared)?,
        structural_runtime_requirements: None,
        inferred: PackagePolicyInferredCrash::Unknown,
    })
}

fn reviewed_crash_cause(cause: semantic_vocabulary::CrashCause) -> PackageReviewCrashCause {
    match cause {
        semantic_vocabulary::CrashCause::Trap => PackageReviewCrashCause::Trap,
        semantic_vocabulary::CrashCause::Abort => PackageReviewCrashCause::Abort,
    }
}

pub(super) fn published_crash_routes(
    declared: &[terminal_psi::CrashRouteBucket],
) -> Result<Vec<PackagePolicyCrashRoute>, Vec<Diagnostic>> {
    let mut published = Vec::new();
    for route in declared {
        if route.alternatives.as_slice() != [CrashRouteGuard::Truth] {
            return Err(unavailable(
                "the callable's conditional published crash guards",
            ));
        }
        published.push(PackagePolicyCrashRoute {
            cause: reviewed_crash_cause(route.cause),
            alternative_guards: vec![PackagePolicyCrashGuard::Truth],
        });
    }
    published.sort_by_key(|route| route.cause);
    Ok(published)
}

#[cfg(test)]
mod exposure_tests {
    #[test]
    fn exported_boundary_supply_exposes_its_contract_without_publishing_its_name() {
        use source_compiler::representations::{
            declaration_selection::AuthoredDeclarationSelectionExposure as Exposure,
            source_state::machine::Machine,
        };
        let mut machine = Machine {
            is_public: false,
            interface_exposure: Exposure::PublicInterface,
            supply_mode: source_declarations::MachineSupplyMode::Boundary,
            ..Default::default()
        };
        assert!(super::exposes_machine_interface(&machine));
        machine.interface_exposure = Exposure::PrivateImplementation;
        assert!(!super::exposes_machine_interface(&machine));
    }
}

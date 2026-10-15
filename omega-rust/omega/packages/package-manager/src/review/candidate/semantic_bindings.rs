use super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet};
use crate::declarations::{DependencyPurpose, PackageKey};
use crate::lock::{PackageCheckedContext, PackageOccurrenceRoster};
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use compiler::CheckedCompilation;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ServiceMethod, ServiceSchema};
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use std::collections::{BTreeMap, BTreeSet};
use symbols::SymbolHandle;
use terminal_psi_to_abstract_operations::provider_admission;

/// One compiler-issued semantic-binding proposal together with the exact
/// checked service schema from which a consumer may author permission rows.
///
/// The schema and any hosted permission rows are review proposals, not grants.
/// Permissions bind exact requirement identities to the compiler's hosted-leaf
/// classifications; consumer scoping removes them from non-root/build inputs.
/// The resulting binding still requires complete checked recompilation and
/// the project's explicit policy decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticBindingReviewCandidate {
    binding: AcceptedSemanticBinding,
    service_schema: ServiceSchema,
}

impl SemanticBindingReviewCandidate {
    pub(super) fn new(binding: AcceptedSemanticBinding, service_schema: ServiceSchema) -> Self {
        debug_assert_eq!(
            binding.normalized_schema_digest(),
            package_compilation::accepted_service_schema_digest(binding.role(), &service_schema,),
        );
        Self {
            binding,
            service_schema,
        }
    }

    pub const fn binding(&self) -> &AcceptedSemanticBinding {
        &self.binding
    }

    pub const fn service_schema(&self) -> &ServiceSchema {
        &self.service_schema
    }
}

/// One consumer-scoped semantic-binding policy input for candidate review.
///
/// The consumer and checked context name an exact occurrence in the resolver-owned
/// closure. The binding is policy authority supplied only to that compilation; this
/// input is not proof of an audit, an audit receipt, or package admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsumerScopedSemanticBindingReviewInput {
    consumer: PackageKey,
    checked_context: PackageCheckedContext,
    binding: AcceptedSemanticBinding,
}

impl ConsumerScopedSemanticBindingReviewInput {
    pub fn new(
        consumer: PackageKey,
        checked_context: PackageCheckedContext,
        binding: AcceptedSemanticBinding,
    ) -> Self {
        Self {
            consumer,
            checked_context,
            binding,
        }
    }

    pub fn consumer(&self) -> &PackageKey {
        &self.consumer
    }

    pub const fn checked_context(&self) -> PackageCheckedContext {
        self.checked_context
    }

    pub fn binding(&self) -> &AcceptedSemanticBinding {
        &self.binding
    }
}

pub(super) fn semantic_bindings_by_consumer(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    execution_profile: Option<target::TargetProfile>,
    roster: &PackageOccurrenceRoster,
    inputs: &[ConsumerScopedSemanticBindingReviewInput],
) -> Result<
    BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    CompileResolvedPackageReviewsError,
> {
    let mut seen_roles = BTreeSet::new();
    let mut bindings_by_consumer =
        BTreeMap::<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>::new();
    if inputs.is_empty() {
        return Ok(bindings_by_consumer);
    }
    let closure = target_closure.source_closure();
    for input in inputs {
        let consumer = input.consumer();
        let role = input.binding().role();
        if closure.custody(consumer).is_none() {
            return Err(
                CompileResolvedPackageReviewsError::SemanticBindingConsumerAbsent {
                    consumer: consumer.clone(),
                    role,
                },
            );
        }
        let context = input.checked_context();
        let expected_target = match context.purpose() {
            DependencyPurpose::Product => Some(target_closure.target_profile()),
            DependencyPurpose::Build => execution_profile,
        };
        if !roster.is_occurrence(consumer, context.purpose())
            || Some(context.target()) != expected_target
            || context.build_execution_profile() != execution_profile
        {
            return Err(
                CompileResolvedPackageReviewsError::SemanticBindingContextMismatch {
                    consumer: consumer.clone(),
                    role,
                },
            );
        }
        if !seen_roles.insert((consumer.clone(), context.purpose(), role)) {
            return Err(
                CompileResolvedPackageReviewsError::DuplicateConsumerSemanticBindingRole {
                    consumer: consumer.clone(),
                    role,
                },
            );
        }
        bindings_by_consumer
            .entry((consumer.clone(), context.purpose()))
            .or_default()
            .push(input.binding().clone());
    }
    Ok(bindings_by_consumer)
}

/// Derive non-authoritative semantic-binding proposals from a preliminary
/// compiler review. The bound recompilation remains the exact validator, and
/// its resulting provider and authority rows still require root policy.
///
/// Console and core ProcessExit use the same exit-binding proposal flow. Console
/// owns its schema in the provider package; ProcessExit owns its schema in core
/// and binds the provider package instead. Both retain the exact selected plan
/// and require the bound recompilation to validate that ownership relationship.
/// Intrinsic and imported exit leaves go through the same review: target linkage
/// changes realization, not the consumer's obligation to approve termination.
///
/// The exit proposal for the closure root also carries the
/// terminal-authority permissions its recognized compiler-intrinsic leaves
/// need at native realization: process termination for `exit_process`, and
/// process output and input for the `write_byte` and `read_byte` rows the
/// nominated provider declares. Without those rows the review has nothing to
/// decide, the lock retains no permission, and the accepted policy projected
/// for realization is empty, so the ordinary CLI route can never realize a
/// program that writes, reads or exits. Each row is a proposal, not a grant:
/// it surfaces in the review as its own blocking `terminal_permission`
/// decision, the final pass rejoins it to exactly one schema method, and
/// realization still checks the selected mechanism's exercised classes
/// against it. Only the root consumer receives them because permissions are
/// the consuming project's decision and every package's open permissions
/// propagate into one root policy, where an identical row accepted twice
/// would be a duplicate rather than a grant.
///
/// A discovered `FilesystemHost` or `TimeHost` service binding at the root
/// likewise carries the toolchain-settled cohort permission table for its
/// exact checked schema: every method row the canonical boundary declares
/// resolves its own settled disposition, so the proposal remains exact per
/// requirement and the accepted policy projected for realization covers the
/// demanded leaves. The table refuses to fabricate rows for requirements
/// outside the settled cohorts, and the accepted binding still rejoins each
/// row to its exact schema digest.
pub(super) fn candidate_semantic_binding_inputs(
    preliminary: &CompilerIssuedPackageReviewSet,
    root: &PackageKey,
) -> Result<Vec<ConsumerScopedSemanticBindingReviewInput>, CompileResolvedPackageReviewsError> {
    let mut inputs = Vec::new();
    for review in preliminary.reviews() {
        for candidate in &review.semantic_binding_candidates {
            let mut binding = candidate.binding().clone();
            let role = binding.role();
            if review.key() == root
                && review.checked_context().purpose() == DependencyPurpose::Product
            {
                let mut permission_identities =
                    terminal_psi::TerminalIdentityTableBuilder::default();
                let settled_permission_rows = match role {
                    AcceptedSemanticBindingRole::FilesystemHostService => {
                        provider_admission::filesystem_host_permission_rows(
                            candidate.service_schema(),
                            &mut permission_identities,
                        )
                        .ok()
                    }
                    AcceptedSemanticBindingRole::TimeHostService => {
                        provider_admission::time_host_permission_rows(
                            candidate.service_schema(),
                            &mut permission_identities,
                        )
                        .ok()
                    }
                    _ => None,
                };
                if matches!(
                    role,
                    AcceptedSemanticBindingRole::FilesystemHostService
                        | AcceptedSemanticBindingRole::TimeHostService
                ) {
                    let permissions = settled_permission_rows.ok_or_else(|| {
                        CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                            consumer: review.key().clone(),
                            role,
                        }
                    })?;
                    binding = binding
                        .with_terminal_authority_permissions(
                            permissions,
                            &permission_identities.snapshot(),
                        )
                        .map_err(|_| {
                            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                                consumer: review.key().clone(),
                                role,
                            }
                        })?;
                }
            } else {
                // Cached nominations are package-local, not root-local.
                // A hosted candidate may propose permissions, but only this
                // invocation's product root may carry them into policy.
                binding = binding
                    .with_terminal_authority_permissions(
                        Vec::new(),
                        &terminal_psi::TerminalIdentityTable::from(vec![String::new()]),
                    )
                    .map_err(|_| {
                        CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                            consumer: review.key().clone(),
                            role,
                        }
                    })?;
            }
            inputs.push(ConsumerScopedSemanticBindingReviewInput::new(
                review.key().clone(),
                review.checked_context(),
                binding,
            ));
        }
    }
    Ok(inputs)
}

/// The terminal-authority permission rows for the hosted leaves of one
/// nominated provider schema: process termination for the
/// exact `exit_process` requirement, process output for `write_byte` and
/// process input for `read_byte`. Each class is the one the compiler's own
/// mechanism classification assigns to that hosted leaf. The readable names
/// only select the schema methods, as the binding candidate itself does; each
/// row binds the schema digest and requirement identity, so a lookalike
/// schema cannot reuse an accepted row. The exit leaf is the nomination
/// itself and must be present exactly once; the byte leaves are proposed only
/// when a Console provider declares their external rows. The separate
/// ProcessExit role proposes only termination, never console I/O authority.
fn candidate_hosted_exit_permissions(
    provider: &ProviderPlan,
    role: AcceptedSemanticBindingRole,
    permission_identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Option<Vec<ServiceTerminalAuthorityPermission>> {
    const LEAVES: [(&str, TerminalAuthorityClass, bool); 3] = [
        (
            "exit_process",
            TerminalAuthorityClass::ProcessTermination,
            true,
        ),
        ("write_byte", TerminalAuthorityClass::ProcessOutput, false),
        ("read_byte", TerminalAuthorityClass::ProcessInput, false),
    ];
    let leaves = match role {
        AcceptedSemanticBindingRole::ConsoleExitProcessI32 => &LEAVES[..],
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32 => &LEAVES[..1],
        _ => return None,
    };
    let mut permissions = Vec::with_capacity(leaves.len());
    for &(name, class, required) in leaves {
        let mut methods =
            candidate_hosted_exit_methods(provider).filter(|method| method.name == name);
        let Some(method) = methods.next() else {
            if required {
                return None;
            }
            continue;
        };
        if methods.next().is_some() {
            return None;
        }
        permissions.push(ServiceTerminalAuthorityPermission::new(
            provider.schema.identity_digest(),
            permission_identities
                .intern(provider.schema.identity_text(method.requirement_identity)),
            TerminalAuthorityDisposition::from_classes([class]),
        ));
    }
    Some(permissions)
}

/// Nominate exact package-owned requirement surfaces that the consumer's
/// checked API actually reaches. Readable names guide this confined candidate
/// pass only; the returned row binds exact package ownership, nominal path, and
/// normalized schema and must be consumed by the compiler on the final pass.
///
/// Each canonical host boundary path has its own semantic role; every
/// referenced path may nominate at most one (package, path) candidate.
pub(super) fn candidate_service_bindings(
    checked: &CheckedCompilation,
    consumer: &PackageKey,
) -> Result<Vec<SemanticBindingReviewCandidate>, CompileResolvedPackageReviewsError> {
    const HOST_SERVICE_PATHS: &[(&str, AcceptedSemanticBindingRole)] = &[
        (
            "FilesystemHost",
            AcceptedSemanticBindingRole::FilesystemHostService,
        ),
        ("TimeHost", AcceptedSemanticBindingRole::TimeHostService),
    ];
    let mut bindings = Vec::new();
    let referenced = checked_callable_service_references(checked, consumer)?;
    for (path, role) in HOST_SERVICE_PATHS {
        let mut candidate_packages = referenced
            .iter()
            .filter_map(|&service| {
                (checked.symbols.display_path(service, "::") == *path)
                    .then(|| checked.symbols.symbol_package_identity(service))
                    .flatten()
            })
            .collect::<BTreeSet<_>>();
        // A root-bound ProgramEntry is not a package callable, so its own
        // reach and carrier fields nominate through the entry's service
        // requirements instead of callable reaches.
        for requirement in checked.execution().program_entry_service_requirements() {
            let definitions = checked
                .traits()
                .iter()
                .filter(|definition| {
                    definition.symbol == requirement
                        && definition.is_boundary
                        && checked.symbols.display_path(definition.symbol, "::") == *path
                })
                .collect::<Vec<_>>();
            let [definition] = definitions.as_slice() else {
                continue;
            };
            if let Some(package) = checked
                .typed
                .symbols
                .symbol_package_identity(definition.symbol)
            {
                candidate_packages.insert(package);
            }
        }
        let candidates = candidate_packages.iter().collect::<Vec<_>>();
        let [package] = candidates.as_slice() else {
            if candidates.is_empty() {
                continue;
            }
            return Err(
                CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                    consumer: consumer.clone(),
                    role: *role,
                    candidate_count: candidates.len(),
                },
            );
        };
        let binding = checked
            .execution()
            .candidate_service_binding(
                &checked.providers().boundary_calling_plan_realizations,
                *role,
                **package,
                path,
            )
            .map_err(
                |_| CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                    consumer: consumer.clone(),
                    role: *role,
                },
            )?;
        let definitions = checked
            .traits()
            .iter()
            .filter(|definition| {
                checked
                    .typed
                    .symbols
                    .symbol_package_identity(definition.symbol)
                    == Some(**package)
                    && checked.symbols.display_path(definition.symbol, "::") == *path
            })
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(
                CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                    consumer: consumer.clone(),
                    role: *role,
                },
            );
        };
        let Some(service_schema) = provider_planning::service_schema::from_typed(
            provider_planning::ProviderPlanningContext::new(
                checked,
                source::DependencyScope::Product,
            ),
            &checked.providers().boundary_calling_plan_realizations,
            definition,
        ) else {
            return Err(
                CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                    consumer: consumer.clone(),
                    role: *role,
                },
            );
        };
        if package_compilation::accepted_service_schema_digest(binding.role(), &service_schema)
            != binding.normalized_schema_digest()
        {
            return Err(
                CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                    consumer: consumer.clone(),
                    role: *role,
                },
            );
        }
        bindings.push(SemanticBindingReviewCandidate::new(binding, service_schema));
    }
    Ok(bindings)
}

/// A checked dependency can nominate its target entry schema before an
/// application is checked. The proposal grants no entry authority: the
/// application's bound compilation must replay the exact package/schema and
/// calling pair, and the resulting review still requires consumer acceptance.
pub(super) fn candidate_target_entry_binding(
    checked: &CheckedCompilation,
    owner: &PackageKey,
    target: target::TargetProfile,
) -> Result<Option<SemanticBindingReviewCandidate>, CompileResolvedPackageReviewsError> {
    let slot = target.program_entry_slot();
    let (Some(package), Some(schema_name)) = (slot.physical_contract_package, slot.boundary_schema)
    else {
        return Ok(None);
    };
    let role = package_compilation::program_entry_semantic_binding_role(package);
    let mut definitions = checked.traits().iter().filter(|definition| {
        definition.is_boundary
            && definition.name.as_str() == schema_name
            && checked
                .typed
                .symbols
                .symbol_package_identity(definition.symbol)
                == Some(owner.identity())
    });
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    if definitions.next().is_some() {
        return Err(
            CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                consumer: owner.clone(),
                role,
                candidate_count: 2,
            },
        );
    }
    let path = checked.symbols.display_path(definition.symbol, "::");
    let binding = checked
        .execution()
        .candidate_service_binding(
            &checked.providers().boundary_calling_plan_realizations,
            role,
            owner.identity(),
            &path,
        )
        .map_err(
            |_| CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: owner.clone(),
                role,
            },
        )?;
    let service_schema = provider_planning::service_schema::from_typed(
        provider_planning::ProviderPlanningContext::new(checked, source::DependencyScope::Product),
        &checked.providers().boundary_calling_plan_realizations,
        definition,
    )
    .ok_or_else(
        || CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
            consumer: owner.clone(),
            role,
        },
    )?;
    Ok(Some(SemanticBindingReviewCandidate::new(
        binding,
        service_schema,
    )))
}

/// Binding nomination consumes symbol-backed checked facts, not their audit
/// rendering. Keep the public/boundary/build surface and exact invocation
/// edges distinct from transitive reach; neither can stand in for the other.
fn checked_callable_service_references(
    checked: &CheckedCompilation,
    consumer: &PackageKey,
) -> Result<Vec<SymbolHandle>, CompileResolvedPackageReviewsError> {
    let mut services = Vec::new();
    let reaches = &checked.facts.service_reaches;
    for machine in checked.machines().iter().filter(|machine| {
        checked
            .typed
            .symbols
            .symbol_package_identity(machine.symbol)
            == Some(consumer.identity())
            && (machine.is_public
                || machine.supply_mode.is_boundary_declaration()
                || Some(machine.symbol) == checked.build().selected_build_machine_symbol)
    }) {
        let missing = || CompileResolvedPackageReviewsError::Compilation {
            package: consumer.clone(),
            diagnostics: vec![diagnostics::Diagnostic::error(format!(
                "checked callable `{}` has no service-reach or synchronous-invocation facts",
                checked.symbols.display_path(machine.symbol, "::"),
            ))],
        };
        let reach = reaches.for_machine(machine.symbol).ok_or_else(missing)?;
        let invocations = checked
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .find(|fact| fact.machine == machine.symbol)
            .ok_or_else(missing)?;
        let published = match reach.interface {
            language_semantics::ServiceReachInterface::PublishedCeiling(row) => Some(row),
            language_semantics::ServiceReachInterface::InternalInferred => None,
        };
        let body = (machine.body_is_present
            && matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::CheckedBody
                    | language_semantics::MachineSupplyMode::Boundary
            ))
        .then_some([reach.inferred_transitive, reach.concrete_transitive]);
        for row in published
            .into_iter()
            .chain(body.into_iter().flatten())
            .chain(
                reach
                    .unresolved_installation_reaches
                    .iter()
                    .map(|reach| reach.upper_bound),
            )
        {
            for &service in reaches.rows.services(row) {
                services.push(
                    reaches
                        .services
                        .definition(service)
                        .ok_or_else(missing)?
                        .symbol,
                );
            }
        }
        let published_invocations = match invocations.plan.interface {
            language_semantics::SynchronousInvocationInterface::PublishedCeiling => {
                invocations.published_targets.as_slice()
            }
            language_semantics::SynchronousInvocationInterface::InternalInferred => &[],
        };
        services.extend(
            published_invocations
                .iter()
                .chain(&invocations.checked_inferred_targets)
                .filter_map(|target| match target {
                    flow_effects::InvocationTarget::Service(symbol) => Some(*symbol),
                    flow_effects::InvocationTarget::Parameter(_) => None,
                }),
        );
    }
    services.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    services.dedup();
    Ok(services)
}

/// Retain hosted proposals while the checked provider plans still exist.
/// Permissions remain proposals for the subsequent policy decision. The
/// consumer-scoping step removes them from dependency and build-host inputs,
/// including candidates recovered from the package-local review cache.
pub(super) fn candidate_hosted_exit_bindings(
    checked: &CheckedCompilation,
    consumer: &PackageKey,
) -> Result<Vec<SemanticBindingReviewCandidate>, CompileResolvedPackageReviewsError> {
    let mut bindings = Vec::new();
    for role in [
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
    ] {
        let candidates = checked
            .execution()
            .selected_provider_provenance
            .iter()
            .filter_map(|retained| {
                candidate_hosted_exit_provider_package(checked, retained, role)
                    .map(|package| (retained, package))
            })
            .collect::<Vec<_>>();
        let (retained, package) = match candidates.as_slice() {
            [] => continue,
            [candidate] => *candidate,
            _ => {
                return Err(
                    CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                        consumer: consumer.clone(),
                        role,
                        candidate_count: candidates.len(),
                    },
                );
            }
        };
        let invalid = || CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
            consumer: consumer.clone(),
            role,
        };
        let provider = &retained.plan;
        let binding = AcceptedSemanticBinding::new(
            role,
            package,
            checked
                .typed
                .symbols
                .display_path(retained.provider.schema.symbol(), "::"),
            provider.schema.identity_digest(),
            provider.identity_digest(),
        )
        .map_err(|_| invalid())?;
        let mut permission_identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let binding = binding
            .with_terminal_authority_permissions(
                candidate_hosted_exit_permissions(provider, role, &mut permission_identities)
                    .ok_or_else(invalid)?,
                &permission_identities.snapshot(),
            )
            .map_err(|_| invalid())?;
        bindings.push(SemanticBindingReviewCandidate::new(
            binding,
            provider.schema.clone(),
        ));
    }
    Ok(bindings)
}

fn candidate_hosted_exit_provider_package(
    checked: &CheckedCompilation,
    retained: &provider_planning::SelectedProviderReviewProvenance,
    role: AcceptedSemanticBindingRole,
) -> Option<semantic_vocabulary::PackageKeyIdentity> {
    let provider = &retained.plan;
    let package = provider.provider_type_package_identity?;
    let schema = retained.provider.schema.symbol();
    let schema_package = checked.symbols.symbol_package_identity(schema);
    let schema_path = checked.symbols.display_path(schema, "::");
    let expected_schema_owner = match role {
        AcceptedSemanticBindingRole::ConsoleExitProcessI32 => {
            schema_path == "Console" && schema_package == Some(package)
        }
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32 => {
            schema_path == "ProcessExit"
                && schema_package.is_none()
                && checked.symbols.symbol_source_origin(schema)
                    == Some(source::SourceOrigin::Toolchain)
        }
        _ => false,
    };
    (expected_schema_owner
        && provider.origin_package_identity == Some(package)
        && candidate_hosted_exit_methods(provider).any(|method| method.name == "exit_process"))
    .then_some(package)
}

fn candidate_hosted_exit_methods(provider: &ProviderPlan) -> impl Iterator<Item = &ServiceMethod> {
    provider.rows.iter().filter_map(|row| {
        if matches!(
            row.binding,
            ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Import { .. }
        ) {
            provider.schema.method_for_row(row, &provider.identities)
        } else {
            None
        }
    })
}

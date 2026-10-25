//! Project semantic-binding proposals while checked source and provider facts exist.
//!
//! These are exact schema nominations, not permission grants or accepted inputs.
//! Package review scopes proposals to consumers and makes policy decisions; it
//! must not interpret checked symbols, reaches or provider plans itself.

use crate::record::SemanticBindingReviewCandidate;
use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ServiceMethod};
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeSet;
use symbols::SymbolHandle;

/// Projection failures without resolver or consumer-policy ownership.
#[derive(Debug, Clone)]
pub enum SemanticBindingProjectionError {
    Ambiguous {
        role: AcceptedSemanticBindingRole,
        candidate_count: usize,
    },
    Invalid {
        role: AcceptedSemanticBindingRole,
    },
    MissingCallableFacts {
        diagnostics: Vec<Diagnostic>,
    },
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
pub fn candidate_service_bindings(
    checked: &CheckedCompilation,
    consumer: PackageKeyIdentity,
) -> Result<Vec<SemanticBindingReviewCandidate>, SemanticBindingProjectionError> {
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
            return Err(SemanticBindingProjectionError::Ambiguous {
                role: *role,
                candidate_count: candidates.len(),
            });
        };
        let binding = checked
            .execution()
            .candidate_service_binding(
                &checked.providers().boundary_calling_plan_realizations,
                *role,
                **package,
                path,
            )
            .map_err(|_| SemanticBindingProjectionError::Invalid { role: *role })?;
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
            return Err(SemanticBindingProjectionError::Invalid { role: *role });
        };
        let Some(service_schema) = provider_planning::service_schema::from_typed(
            provider_planning::ProviderPlanningContext::new(
                checked,
                source::DependencyScope::Product,
            ),
            &checked.providers().boundary_calling_plan_realizations,
            definition,
        ) else {
            return Err(SemanticBindingProjectionError::Invalid { role: *role });
        };
        if package_compilation::accepted_service_schema_digest(binding.role(), &service_schema)
            != binding.normalized_schema_digest()
        {
            return Err(SemanticBindingProjectionError::Invalid { role: *role });
        }
        bindings.push(SemanticBindingReviewCandidate::new(binding, service_schema));
    }
    Ok(bindings)
}

/// A checked dependency can nominate its target entry schema before an
/// application is checked. The proposal grants no entry authority: the
/// application's bound compilation must replay the exact package/schema and
/// calling pair, and the resulting review still requires consumer acceptance.
pub fn candidate_target_entry_binding(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
    target: target::TargetProfile,
) -> Result<Option<SemanticBindingReviewCandidate>, SemanticBindingProjectionError> {
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
                == Some(owner)
    });
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    if definitions.next().is_some() {
        return Err(SemanticBindingProjectionError::Ambiguous {
            role,
            candidate_count: 2,
        });
    }
    let path = checked.symbols.display_path(definition.symbol, "::");
    let binding = checked
        .execution()
        .candidate_service_binding(
            &checked.providers().boundary_calling_plan_realizations,
            role,
            owner,
            &path,
        )
        .map_err(|_| SemanticBindingProjectionError::Invalid { role })?;
    let service_schema = provider_planning::service_schema::from_typed(
        provider_planning::ProviderPlanningContext::new(checked, source::DependencyScope::Product),
        &checked.providers().boundary_calling_plan_realizations,
        definition,
    )
    .ok_or_else(|| SemanticBindingProjectionError::Invalid { role })?;
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
    consumer: PackageKeyIdentity,
) -> Result<Vec<SymbolHandle>, SemanticBindingProjectionError> {
    let mut services = Vec::new();
    let reaches = &checked.facts.service_reaches;
    for machine in checked.machines().iter().filter(|machine| {
        checked
            .typed
            .symbols
            .symbol_package_identity(machine.symbol)
            == Some(consumer)
            && (machine.is_public
                || machine.supply_mode.is_boundary_declaration()
                || Some(machine.symbol) == checked.build().selected_build_machine_symbol)
    }) {
        let missing = || SemanticBindingProjectionError::MissingCallableFacts {
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
pub fn candidate_hosted_exit_bindings(
    checked: &CheckedCompilation,
) -> Result<Vec<SemanticBindingReviewCandidate>, SemanticBindingProjectionError> {
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
                return Err(SemanticBindingProjectionError::Ambiguous {
                    role,
                    candidate_count: candidates.len(),
                });
            }
        };
        let invalid = || SemanticBindingProjectionError::Invalid { role };
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
        && provider.rows.iter().enumerate().any(|(position, row)| {
            provider
                .schema
                .method_for_row(row, &provider.identities)
                .is_some_and(|method| method.name == "exit_process")
                && retained
                    .row_external_completion_leaves
                    .get(position)
                    .is_some_and(|leaves| !leaves.is_empty())
        }))
    .then_some(package)
}

fn candidate_hosted_exit_methods(provider: &ProviderPlan) -> impl Iterator<Item = &ServiceMethod> {
    provider.rows.iter().filter_map(|row| {
        if matches!(
            row.binding,
            ProviderBinding::CompilerIntrinsic { .. }
                | ProviderBinding::Import { .. }
                | ProviderBinding::CheckedAdapter { .. }
        ) {
            provider.schema.method_for_row(row, &provider.identities)
        } else {
            None
        }
    })
}

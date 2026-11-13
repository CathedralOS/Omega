/*
Projection of semantic-binding review candidates from checked source, service
reaches, and provider plans (`candidate_service_bindings`,
`candidate_target_entry_binding`, and `candidate_hosted_exit_bindings`).

We nominate exact schema bindings while checked symbols and provider plans are
still live:
1. `candidate_service_bindings` inspects `consumer`'s public, boundary, and
   selected build machines plus root `ProgramEntry` service requirements to
   nominate at most one `(package, path)` candidate per canonical host boundary
   (`FilesystemHost`, `TimeHost`).
2. `candidate_target_entry_binding` nominates `owner`'s boundary trait matching
   `target.program_entry_slot()` without granting entry authority.
3. `candidate_hosted_exit_bindings` inspects `selected_provider_provenance` for
   `Console` (`exit_process` required, `write_byte`/`read_byte` optional) and
   `ProcessExit` (`exit_process` only), binding each terminal-authority
   permission row to the schema digest and requirement identity.
*/

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
            .candidate_service_binding(*role, **package, path)
            .map_err(|_| SemanticBindingProjectionError::Invalid { role: *role })?;
        let service_schema = checked
            .execution()
            .service_declarations
            .select(**package, path)
            .map_err(|_| SemanticBindingProjectionError::Invalid { role: *role })?
            .declaration
            .schema
            .clone();
        bindings.push(SemanticBindingReviewCandidate::new(binding, service_schema));
    }
    Ok(bindings)
}

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
        .candidate_service_binding(role, owner, &path)
        .map_err(|_| SemanticBindingProjectionError::Invalid { role })?;
    let service_schema = checked
        .execution()
        .service_declarations
        .select(owner, &path)
        .map_err(|_| SemanticBindingProjectionError::Invalid { role })?
        .declaration
        .schema
        .clone();
    Ok(Some(SemanticBindingReviewCandidate::new(
        binding,
        service_schema,
    )))
}

fn checked_callable_service_references(
    checked: &CheckedCompilation,
    consumer: PackageKeyIdentity,
) -> Result<Vec<SymbolHandle>, SemanticBindingProjectionError> {
    let source = checked.source_inspection().program();
    let mut services = Vec::new();
    for machine in source.typed.machines().iter().filter(|machine| {
        source.typed.symbols.symbol_package_identity(machine.symbol) == Some(consumer)
            && (machine.is_public
                || machine.supply_mode.is_boundary_declaration()
                || Some(machine.symbol) == checked.build().selected_build_machine_symbol)
    }) {
        // A declaration that has no accepted executable or boundary header
        // cannot acquire behavior from a historical checker summary.
        let reach = if machine.supply_mode.is_boundary_declaration() {
            let boundary = source.accepted_boundary(machine.symbol).map_err(|reason| {
                SemanticBindingProjectionError::MissingCallableFacts {
                    diagnostics: vec![Diagnostic::error(reason)],
                }
            })?;
            &boundary.published_service_ceiling
        } else {
            &source
                .accepted_machine(machine.symbol)
                .map_err(
                    |reason| SemanticBindingProjectionError::MissingCallableFacts {
                        diagnostics: vec![Diagnostic::error(reason)],
                    },
                )?
                .published_service_ceiling
        };
        for &service in reach {
            services.push(source.service_symbol(service).map_err(|reason| {
                SemanticBindingProjectionError::MissingCallableFacts {
                    diagnostics: vec![Diagnostic::error(reason)],
                }
            })?);
        }
    }
    services.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    services.dedup();
    Ok(services)
}

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

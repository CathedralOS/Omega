//! Fresh package declaration-role inputs for immutable boundary catalogues.
//!
//! Confined review may propose an exact package's source roles. A consumer
//! supplies its separately accepted role anchors. Neither route selects a
//! provider, supplies native backing or changes User/Toolchain provenance.

use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use typed_trees::TypedTrees;
use typed_trees::typed_trees::declarations::boundary_catalogues::{
    BoundaryCatalogueDeclaration, BoundaryCatalogueIssuer, BoundaryCatalogueRole,
};
use typed_trees::typed_trees::declarations::hosted_intrinsics::HostedIntrinsicRole;

#[derive(Clone, Copy)]
pub struct SourceCatalogueBinding<'request> {
    pub package: PackageKeyIdentity,
    pub role: BoundaryCatalogueRole,
    pub declaration_path: &'request str,
    pub accepted_schema: [u8; 32],
}

#[derive(Default)]
pub struct SourceCatalogueAdmission<'request> {
    /// Explicit library suppliers in the invocation's confined Product review.
    /// Application roots and Build-only dependencies are never nominees.
    pub discover_packages: &'request [PackageKeyIdentity],
    pub bindings: &'request [SourceCatalogueBinding<'request>],
}

pub fn prepare_source_catalogues(
    typed: &mut TypedTrees,
    admission: &SourceCatalogueAdmission<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let trace = std::env::var_os("OMEGA_SOURCE_CATALOGUE_TRACE").is_some();
    if trace {
        eprintln!(
            "SOURCE_CATALOGUES discover={:?} accepted={}",
            admission.discover_packages,
            admission.bindings.len()
        );
    }
    // No restored or sibling source checkpoint can donate a prior role.
    typed.boundary_catalogue_declarations.clear();
    for (ordinal, binding) in admission.bindings.iter().enumerate() {
        if binding.role == BoundaryCatalogueRole::None
            || binding.accepted_schema == [0; 32]
            || binding.declaration_path.is_empty()
            || binding.declaration_path.chars().any(char::is_control)
            || admission.bindings[..ordinal]
                .iter()
                .any(|prior| prior.role == binding.role && prior.package == binding.package)
        {
            return Err(vec![Diagnostic::error(
                "source catalogue admission has an invalid or duplicate package declaration role",
            )]);
        }
    }
    let mut rows = Vec::new();
    for machine in typed.machines() {
        let Some(span) = typed.symbols.symbol_source_span(machine.symbol) else {
            continue;
        };
        let Some(source) = typed.symbols.source_file(span) else {
            continue;
        };
        if trace
            && matches!(
                machine.name.as_str(),
                "console_read_byte"
                    | "console_write_byte"
                    | "console_exit_process"
                    | "process_exit"
                    | "uefi_transfer_to_entry"
                    | "uefi_firmware_return"
            )
        {
            eprintln!(
                "SOURCE_CATALOGUES source={} package={:?} purpose={:?}",
                machine.name, source.package_identity, source.dependency_scope
            );
        }
        if source.dependency_scope != source::DependencyScope::Product {
            continue;
        }
        let Some(package) = source.package_identity else {
            continue;
        };
        let candidate =
            if let Some(declaration) = typed.hosted_intrinsic_source_declaration(machine.symbol) {
                let role = match declaration.role {
                    HostedIntrinsicRole::ProcessExit => BoundaryCatalogueRole::HostedProcessExit,
                    _ => BoundaryCatalogueRole::HostedConsole,
                };
                Some((
                    role,
                    declaration.service.symbol,
                    declaration.requirement.symbol,
                ))
            } else {
                typed.machine_states(machine).first().and_then(|entry| {
                    let requirement = typed.uefi_handoff_source_requirement(entry.symbol)?;
                    let (owner, _) = typed.trait_machine_signature(requirement)?;
                    Some((
                        BoundaryCatalogueRole::UefiHandoff,
                        owner.symbol,
                        requirement,
                    ))
                })
            };
        let Some((role, owner, requirement)) = candidate else {
            continue;
        };
        if trace {
            eprintln!(
                "SOURCE_CATALOGUES candidate={} package={:?} role={:?} owner={}",
                machine.name,
                package,
                role,
                typed.symbols.display_path(owner, "::")
            );
        }
        let (issuer, accepted_schema) = if admission.discover_packages.contains(&package) {
            (BoundaryCatalogueIssuer::Proposed, [0; 32])
        } else {
            let mut bindings = admission.bindings.iter().filter(|binding| {
                binding.package == package
                    && binding.role == role
                    && typed.symbols.display_path(owner, "::") == binding.declaration_path
            });
            let Some(binding) = bindings.next() else {
                if trace {
                    eprintln!("SOURCE_CATALOGUES no accepted role for {}", machine.name);
                }
                continue;
            };
            if bindings.next().is_some() {
                continue;
            }
            (BoundaryCatalogueIssuer::Accepted, binding.accepted_schema)
        };
        rows.push(BoundaryCatalogueDeclaration {
            role,
            issuer,
            package: Some(package),
            accepted_schema,
            realization: machine.symbol,
            requirement,
            source: span,
        });
    }
    for row in rows {
        let _ = typed.boundary_catalogue_declarations.append(row);
    }
    if trace {
        eprintln!(
            "SOURCE_CATALOGUES retained={}",
            typed.boundary_catalogue_declarations.len()
        );
    }
    Ok(())
}

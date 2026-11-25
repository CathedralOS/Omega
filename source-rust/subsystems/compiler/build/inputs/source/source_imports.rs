//! Resolve the imports written in each source file. A standalone program
//! uses its captured source root. A packaged program also has the dependency
//! grants of its checked Build or Product. A file checked in both contexts
//! has a separate set of imports in each.
//!
//! `files` selects exact filenames. `logical` finds declarations for legacy
//! namespace imports and inventories package operators. `package_selection`
//! checks dependencies before and after a target is chosen. `bindings` records
//! which file each import selected for name resolution.

mod bindings;
mod build_prelude;
mod entry_contract_seed;
mod files;
pub(crate) use files::{generated_file_imports, product_file_sources};
mod source_targets;
pub use source_targets::{source_target, source_targets};
mod frontier;
mod generated;
mod import_queue;
mod logical;
mod package_selection;
mod paths;
mod product_queries;
mod supplied_library;
pub use product_queries::{ProductQueryDependency, ProductQueryInputs};
pub use supplied_library::SuppliedBuildLibrary;

pub(crate) use bindings::{
    PendingPackageImport, ResolvedSourceImport, pending_build_imports,
    retain_import_bindings,
};
pub(crate) use build_prelude::prepare_build_prelude;
pub(crate) use entry_contract_seed::EntryContractSource;
pub(crate) use entry_contract_seed::entry_contract_seeds;
pub(crate) use frontier::{
    SourceStorage, ToolchainContractCustody, validate_package_source_frontier,
};
pub(crate) use generated::{
    DependencyGeneratedSource, admit_dependency_build_handoffs,
    dependency_generated_source_logical_path,
};
pub use generated::{GeneratedSourceExtension, admit_generated_extension};
pub(crate) use import_queue::ImportQueue;
pub(crate) use logical::{LogicalSourceDiscovery, retained_unit};

pub(crate) use package_selection::{
    ReconciledPackageImport, ReconciledPackageImportRequest, reconciled_package_import,
    source_import_scope,
};
pub use paths::bundled_core_root;
pub(crate) use paths::bundled_omega_root;

use crate::PackageCompilationInputs;
use bindings::ImportOccurrence;
use diagnostics::Diagnostic;
use files::{FileImport, FileSelection};
use paths::{identifier_path_text, is_bundled_omega_path};
use source_compiler::ParsedSourceBatch;
use source_compiler::SourceImportTarget;
use source_provenance::DependencyScope;
use std::path::{Path, PathBuf};

pub(crate) fn discover_imports(
    parsed: &ParsedSourceBatch,
    root_path: &Path,
    contracts: &ToolchainContractCustody,
    filesystem: &source_capture::SourceFileSystem,
    retained: &mut Vec<ResolvedSourceImport>,
    discovery: &mut impl LogicalSourceDiscovery,
) -> Result<Vec<(PathBuf, DependencyScope)>, Vec<Diagnostic>> {
    let default_root = if filesystem.is_dir(root_path) {
        root_path
    } else {
        root_path.parent().unwrap_or_else(|| Path::new("."))
    };
    let core = bundled_core_root()?;
    let mut imports = Vec::new();
    for unit in parsed.sources.span_or_empty(parsed.batch) {
        let source_root = if unit.path.starts_with(&core) {
            core.as_path()
        } else {
            default_root
        };
        for import in &unit.imports {
            let ordinal = import.ordinal;
            if let Some(file) = FileImport::from(unit, import) {
                let root = contracts.root_for(&unit.path).unwrap_or(source_root);
                match file.standalone(root, None, filesystem)? {
                    FileSelection::Selected(resolved) => {
                        imports.push((resolved.path.clone(), unit.scope));
                        retained.push(resolved);
                    }
                    FileSelection::Unprepared => {}
                    FileSelection::Package(_) => unreachable!("a standalone input has no mounted graph"),
                }
                continue;
            }
            let SourceImportTarget::Namespace(members) = &import.target else { unreachable!() };
            let (root, prefix) = if paths::is_bundled_core_path(members) {
                (core.as_path(), 3)
            } else if is_bundled_omega_path(members) {
                return Err(vec![Diagnostic::error(
                    "ordinary library imports require a declared dependency",
                )]);
            } else if let Some(root) = contracts.root_for(&unit.path) {
                (root, paths::contract_package_prefix(root, &members[0])?)
            } else {
                (source_root, 0)
            };
            let logical = members[prefix..]
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect::<Vec<_>>();
            // modules.md, "Names and imports": in Build source "a logical
            // `use dependency;` may remain deferred until explicit dependency
            // registration and preparation make its exact declaration scope
            // available". A Build import of its own package therefore
            // selects only what declares the imported name, and one that
            // names nothing here selects no source and is left for the stage
            // that prepares the dependency it names.
            let own_build_import =
                unit.scope == DependencyScope::Build && prefix == 0 && root == default_root;
            let selected = if own_build_import {
                discovery.select_declaring(root, &logical, filesystem, Vec::new())?
            } else {
                discovery.select(root, &logical, filesystem)?
            };
            if selected.is_empty() {
                if own_build_import {
                    continue;
                }
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` selects no logical source namespace",
                    identifier_path_text(members)
                ))]);
            }
            for path in selected {
                retained.push(ResolvedSourceImport {
                    occurrence: ImportOccurrence::new(unit.source_id, ordinal, members, prefix),
                    path: path.clone(),
                });
                imports.push((path, unit.scope));
            }
        }
    }
    Ok(imports)
}

pub(crate) fn discover_imports_with_packages(
    parsed: &ParsedSourceBatch,
    packages: &PackageCompilationInputs,
    generated_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
    discovery: &mut impl LogicalSourceDiscovery,
    filesystem: &source_capture::SourceFileSystem,
) -> Result<Vec<(PathBuf, DependencyScope)>, Vec<Diagnostic>> {
    let (imports, _) = discover_package_imports(
        parsed,
        packages,
        PackageImportPhase::ExactTarget(generated_owner),
        retained,
        contract_custody,
        discovery,
        filesystem,
    )?;
    Ok(imports)
}

#[derive(Clone, Copy)]
pub(crate) enum PackageImportPhase {
    TargetIndependent,
    ExactTarget(Option<semantic_vocabulary::PackageKeyIdentity>),
}

pub(crate) fn discover_package_imports(
    parsed: &ParsedSourceBatch,
    packages: &PackageCompilationInputs,
    phase: PackageImportPhase,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
    discovery: &mut impl LogicalSourceDiscovery,
    filesystem: &source_capture::SourceFileSystem,
) -> Result<(Vec<(PathBuf, DependencyScope)>, Vec<PendingPackageImport>), Vec<Diagnostic>> {
    let generated_owner = match phase {
        PackageImportPhase::TargetIndependent => None,
        PackageImportPhase::ExactTarget(owner) => owner,
    };
    let mut imports = Vec::new();
    let mut requests = Vec::new();
    for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
        let requester =
            generated_owner.or_else(|| packages.package_for_source(&parsed_source.path));
        let purpose = source_import_scope(packages, parsed_source.scope, &parsed_source.path);
        let contract_root = contract_custody.root_for(&parsed_source.path);
        for import in &parsed_source.imports {
            let ordinal = import.ordinal;
            if let Some(file) = FileImport::from(parsed_source, import) {
                match file.reconciled(packages, requester, purpose, contract_root, filesystem)? {
                    FileSelection::Selected(resolved) => {
                        imports.push((resolved.path.clone(), parsed_source.scope));
                        retained.push(resolved);
                    }
                    FileSelection::Package(pending) => collect_package_import(pending, phase, packages, discovery, &mut imports, retained, &mut requests)?,
                    FileSelection::Unprepared => {}
                }
                continue;
            }
            let SourceImportTarget::Namespace(members) = &import.target else { unreachable!() };
            match reconciled_package_import(
                &parsed_source.path,
                members,
                requester,
                purpose,
                contract_root,
                packages,
            )? {
                ReconciledPackageImport::Toolchain { root, logical_path }
                | ReconciledPackageImport::ToolchainContract { root, logical_path } => {
                    let prefix = members.len() - logical_path.len();
                    let selected = discovery.select(&root, &logical_path, filesystem)?;
                    if selected.is_empty() {
                        return Err(vec![Diagnostic::error(format!(
                            "import `{}` selects no logical toolchain namespace",
                            identifier_path_text(members)
                        ))]);
                    }
                    for path in selected {
                        retained.push(ResolvedSourceImport {
                            occurrence: ImportOccurrence::new(
                                parsed_source.source_id,
                                ordinal,
                                members,
                                prefix,
                            ),
                            path: path.clone(),
                        });
                        imports.push((path, parsed_source.scope));
                    }
                }
                ReconciledPackageImport::Package(request) => {
                    let prefix = members.len() - request.relative_path.components().count();
                    let pending = PendingPackageImport {
                        occurrence: ImportOccurrence::new(
                            parsed_source.source_id,
                            ordinal,
                            members,
                            prefix,
                        ),
                        scope: parsed_source.scope,
                        request,
                    };
                    collect_package_import(pending, phase, packages, discovery, &mut imports, retained, &mut requests)?;
                }
            }
        }
    }
    Ok((imports, requests))
}

fn collect_package_import(
    pending: PendingPackageImport,
    phase: PackageImportPhase,
    packages: &PackageCompilationInputs,
    discovery: &mut impl LogicalSourceDiscovery,
    imports: &mut Vec<(PathBuf, DependencyScope)>,
    retained: &mut Vec<ResolvedSourceImport>,
    requests: &mut Vec<PendingPackageImport>,
) -> Result<(), Vec<Diagnostic>> {
    let scope = pending.scope;
    match phase {
        PackageImportPhase::TargetIndependent => {
            for physical in pending.physical_sources(packages, discovery)? {
                imports.push((physical, scope));
            }
            requests.push(pending);
        }
        PackageImportPhase::ExactTarget(_) => {
            for resolved in pending.resolve_for_exact_target(packages, discovery)? {
                imports.push((resolved.path.clone(), scope));
                retained.push(resolved);
            }
        }
    }
    Ok(())
}

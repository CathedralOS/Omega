//! Discover source imports in authored order, then retain their exact bindings.
//!
//! Package selection owns dependency-scope and generated-source policy; path
//! resolution confines physical candidates to their source roots. The frontier
//! retains parsed-file ownership and the bindings rejoin exact source instances.
//! This owner does not schedule lexing, parsing, or source-program checking.

mod bindings;
mod build_prelude;
mod entry_contract_seed;
mod frontier;
mod generated;
mod import_queue;
mod package_selection;
mod paths;
mod product_queries;
mod project;
pub use product_queries::{ProductQueryDependency, ProductQueryInputs};

pub(crate) use bindings::{
    PendingPackageImport, ResolvedSourceImport, retain_module_import_bindings,
};
pub(crate) use build_prelude::prepare_build_prelude;
pub(crate) use entry_contract_seed::EntryContractSource;
pub(crate) use entry_contract_seed::entry_contract_seeds;
pub(crate) use frontier::{
    SourceStorage, ToolchainContractCustody, validate_package_source_frontier,
};
pub(crate) use generated::{DependencyGeneratedSource, admit_dependency_generated_sources};
pub use generated::{GeneratedSourceExtension, admit_generated_extension};
pub(crate) use import_queue::ImportQueue;

pub(crate) use package_selection::{
    ReconciledPackageImport, ReconciledPackageImportRequest, reconciled_package_import,
    source_import_scope,
};
pub use paths::bundled_core_root;
pub(crate) use paths::bundled_omega_root;
pub(crate) use project::ProjectSourceSelection;
pub use project::project_roots;

use crate::PackageCompilationInputs;
use bindings::{ImportOccurrence, direct_source_import};
use diagnostics::Diagnostic;
use package_selection::local_source_exists;
use paths::{identifier_path_text, is_bundled_omega_path, normalize_path, resolve_source_path};
use source::DependencyScope;
use std::path::{Path, PathBuf};
use syntax_trees::item::Item;
use syntax_trees::{ParsedSources, SyntaxTrees};

/// Discover standalone imports without interpreting dependency declarations.
/// Package aliases are meaningful only on the reconciled package-aware path.
pub(crate) fn discover_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    root_path: &Path,
    filesystem: &source::SourceFileSystem,
    retained: &mut Vec<ResolvedSourceImport>,
) -> Result<Vec<(PathBuf, DependencyScope)>, Vec<Diagnostic>> {
    let root_dir = if filesystem.is_dir(root_path) {
        root_path.to_path_buf()
    } else {
        root_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    };
    let parsed_sources = parsed.sources.span_or_empty(parsed.batch);
    let mut imports = Vec::with_capacity(parsed_sources.len());

    for parsed_source in parsed_sources {
        let source_root = standalone_source_root(&root_dir, &parsed_source.path, filesystem)?;
        for (offset, root_item) in parsed_source.root_items[parsed_source.import_root_range.clone()]
            .iter()
            .enumerate()
        {
            let ordinal = parsed_source.import_root_range.start + offset;
            if let Item::Use(use_item) = syntax_trees.root_item(*root_item) {
                let members = syntax_trees.items.identifier_path_members(use_item.path);
                if is_bundled_omega_path(syntax_trees, members)
                    && local_source_exists(syntax_trees, &source_root, members, filesystem)
                {
                    return Err(vec![Diagnostic::error(format!(
                        "import `{}` in {} collides with a requester-local source; `omega::language` names are claimed by the bundled toolchain namespace, so the local source is unreachable -- rename or remove it",
                        identifier_path_text(syntax_trees, members),
                        parsed_source.path.display(),
                    ))]);
                }
                let resolved = normalize_path(
                    &resolve_source_path(syntax_trees, &source_root, members, filesystem)?,
                    filesystem,
                )?;
                let prefix = if is_bundled_omega_path(syntax_trees, members) {
                    2
                } else {
                    0
                };
                let owner = if prefix == 2 {
                    bundled_omega_root()?
                } else {
                    source_root.clone()
                };
                retained.push(ResolvedSourceImport {
                    occurrence: ImportOccurrence::new(
                        syntax_trees,
                        parsed_source.source_id,
                        ordinal,
                        members,
                        prefix,
                    ),
                    requires_module: !direct_source_import(
                        syntax_trees,
                        &owner,
                        &members[prefix..],
                        &resolved,
                        filesystem,
                    ),
                    path: resolved.clone(),
                });
                // Standalone imports inherit the importer's checked
                // instance: a build-scope source's imports are host-context
                // content, never product content by accident of sharing
                // bytes.
                imports.push((resolved, parsed_source.scope));
            }
        }
    }

    Ok(imports)
}

/// Preserve standalone compilation of the bundled std sources while their
/// authored self-imports use ordinary package-local paths. This is standalone
/// toolchain routing only; package-aware compilation resolves the same paths
/// through exact package custody.
fn standalone_source_root(
    default_root: &Path,
    source: &Path,
    filesystem: &source::SourceFileSystem,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let standard_library_root = bundled_omega_root()?.join("std");
    Ok(filesystem
        .canonicalize(source)
        .ok()
        .filter(|source| source.starts_with(&standard_library_root))
        .map(|_| standard_library_root)
        .unwrap_or_else(|| default_root.to_path_buf()))
}

/// Resolve imports exclusively through a reconciled, requester-local package
/// graph. This path never reads or combines dependency rows from `build.omg`.
pub(crate) fn discover_imports_with_packages(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    generated_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
) -> Result<Vec<(PathBuf, DependencyScope)>, Vec<Diagnostic>> {
    let (imports, _) = discover_package_imports(
        parsed,
        syntax_trees,
        packages,
        PackageImportPhase::ExactTarget(generated_owner),
        retained,
        contract_custody,
    )?;
    Ok(imports)
}

#[derive(Clone, Copy)]
pub(crate) enum PackageImportPhase {
    TargetIndependent,
    ExactTarget(Option<semantic_vocabulary::PackageKeyIdentity>),
}

/// Retain package requests until the exact child checks generated-source collisions.
///
/// Each returned path carries the checked instance it joins under: the
/// importer's own scope. A source both scopes import loads twice — the two
/// instances share source bytes and package identity but nothing else.
pub(crate) fn discover_package_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    phase: PackageImportPhase,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
) -> Result<(Vec<(PathBuf, DependencyScope)>, Vec<PendingPackageImport>), Vec<Diagnostic>> {
    let filesystem = packages.source_filesystem();
    let generated_owner = match phase {
        PackageImportPhase::TargetIndependent => None,
        PackageImportPhase::ExactTarget(owner) => owner,
    };
    let mut imports = Vec::new();
    let mut requests = Vec::new();
    for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
        // Physical loaders normalize paths. Generated producers already own the
        // package association; their virtual paths need no filesystem lookup.
        let requester =
            generated_owner.or_else(|| packages.package_for_source(&parsed_source.path));
        let purpose = source_import_scope(packages, parsed_source.scope, &parsed_source.path);
        let contract_root = contract_custody.root_for(&parsed_source.path);
        for (offset, root_item) in parsed_source.root_items[parsed_source.import_root_range.clone()]
            .iter()
            .enumerate()
        {
            let ordinal = parsed_source.import_root_range.start + offset;
            let Item::Use(use_item) = syntax_trees.root_item(*root_item) else {
                continue;
            };
            let members = syntax_trees.items.identifier_path_members(use_item.path);
            match reconciled_package_import(
                syntax_trees,
                &parsed_source.path,
                members,
                requester,
                purpose,
                contract_root,
                packages,
            )? {
                ReconciledPackageImport::Toolchain(path) => {
                    retained.push(ResolvedSourceImport {
                        occurrence: ImportOccurrence::new(
                            syntax_trees,
                            parsed_source.source_id,
                            ordinal,
                            members,
                            2,
                        ),
                        requires_module: !direct_source_import(
                            syntax_trees,
                            &bundled_omega_root()?,
                            &members[2..],
                            &path,
                            filesystem,
                        ),
                        path: path.clone(),
                    });
                    imports.push((path, parsed_source.scope));
                }
                ReconciledPackageImport::ToolchainContract { root, path } => {
                    retained.push(ResolvedSourceImport {
                        occurrence: ImportOccurrence::new(
                            syntax_trees,
                            parsed_source.source_id,
                            ordinal,
                            members,
                            0,
                        ),
                        requires_module: !direct_source_import(
                            syntax_trees,
                            &root,
                            members,
                            &path,
                            filesystem,
                        ),
                        path: path.clone(),
                    });
                    imports.push((path, parsed_source.scope));
                }
                ReconciledPackageImport::Package(request) => {
                    let prefix = members.len() - request.relative_path.components().count();
                    let pending = PendingPackageImport {
                        occurrence: ImportOccurrence::new(
                            syntax_trees,
                            parsed_source.source_id,
                            ordinal,
                            members,
                            prefix,
                        ),
                        scope: parsed_source.scope,
                        request,
                    };
                    match phase {
                        PackageImportPhase::TargetIndependent => {
                            if let Some(physical) = pending.physical_source(packages)? {
                                imports.push((physical, parsed_source.scope));
                            }
                            requests.push(pending);
                        }
                        PackageImportPhase::ExactTarget(_) => {
                            let resolved = pending.resolve_for_exact_target(packages)?;
                            imports.push((resolved.path.clone(), parsed_source.scope));
                            retained.push(resolved);
                        }
                    }
                }
            }
        }
    }
    Ok((imports, requests))
}

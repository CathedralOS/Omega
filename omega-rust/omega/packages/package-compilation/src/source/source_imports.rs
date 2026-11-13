/*
We discover `use` imports in authored order across parsed sources and retain their exact module-import bindings (`ResolvedSourceImport`, `PendingPackageImport`).

We provide two import discovery routes:
1. `discover_imports` resolves standalone imports by authored logical names within the project or reserved `omega::language::core` mount.
2. `discover_package_imports` resolves imports exclusively through the reconciled `PackageCompilationInputs` graph in two phases: `TargetIndependent` (captured logical source discovery during checkpoint preparation) and `ExactTarget` (joining dependency-generated source bundles and rejecting physical/generated collisions).
*/

/*
Why every discovered import carries its importer's `DependencyScope`:
In Omega's two-checked-context model (`Build` vs `Product`), a source file imported by both contexts loads as two distinct checked instances that share immutable source bytes and `PackageKeyIdentity` but never share `SourceId` coordinates or symbol tables. Propagating `parsed_source.scope` on every discovered import ensures a `Build`-scope helper's transitive imports stay in the `Build` host context rather than leaking into `Product`.
*/

/*
Provenance:
- Import occurrence custody and `SourceScopedTopLevelBinding` construction: `src/source/source_imports/bindings.rs`.
- Conditional `<build-prelude>` injection: `src/source/source_imports/build_prelude.rs`.
- Multi-target `ProgramEntry` contract source seeding: `src/source/source_imports/entry_contract_seed.rs`.
- Source frontier storage (`SourceStorage`) and `SourceCatalogue` cohort selection: `src/source/source_imports/frontier.rs`.
- Generated-source extension and dependency handoff admission: `src/source/source_imports/generated.rs`.
- Captured logical module inventory on registered owner rows: `src/source/source_imports/logical.rs`.
- Scope-keyed `(PathBuf, DependencyScope)` import queue: `src/source/source_imports/import_queue.rs`.
- Reconciled package alias resolution and missing-edge diagnostics: `src/source/source_imports/package_selection.rs`.
- Reserved toolchain mount recognition and captured-path normalization: `src/source/source_imports/paths.rs`.
- Paused `Build` product-query dependency mounting and discovery: `src/source/source_imports/product_queries.rs`.
- Companion `build.omg` / `main.omg` project root discovery: `src/source/source_imports/project.rs`.
*/

mod bindings;
mod build_prelude;
mod entry_contract_seed;
mod frontier;
mod generated;
mod import_queue;
mod logical;
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
pub use entry_contract_seed::selected_program_entry_boundary;
pub(crate) use frontier::{
    SourceStorage, ToolchainContractCustody, validate_package_source_frontier,
};
pub(crate) use generated::{DependencyGeneratedSource, admit_dependency_build_handoffs};
pub use generated::{GeneratedSourceExtension, admit_generated_extension};
pub(crate) use import_queue::ImportQueue;
pub(crate) use logical::LogicalSourceDiscovery;

pub(crate) use package_selection::{
    ReconciledPackageImport, ReconciledPackageImportRequest, reconciled_package_import,
    source_import_scope,
};
pub use paths::bundled_core_root;
pub(crate) use paths::bundled_omega_root;
pub(crate) use project::ProjectSourceSelection;
pub use project::project_roots;

use crate::PackageCompilationInputs;
use bindings::ImportOccurrence;
use diagnostics::Diagnostic;
use paths::{identifier_path_text, is_bundled_omega_path};
use source::DependencyScope;
use std::path::{Path, PathBuf};
use syntax_trees::item::Item;
use syntax_trees::{ParsedSources, SyntaxTrees};

pub(crate) fn discover_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    root_path: &Path,
    filesystem: &source::SourceFileSystem,
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
        for (offset, item) in unit.root_items[unit.import_root_range.clone()]
            .iter()
            .enumerate()
        {
            let Item::Use(import) = syntax_trees.root_item(*item) else {
                continue;
            };
            let members = syntax_trees.items.identifier_path_members(import.path);
            let (root, prefix) = if paths::is_bundled_core_path(syntax_trees, members) {
                (core.as_path(), 3)
            } else if is_bundled_omega_path(syntax_trees, members) {
                return Err(vec![Diagnostic::error(
                    "ordinary library imports require a declared dependency",
                )]);
            } else {
                (source_root, 0)
            };
            let logical = members[prefix..]
                .iter()
                .map(|name| name.as_str(syntax_trees).to_owned())
                .collect::<Vec<_>>();
            let selected = discovery.select(root, &logical, filesystem)?;
            if selected.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` selects no logical source namespace",
                    identifier_path_text(syntax_trees, members)
                ))]);
            }
            for path in selected {
                retained.push(ResolvedSourceImport {
                    occurrence: ImportOccurrence::new(
                        syntax_trees,
                        unit.source_id,
                        unit.import_root_range.start + offset,
                        members,
                        prefix,
                    ),
                    path: path.clone(),
                });
                imports.push((path, unit.scope));
            }
        }
    }
    Ok(imports)
}

pub(crate) fn discover_imports_with_packages(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    generated_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
    discovery: &mut impl LogicalSourceDiscovery,
    filesystem: &source::SourceFileSystem,
) -> Result<Vec<(PathBuf, DependencyScope)>, Vec<Diagnostic>> {
    let (imports, _) = discover_package_imports(
        parsed,
        syntax_trees,
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
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    phase: PackageImportPhase,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
    discovery: &mut impl LogicalSourceDiscovery,
    filesystem: &source::SourceFileSystem,
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
                ReconciledPackageImport::Toolchain { root, logical_path }
                | ReconciledPackageImport::ToolchainContract { root, logical_path } => {
                    let prefix = members.len() - logical_path.len();
                    let selected = discovery.select(&root, &logical_path, filesystem)?;
                    if selected.is_empty() {
                        return Err(vec![Diagnostic::error(format!(
                            "import `{}` selects no logical toolchain namespace",
                            identifier_path_text(syntax_trees, members)
                        ))]);
                    }
                    for path in selected {
                        retained.push(ResolvedSourceImport {
                            occurrence: ImportOccurrence::new(
                                syntax_trees,
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
                            for physical in pending.physical_sources(packages, discovery)? {
                                imports.push((physical, parsed_source.scope));
                            }
                            requests.push(pending);
                        }
                        PackageImportPhase::ExactTarget(_) => {
                            for resolved in pending.resolve_for_exact_target(packages, discovery)? {
                                imports.push((resolved.path.clone(), parsed_source.scope));
                                retained.push(resolved);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok((imports, requests))
}

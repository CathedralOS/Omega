use crate::frontend::ParsedSources;
use crate::source::SourceFile;
use arena::Arena;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source::SourceMap;
use source::SourceOrigin;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use syntax_trees::SyntaxTrees;

#[derive(Clone, Default)]
pub struct SourceStorage {
    pub files: Arena<SourceFile>,
    pub sources: SourceMap,
    pub syntax_trees: SyntaxTrees,
    pub(crate) resolved_imports: Vec<crate::frontend::ResolvedSourceImport>,
    default_package_root: PathBuf,
    default_package_identity: Option<PackageKeyIdentity>,
    package_roots: Vec<RegisteredPackageRoot>,
    toolchain_root: Option<PathBuf>,
    toolchain_contract_roots: BTreeMap<PathBuf, PathBuf>,
    toolchain_contract_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisteredPackageRoot {
    root: PathBuf,
    identity: Option<PackageKeyIdentity>,
}

impl SourceStorage {
    pub fn for_compilation(default_package_root: PathBuf, toolchain_root: PathBuf) -> Self {
        Self {
            default_package_root: normalize_directory(default_package_root),
            toolchain_root: Some(normalize_directory(toolchain_root)),
            ..Self::default()
        }
    }

    pub fn for_package_compilation(
        default_package_root: PathBuf,
        default_package_identity: PackageKeyIdentity,
        toolchain_root: PathBuf,
    ) -> Self {
        Self {
            default_package_root: normalize_directory(default_package_root),
            default_package_identity: Some(default_package_identity),
            toolchain_root: Some(normalize_directory(toolchain_root)),
            ..Self::default()
        }
    }

    pub fn register_reconciled_package_root(
        &mut self,
        package_root: PathBuf,
        identity: PackageKeyIdentity,
    ) {
        self.register_package_source(package_root, Some(identity));
    }

    /// Register one exact source path as closed toolchain contract custody.
    /// Used only for the bundled hosted-entry fallback. The file keeps its
    /// contract root so `package_relative_source` retains the canonical path.
    /// A reconciled supplier must not be registered here: doing so would erase
    /// its ordinary package identity and defeat accepted-binding replay.
    pub fn register_toolchain_contract_root(&mut self, path: PathBuf, contract_root: PathBuf) {
        self.toolchain_contract_roots.insert(
            normalize_directory(path),
            normalize_directory(contract_root),
        );
    }

    /// The bundled contract fallback owns an entire closed toolchain subtree:
    /// its transitive `use` closure resolves inside the same root, so every
    /// file under it inherits the contract's toolchain custody rather than a
    /// package identity it cannot have.
    pub fn register_toolchain_contract_dir(&mut self, root: PathBuf) {
        let root = normalize_directory(root);
        if !self.toolchain_contract_dirs.contains(&root) {
            self.toolchain_contract_dirs.push(root);
            self.toolchain_contract_dirs
                .sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
        }
    }

    /// A path registered for toolchain contract custody clears the package
    /// source frontier even when it sits outside every reconciled root: its
    /// origin and identity are fixed toolchain by registration, so loading it
    /// cannot launder package content into the compiled program.
    pub fn is_toolchain_contract_source(&self, path: &Path) -> bool {
        self.toolchain_contract_root_for(path).is_some()
    }

    /// Snapshot toolchain contract custody for import reconciliation, where a
    /// mutable `resolved_imports` borrow forbids retaining `&self`.
    pub fn toolchain_contract_custody(&self) -> ToolchainContractCustody {
        ToolchainContractCustody {
            roots: self.toolchain_contract_roots.clone(),
            dirs: self.toolchain_contract_dirs.clone(),
        }
    }

    fn toolchain_contract_root_for(&self, path: &Path) -> Option<&Path> {
        toolchain_contract_root_for(
            &self.toolchain_contract_roots,
            &self.toolchain_contract_dirs,
            path,
        )
    }

    fn register_package_source(
        &mut self,
        package_root: PathBuf,
        identity: Option<PackageKeyIdentity>,
    ) {
        let package_root = RegisteredPackageRoot {
            root: normalize_directory(package_root),
            identity,
        };
        if !self.package_roots.contains(&package_root) {
            self.package_roots.push(package_root);
            self.package_roots
                .sort_by_key(|registered| std::cmp::Reverse(registered.root.components().count()));
        }
    }

    pub fn extend(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
            let (package_root, package_identity, origin) =
                self.source_metadata(&parsed_source.path, parsed_source.origin);
            let added = self.sources.add_checked_instance(
                parsed_source.path.clone(),
                parsed_source.source.to_string(),
                package_root,
                package_identity,
                origin,
                source::SourceResolutionStratum::Base,
                parsed_source.scope,
            );

            debug_assert_eq!(added.source_id, parsed_source.source_id);

            self.files.append(SourceFile {
                source_id: parsed_source.source_id,
                path: parsed_source.path.clone(),
                root_items: parsed_source.root_items.clone(),
            });
        }

        Ok(())
    }

    pub fn next_source_id(&self) -> usize {
        self.sources.len()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    fn source_metadata(
        &self,
        path: &Path,
        origin_override: Option<SourceOrigin>,
    ) -> (PathBuf, Option<PackageKeyIdentity>, SourceOrigin) {
        if let Some(contract_root) = self.toolchain_contract_root_for(path) {
            return (contract_root.to_path_buf(), None, SourceOrigin::Toolchain);
        }
        if origin_override == Some(SourceOrigin::Toolchain) {
            return (
                self.toolchain_root.clone().unwrap_or_default(),
                None,
                SourceOrigin::Toolchain,
            );
        }
        if let Some(toolchain_root) = &self.toolchain_root
            && path.starts_with(toolchain_root)
        {
            let toolchain_is_core =
                toolchain_root.file_name().and_then(|name| name.to_str()) == Some("core");
            let core_roots = if toolchain_is_core {
                vec![toolchain_root.clone()]
            } else {
                vec![
                    toolchain_root.join("core"),
                    toolchain_root.join("language").join("core"),
                ]
            };
            if let Some(core_root) = core_roots
                .into_iter()
                .filter(|root| path.starts_with(root))
                .max_by_key(|root| root.components().count())
            {
                return (core_root, None, SourceOrigin::Toolchain);
            }

            // Standalone compilation retains the legacy compiler-bundle lane
            // until every std/alloc consumer has an exact source-role
            // compatibility check. Package-aware compilation supplies only
            // the core root here, so ordinary std/alloc dependencies never
            // inherit this compatibility provenance.
            if !toolchain_is_core
                && let Some(package_root) = ["std", "alloc"]
                    .into_iter()
                    .flat_map(|name| {
                        [
                            toolchain_root.join(name),
                            toolchain_root.join("language").join(name),
                        ]
                    })
                    .filter(|root| path.starts_with(root))
                    .max_by_key(|root| root.components().count())
            {
                return (package_root, None, SourceOrigin::Toolchain);
            }
        }

        let package = self
            .package_roots
            .iter()
            .find(|package| path.starts_with(&package.root));
        match package {
            Some(package) => (package.root.clone(), package.identity, SourceOrigin::User),
            None => (
                self.default_package_root.clone(),
                self.default_package_identity,
                SourceOrigin::User,
            ),
        }
    }
}

fn normalize_directory(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

/// Closed toolchain contract custody shared by source metadata, the package
/// source frontier, and import reconciliation: exact contract files map to
/// their registered root, while bundled contract fallback roots admit their
/// whole closed subtree.
#[derive(Clone, Default)]
pub struct ToolchainContractCustody {
    roots: BTreeMap<PathBuf, PathBuf>,
    dirs: Vec<PathBuf>,
}

impl ToolchainContractCustody {
    pub fn root_for(&self, path: &Path) -> Option<&Path> {
        toolchain_contract_root_for(&self.roots, &self.dirs, path)
    }
}

fn toolchain_contract_root_for<'a>(
    roots: &'a BTreeMap<PathBuf, PathBuf>,
    dirs: &'a [PathBuf],
    path: &Path,
) -> Option<&'a Path> {
    roots.get(path).map(PathBuf::as_path).or_else(|| {
        dirs.iter()
            .find(|dir| path.starts_with(dir.as_path()))
            .map(PathBuf::as_path)
    })
}

#[cfg(test)]
mod tests {
    use super::{PackageKeyIdentity, SourceOrigin, SourceStorage};
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn standalone_bundle_keeps_deepest_legacy_toolchain_roots() {
        let storage = SourceStorage::for_compilation(
            PathBuf::from("workspace/application"),
            PathBuf::from("toolchain"),
        );
        for (source, expected_root, expected_origin) in [
            (
                "toolchain/std/targets/uefi_x86_64/entry.omg",
                "toolchain/std",
                SourceOrigin::Toolchain,
            ),
            (
                "toolchain/core/targets/common.omg",
                "toolchain/core",
                SourceOrigin::Toolchain,
            ),
            (
                "toolchain/language/std/targets/legacy.omg",
                "toolchain/language/std",
                SourceOrigin::Toolchain,
            ),
        ] {
            let (root, package, origin) = storage.source_metadata(Path::new(source), None);
            assert_eq!(root, PathBuf::from(expected_root));
            assert_eq!(package, None);
            assert_eq!(origin, expected_origin);
        }
    }

    #[test]
    fn package_mode_core_root_remains_exact_toolchain_source() {
        let storage = SourceStorage::for_package_compilation(
            PathBuf::from("workspace/application"),
            PackageKeyIdentity::from_digest([1; 32]).expect("nonzero package identity"),
            PathBuf::from("toolchain/core"),
        );
        let (root, package, origin) =
            storage.source_metadata(Path::new("toolchain/core/extent.omg"), None);
        assert_eq!(root, PathBuf::from("toolchain/core"));
        assert_eq!(package, None);
        assert_eq!(origin, SourceOrigin::Toolchain);
    }
}

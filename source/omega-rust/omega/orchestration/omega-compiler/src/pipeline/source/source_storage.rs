use crate::pipeline::frontend::ParsedSources;
use crate::pipeline::source::SourceFile;
use crate::source::SourceMap;
use psi_arena::Arena;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_source::SourceOrigin;
use psi_syntax_trees::SyntaxTrees;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct SourceStorage {
    pub files: Arena<SourceFile>,
    pub sources: SourceMap,
    pub syntax_trees: SyntaxTrees,
    default_package_root: PathBuf,
    default_package_identity: Option<PackageKeyIdentity>,
    package_roots: Vec<RegisteredPackageRoot>,
    toolchain_root: Option<PathBuf>,
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
            let added = self.sources.add_with_metadata(
                parsed_source.path.clone(),
                parsed_source.source.to_string(),
                package_root,
                package_identity,
                origin,
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
            // The bundled packages now live directly under `source/library`.
            // Retain the former `language/<package>` recognition while
            // migrated trees and cached fixtures drain, but always choose the
            // deepest matching package root so exact toolchain provenance is
            // independent of the surrounding library directory.
            let package_root = ["core", "std", "alloc"]
                .into_iter()
                .flat_map(|name| {
                    [
                        toolchain_root.join(name),
                        toolchain_root.join("language").join(name),
                    ]
                })
                .filter(|root| path.starts_with(root))
                .max_by_key(|root| root.components().count())
                .unwrap_or_else(|| toolchain_root.clone());
            return (package_root, None, SourceOrigin::Toolchain);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_and_legacy_toolchain_packages_keep_their_deepest_roots() {
        let storage = SourceStorage::for_compilation(
            PathBuf::from("workspace/application"),
            PathBuf::from("toolchain"),
        );
        for (source, expected_root) in [
            ("toolchain/std/targets/uefi_x64/entry.omg", "toolchain/std"),
            ("toolchain/core/targets/common.omg", "toolchain/core"),
            (
                "toolchain/language/std/targets/legacy.omg",
                "toolchain/language/std",
            ),
            ("toolchain/shared/prelude.omg", "toolchain"),
        ] {
            let (root, package, origin) = storage.source_metadata(Path::new(source), None);
            assert_eq!(root, PathBuf::from(expected_root));
            assert_eq!(package, None);
            assert_eq!(origin, SourceOrigin::Toolchain);
        }
    }
}

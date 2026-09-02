use crate::pipeline::frontend::ParsedSources;
use crate::pipeline::source::SourceFile;
use crate::source::SourceMap;
use psi_arena::Arena;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_source::SourceOrigin;
use psi_syntax_trees::SyntaxTrees;
use std::path::{Path, PathBuf};

#[derive(Clone, Default)]
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

#[cfg(test)]
mod tests {
    use super::*;

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

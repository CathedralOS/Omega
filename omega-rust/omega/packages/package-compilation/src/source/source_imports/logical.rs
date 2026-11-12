//! Discover authored logical modules from immutable captured source units.
//!
//! Discovery uses the normal parser in a separate syntax forest. Its coordinates
//! and declarations never join a checked source cohort: the selected paths go
//! through the ordinary scope, owner and stratum admission afterwards. This cache
//! records only module membership, not visibility or declaration resolution.

use super::frontier::RegisteredPackageRoot;
use diagnostics::Diagnostic;
use psi_compiler::{SourceInput, UnmeasuredProduction};
use source::{DependencyScope, LoadedSource, SourceFileSystem, SourceId};
use std::path::{Path, PathBuf};
use syntax_trees::{SyntaxTrees, item::Item};

#[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
pub(crate) struct LogicalSourceInventory {
    filesystem: SourceFileSystem,
    excluded_roots: Vec<PathBuf>,
    units: Vec<LogicalSourceUnit>,
}

#[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
struct LogicalSourceUnit {
    path: PathBuf,
    module: Vec<String>,
}

pub(crate) trait LogicalSourceDiscovery {
    fn select(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        self.select_owned(root, logical_path, filesystem, Vec::new())
    }
    fn select_owned(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
    fn siblings(
        &mut self,
        root: &Path,
        source: &Path,
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
}

impl LogicalSourceDiscovery for Vec<RegisteredPackageRoot> {
    fn select_owned(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let inventory = owner_inventory(self, root, filesystem, excluded_roots)?;
        // A namespace import also permits qualified descendant references.
        // Ordinary acquisition observes use declarations, not expression paths;
        // acquire that namespace here and leave exposure to semantic selection.
        Ok(inventory
            .units
            .iter()
            .filter(|unit| {
                logical_path.starts_with(&unit.module) || unit.module.starts_with(logical_path)
            })
            .map(|unit| unit.path.clone())
            .collect())
    }
    fn siblings(
        &mut self,
        root: &Path,
        source: &Path,
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let inventory = owner_inventory(self, root, filesystem, excluded_roots)?;
        let Some(selected) = inventory.units.iter().find(|unit| unit.path == source) else {
            return Ok(Vec::new());
        };
        Ok(inventory
            .units
            .iter()
            .filter(|unit| unit.module == selected.module)
            .map(|unit| unit.path.clone())
            .collect())
    }
}

fn owner_inventory<'a>(
    owners: &'a mut Vec<RegisteredPackageRoot>,
    root: &Path,
    filesystem: &SourceFileSystem,
    mut excluded_roots: Vec<PathBuf>,
) -> Result<&'a LogicalSourceInventory, Vec<Diagnostic>> {
    excluded_roots.sort();
    excluded_roots.dedup();
    let position = match owners.iter().position(|owner| owner.root == root) {
        Some(position) => position,
        None => {
            // Anonymous and compiler-owned captures acquire their row here;
            // reconciled packages have already registered this exact root.
            owners.push(RegisteredPackageRoot {
                root: root.to_path_buf(),
                identity: None,
                logical_inventory: None,
            });
            owners.len() - 1
        }
    };
    let owner = &mut owners[position];
    if owner.logical_inventory.as_ref().is_none_or(|inventory| {
        inventory.filesystem != *filesystem || inventory.excluded_roots != excluded_roots
    }) {
        owner.logical_inventory = Some(LogicalSourceInventory::parse(
            root,
            filesystem,
            &excluded_roots,
        )?);
    }
    Ok(owner
        .logical_inventory
        .as_ref()
        .expect("owner inventory was retained"))
}

impl LogicalSourceInventory {
    fn parse(
        root: &Path,
        filesystem: &SourceFileSystem,
        excluded_roots: &[PathBuf],
    ) -> Result<Self, Vec<Diagnostic>> {
        let inventory = filesystem.captured_files(root).map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot discover logical modules in {}: {error}",
                root.display()
            ))]
        })?;
        // Nested package owners come from the admitted graph, never from a
        // filename convention or a second package-discovery authority.
        let paths = inventory
            .iter()
            .filter(|path| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("omg" | "omega")
                ) && !path.file_name().is_some_and(|name| name == "build.omg")
                    && !excluded_roots
                        .iter()
                        .any(|package| path.starts_with(package))
            })
            .cloned()
            .collect::<Vec<_>>();
        let loaded = paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                filesystem
                    .read_to_string(path)
                    .map(|source| LoadedSource {
                        source_id: SourceId(index),
                        path: path.clone(),
                        source,
                        origin: None,
                        scope: DependencyScope::Product,
                    })
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "cannot read captured logical source {}: {error}",
                            path.display()
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| vec![error])?;
        let mut syntax = SyntaxTrees::default();
        let parsed = SourceInput(loaded.iter().cloned().collect())
            .append(&mut syntax, &mut UnmeasuredProduction)
            .map_err(|mut errors| {
                // Discovery coordinates are private to this temporary forest.
                // Resolve them now rather than letting the caller mistake them
                // for IDs in the real checked source cohort.
                for error in &mut errors {
                    if let Some(span) = error.source_span.take()
                        && let Some(unit) = loaded.get(span.source_id.0)
                    {
                        let position = source::SourcePosition::of(&unit.source, span.span.start);
                        error.resolved_location = Some(diagnostics::ResolvedSourceLocation {
                            package: None,
                            path: unit.path.clone(),
                            line: position.line.min(u32::MAX as usize) as u32,
                            column: position.column.min(u32::MAX as usize) as u32,
                        });
                    }
                }
                errors
            })?;
        let mut result = Self {
            filesystem: filesystem.clone(),
            excluded_roots: excluded_roots.to_vec(),
            units: Vec::new(),
        };
        for unit in parsed.sources.span_or_empty(parsed.batch) {
            let mut module = None;
            for item in &unit.root_items {
                if let Item::Module(declaration) = syntax.root_item(*item) {
                    if module.is_some() {
                        return Err(vec![Diagnostic::error(format!(
                            "source {} declares more than one logical module",
                            unit.path.display()
                        ))]);
                    }
                    module = Some(
                        syntax
                            .items
                            .identifier_path_members(declaration.path)
                            .iter()
                            .map(|name| name.as_str(&syntax).to_owned())
                            .collect::<Vec<_>>(),
                    );
                }
            }
            result.units.push(LogicalSourceUnit {
                path: unit.path.clone(),
                module: module.unwrap_or_default(),
            });
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::{CapturedSourceNode, CapturedSourceTree};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn captured(rows: &[(&str, &str)]) -> SourceFileSystem {
        let mut entries = BTreeMap::new();
        for (name, body) in rows {
            let path = PathBuf::from(name);
            let mut parent = path.parent();
            while let Some(directory) = parent.filter(|path| !path.as_os_str().is_empty()) {
                entries.insert(directory.to_path_buf(), CapturedSourceNode::Directory);
                parent = directory.parent();
            }
            entries.insert(
                path,
                CapturedSourceNode::File {
                    bytes: Arc::from(body.as_bytes()),
                    executable: false,
                },
            );
        }
        let mut filesystem = SourceFileSystem::default();
        filesystem
            .add_capture(
                PathBuf::from("/logical-test"),
                Arc::new(CapturedSourceTree::new(entries).unwrap()),
            )
            .unwrap();
        filesystem
    }

    #[test]
    fn logical_membership_comes_from_headers_not_filenames() {
        let filesystem = captured(&[
            ("odd.omg", "public data Record {}"),
            ("another.omg", "public machine read() -> u8 { 7 }"),
            ("Record.omg", "module unrelated; public data Hidden {}"),
            ("renamed.omg", "module api; public data Export {}"),
            ("build.omg", "not product source syntax"),
        ]);
        let mut discovery = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            discovery
                .select(Path::new("/logical-test"), &["Record".into()], &filesystem)
                .unwrap(),
            ["/logical-test/another.omg", "/logical-test/odd.omg"].map(PathBuf::from)
        );
        assert_eq!(
            discovery
                .siblings(
                    Path::new("/logical-test"),
                    Path::new("/logical-test/odd.omg"),
                    &filesystem,
                    Vec::new()
                )
                .unwrap(),
            ["/logical-test/another.omg", "/logical-test/odd.omg"].map(PathBuf::from)
        );
        assert!(
            discovery
                .select(
                    Path::new("/logical-test"),
                    &["api".into(), "Export".into()],
                    &filesystem
                )
                .unwrap()
                .contains(&PathBuf::from("/logical-test/renamed.omg"))
        );
    }

    #[test]
    fn root_import_acquires_descendants_for_qualified_namespace_lookup() {
        let filesystem = captured(&[
            ("root.omg", "public data Root {}"),
            ("child.omg", "module child; public data Child {}"),
            ("deep.omg", "module child::deep; public data Deep {}"),
        ]);
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            owners
                .select(Path::new("/logical-test"), &[], &filesystem)
                .unwrap(),
            [
                "/logical-test/child.omg",
                "/logical-test/deep.omg",
                "/logical-test/root.omg"
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            owners
                .select(Path::new("/logical-test"), &["child".into()], &filesystem)
                .unwrap(),
            [
                "/logical-test/child.omg",
                "/logical-test/deep.omg",
                "/logical-test/root.omg"
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn named_parent_namespace_can_be_introduced_only_by_a_descendant_unit() {
        let filesystem = captured(&[("renamed.omg", "module outer::inner; public data Item {}")]);
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        let root = Path::new("/logical-test");
        assert_eq!(
            owners.select(root, &[], &filesystem).unwrap(),
            [root.join("renamed.omg")]
        );
        assert_eq!(
            owners.select(root, &["outer".into()], &filesystem).unwrap(),
            [root.join("renamed.omg")]
        );
    }

    #[test]
    fn owner_inventory_refreshes_when_capture_or_exclusions_change() {
        let first = captured(&[("one.omg", "public data One {}")]);
        let second = captured(&[
            ("one.omg", "module hidden; public data One {}"),
            ("two.omg", "public data Two {}"),
            ("child/three.omg", "public data Three {}"),
        ]);
        let root = Path::new("/logical-test");
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            owners.select(root, &[], &first).unwrap(),
            [root.join("one.omg")]
        );
        assert_eq!(
            owners.select(root, &["Two".into()], &second).unwrap(),
            [root.join("child/three.omg"), root.join("two.omg")]
        );
        assert_eq!(
            owners
                .select_owned(root, &["Two".into()], &second, vec![root.join("child")])
                .unwrap(),
            [root.join("two.omg")]
        );
        assert_eq!(
            owners.select(root, &["Two".into()], &second).unwrap(),
            [root.join("child/three.omg"), root.join("two.omg")]
        );
        assert_eq!(
            owners.len(),
            1,
            "capture refresh retains its existing owner row"
        );
    }

    #[test]
    fn owned_inventory_does_not_parse_another_packages_source() {
        let filesystem = captured(&[
            ("unit.omg", "public data Record {}"),
            ("child/not-ours.omg", "deliberately invalid foreign source"),
        ]);
        let paths = Vec::<RegisteredPackageRoot>::new()
            .select_owned(
                Path::new("/logical-test"),
                &["Record".into()],
                &filesystem,
                vec![PathBuf::from("/logical-test/child")],
            )
            .unwrap();
        assert_eq!(paths, [PathBuf::from("/logical-test/unit.omg")]);
    }

    #[test]
    fn discovery_coordinates_do_not_escape_into_checked_source_ids() {
        let filesystem = captured(&[("bad.omg", "machine")]);
        let errors = Vec::<RegisteredPackageRoot>::new()
            .select(Path::new("/logical-test"), &[], &filesystem)
            .unwrap_err();
        assert!(errors.iter().all(|error| error.source_span.is_none()));
        assert!(errors.iter().any(|error| {
            error
                .resolved_location
                .as_ref()
                .is_some_and(|location| location.path == Path::new("/logical-test/bad.omg"))
        }));
    }

    #[test]
    fn missing_capture_cannot_fall_back_to_live_directory_membership() {
        let error = SourceFileSystem::default()
            .captured_files(Path::new("/logical-test"))
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }
}
